# OsomAPI – Uniwersalne API do przetwarzania danych przez LLM

OsomAPI to aplikacja napisana w języku Rust, która umożliwia przetwarzanie danych wejściowych w różnych formatach (JSON, XML, CSV, PDF) przy użyciu modeli językowych (LLM). Aplikacja pozwala na elastyczną konfigurację źródła LLM, definicję schematu wyjściowego oraz wybór formatu i miejsca zapisu wyników.

## Główne funkcjonalności

- **Wielofunkcyjne parsowanie** – obsługa plików JSON, XML, CSV oraz PDF
- **Elastyczna konfiguracja LLM** – wybór między lokalnym modelem (Ollama) a chmurowymi dostawcami (OpenAI GPT, Google Gemini, Anthropic Claude, GitHub Copilot)
- **Definiowalny schemat wyjściowy** – precyzyjna kontrola nad strukturą, typami danych, formatami dat i liczb dziesiętnych oraz normalizacją wielkości liter
- **Wiele formatów zapisu** – eksport do JSON, XML lub bazy danych SQLite
- **Asynchroniczna architektura** – oparta na Tokio dla wysokiej wydajności
- **Kompleksowe logowanie** – oparte na `tracing` dla łatwego monitorowania i debugowania

## Szybki start

### Wymagania

- [Rust](https://rustup.rs/) (najnowsza wersja stabilna)
- Dla lokalnych modeli: [Ollama](https://ollama.com/) zainstalowana i uruchomiona
- Dla modeli chmurowych: odpowiedni klucz API

### Instalacja

```bash
git clone <repo-url>
cd osom-api
cargo build --release
```

### Pierwsze uruchomienie

1. Utwórz plik konfiguracyjny `config.toml` (zobacz [Przykładowa konfiguracja](#przykładowa-konfiguracja))
2. Przygotuj plik wejściowy (np. `dane.csv`)
3. Uruchom aplikację:

```bash
./target/release/osom-api --input dane.csv --config config.toml
```

## Przykładowa konfiguracja

```toml
[llm]
provider = "ollama"
model = "llama3"
url = "http://localhost:11434"

[output]
[output.schema]
name = "faktury"
case_normalization = "lower"

[[output.schema.fields]]
name = "numer_faktury"
type = "string"
required = true
description = "Numer identyfikacyjny faktury"

[[output.schema.fields]]
name = "data_wystawienia"
type = { type = "date", format = "%Y-%m-%d" }
required = true
description = "Data wystawienia faktury"

[[output.schema.fields]]
name = "kwota_brutto"
type = { type = "decimal", precision = 19, scale = 2 }
required = true
description = "Kwota brutto faktury"

[[output.schema.fields]]
name = "kontrahent"
type = { type = "object", fields = [
    { name = "nazwa", type = "string", required = true },
    { name = "nip", type = "string", required = true }
]}
required = true

[output.destination]
type = "json"
path = "wyniki.json"
pretty = true
```

## Struktura projektu

```
osom-api/
├── Cargo.toml              # Workspace root
├── osom-api/               # Główna aplikacja (CLI + pipeline)
├── osom-config/            # Moduł konfiguracji (TOML/JSON)
├── osom-parser/            # Moduł parserów (JSON, XML, CSV, PDF)
├── osom-llm/               # Moduł klientów LLM (5 dostawców)
├── osom-schema/            # Moduł formatowania i walidacji schematu
├── osom-writer/            # Moduł zapisu (JSON, XML, SQLite)
└── docs/                   # Dokumentacja
```

## Dokumentacja

- [Architektura](docs/ARCHITECTURE.md) – szczegóły techniczne i decyzje architektoniczne
- [Konfiguracja](docs/CONFIG.md) – pełny opis pliku konfiguracyjnego
- [Użycie](docs/USAGE.md) – instrukcja użytkownika i przykłady

## Licencja

MIT
