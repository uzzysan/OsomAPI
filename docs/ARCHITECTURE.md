# Architektura OsomAPI

## Przegląd systemu

OsomAPI zbudowane jest jako workspace Cargo składający się z 6 crate'ów, gdzie każdy crate odpowiada za odseparowaną domenę funkcjonalną. Główny binarny crate (`osom-api`) orkiestruje przepływ danych przez pozostałe moduły.

## Diagram przepływu danych

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   Plik      │───▶│   Parser    │───▶│     LLM     │───▶│   Schema    │───▶│   Writer    │
│ wejściowy   │    │ (JSON/XML/  │    │ (Ollama/    │    │ (walidacja+ │    │ (JSON/XML/  │
│             │    │  CSV/PDF)   │    │  GPT/Claude)│    │  formatow.) │    │  SQLite)    │
└─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
       │                                                                               │
       │                    ┌─────────────┐                                           │
       └────────────────────│  Config     │◀──────────────────────────────────────────┘
                            │ (TOML/JSON) │
                            └─────────────┘
```

## Opis crate'ów

### osom-config

**Odpowiedzialność:** Ładowanie, parsowanie i walidacja pliku konfiguracyjnego.

**Kluczowe typy:**
- `Config` – główna struktura konfiguracji
- `LlmConfig` – enum z wariantami dla każdego dostawcy LLM
- `OutputSchema` – definicja schematu wyjściowego z polami
- `FieldType` – typy danych: String, Integer, Decimal, Boolean, Date, DateTime, Object, Array
- `Destination` – enum wyboru miejsca zapisu (JSON, XML, Database)
- `CaseNormalization` – opcje normalizacji wielkości liter

**Walidacja:**
- Sprawdzenie czy schemat zawiera co najmniej jedno pole
- Weryfikacja obecności klucza API dla dostawców chmurowych

### osom-parser

**Odpowiedzialność:** Ekstrakcja tekstowej reprezentacji danych z różnych formatów binarnych/tekstowych.

**Kluczowe typy:**
- `InputParser` – trait z metodą `async fn parse(&self, input: &[u8])`
- `ParsedData` – znormalizowana reprezentacja (content + source_type)
- `SourceType` – enum formatów: Json, Xml, Csv, Pdf

**Implementacje:**
- `JsonParser` – deserializacja przez `serde_json`, potem pretty-print
- `XmlParser` – ręczna iteracja przez `quick-xml::Reader`, konwersja do `serde_json::Value` zachowująca atrybuty (`@attr`) i tekst (`#text`), powtarzające się elementy zamieniane na tablice
- `CsvParser` – `csv::Reader`, konwersja wierszy na tablicę obiektów JSON
- `PdfParser` – `pdf_extract::extract_text_from_mem` dla bezpośredniej ekstrakcji z bajtów

### osom-llm

**Odpowiedzialność:** Komunikacja z modelami językowymi przez HTTP.

**Kluczowe typy:**
- `LlmClient` – async trait (via `async-trait`) z metodą `send(&self, prompt: &str)`
- `RetryingLlmClient` – wrapper z wykładniczym backoffem (exponential backoff) i ponawianiem prób dla błędów przejściowych (429, 5xx, timeout, rozłączenie sieci)
- `build_llm_client(&LlmConfig)` oraz `build_llm_client_with_settings(&LlmConfig, &AppSettings)` – fabryki klientów konfigurujące timeout i politykę ponawiania prób

**Implementacje klientów:**
| Dostawca | Endpoint | Uwagi |
|----------|----------|-------|
| Ollama | `{url}/api/generate` | Lokalna instancja, pole `prompt` |
| OpenAI | `https://api.openai.com/v1/chat/completions` | Bearer token, format messages |
| Gemini | `https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent` | Klucz w query param |
| Anthropic | `https://api.anthropic.com/v1/messages` | Header `x-api-key`, `max_tokens: 4096` |
| Copilot | `https://api.githubcopilot.com/chat/completions` | Bearer token |

**PromptBuilder:**
- Generuje prompt w języku polskim z instrukcjami JSON
- Dokumentuje każde pole ze schematu (nazwa, typ, opis, wymagalność, wartość domyślna)
- Dodaje instrukcję o normalizacji wielkości liter jeśli skonfigurowana

### osom-schema

**Odpowiedzialność:** Walidacja odpowiedzi LLM pod kątem schematu oraz formatowanie wartości zgodnie z konfiguracją.

**Kluczowe typy:**
- `FormattedData` – wektor rekordów `Map<String, Value>` gotowych do zapisu
- `FormatError` – błędy typowania i walidacji formatu
- `ValidationError` – błędy brakujących pól, niezgodności typów, nieprawidłowych tablic

**Proces:**
1. `validate()` – sprawdza czy odpowiedź LLM jest obiektem lub tablicą obiektów, weryfikuje wymagane pola, poprawność dat/czasów (`chrono`) oraz poprawność liczb dziesiętnych
2. `format_values()` – przekształca surową odpowiedź w sformatowane dane:
   - normalizuje i zaokrągla liczby dziesiętne (`Decimal`) do zdefiniowanej skali `scale`,
   - parsuje i ujednolica daty (`Date`) oraz daty i czasy (`DateTime`) do zadanego wzorca `strftime`,
   - aplikuje `normalize_case` dla stringów,
   - uzupełnia wartości domyślne.

### osom-writer

**Odpowiedzialność:** Zapis sformatowanych danych do wybranego celu.

**Implementacje:**
- `JsonWriter` – serializacja przez `serde_json::to_string_pretty` lub `to_string`, zapis async przez `tokio::fs::write`
- `XmlWriter` – generacja XML z escape'owaniem znaków specjalnych
- `DatabaseWriter` – obsługa bazy danych SQLite (z planowaną rozbudową o PostgreSQL/MySQL) przez `sqlx`, bezpieczne parametryzowane zapytania (prepared statements) zapobiegające SQL Injection

### osom-api

**Odpowiedzialność:** CLI (clap) i orkiestracja potoku przetwarzania.

**Kluczowe funkcjonalności:**
- Opcje uruchomienia (`PipelineOptions`): obsługa `--dry-run` oraz `--output`
- Konfiguracja poziomu logowania `tracing` z poziomu `config.settings.log_level` z priorytetem dla zmiennej `RUST_LOG`

**Potok (pipeline):**
1. Odczyt pliku wejściowego (`tokio::fs::read`)
2. Detekcja typu na podstawie rozszerzenia (`.json`, `.xml`, `.csv`, `.pdf`)
3. Parsowanie przez `parse_by_source_type`
4. Budowanie promptu przez `PromptBuilder::build`
5. (Tryb dry-run: wyświetlenie promptu i zakończenie bez zapytań sieciowych)
6. Wywołanie LLM przez `build_llm_client_with_settings` (z uwzględnieniem retry i timeoutów)
7. Ekstrakcja JSON z markdown (`extract_json_from_markdown`)
8. Walidacja schematu (`validate`)
9. Formatowanie (`format_values`)
10. Zapis wyników (`JsonWriter`, `XmlWriter`, lub `DatabaseWriter`)

## Stack technologiczny

| Kategoria | Biblioteka | Wersja |
|-----------|-----------|--------|
| Serializacja | serde, serde_json | 1.0 |
| Konfiguracja | toml | 0.8 |
| XML | quick-xml | 0.36 |
| CSV | csv | 1.3 |
| PDF | pdf-extract | 0.7 |
| HTTP | reqwest | 0.12 |
| Async | tokio | 1.40 |
| CLI | clap | 4.5 |
| Baza danych | sqlx (sqlite) | 0.8 |
| Logowanie | tracing, tracing-subscriber | 0.1/0.3 |
| Błędy | thiserror, anyhow | 1.0 |
| Async traits | async-trait | 0.1 |

## Decyzje architektoniczne

1. **Workspace zamiast monolitu** – separacja domen ułatwia testowanie i ewentualną ekstrakcję crate'ów jako osobnych bibliotek
2. **Async trait z `allow(async_fn_in_trait)`** – w parserze użyto wbudowanego async w trait (stabilne w Rust 2021) bez zewnętrznego crate'u, ale w LLM użyto `async-trait` dla kompatybilności z `dyn Trait`
3. **Tekstowa reprezentacja między parserem a LLM** – wszystkie formaty konwertowane są do JSON string przed przekazaniem do modelu, co zapewnia unifikację interfejsu niezależnie od źródła
4. **Fabryka klientów LLM zamiast enum dispatch** – `Box<dyn LlmClient>` pozwala na jednolity interfejs w potoku bez matchowania na każdym kroku
5. **Markdown JSON extraction** – LLM często zwracają JSON w blokach markdown, dlatego pipeline zawiera heurystykę do wyciągania czystego JSON
