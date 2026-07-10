# Plan Budowy Aplikacji OsomAPI

## Cel
Uniwersalne API w języku Rust do przetwarzania danych z różnych źródeł (JSON, XML, CSV, PDF) przy użyciu modeli LLM (lokalnych przez Ollama lub chmurowych: Gemini, GPT, Claude, Copilot), z możliwością definiowania schematu wyjściowego i zapisu w formacie JSON, XML lub do bazy danych.

## Architektura

### Struktura workspace Cargo
```
osom-api/
├── Cargo.toml                 # workspace root
├── osom-api/                  # główna aplikacja (binary)
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
├── osom-config/               # moduł konfiguracji
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
├── osom-parser/               # moduł parserów (JSON, XML, CSV, PDF)
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
├── osom-llm/                  # moduł klientów LLM
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
├── osom-schema/               # moduł schematu/formatowania danych wyjściowych
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
├── osom-writer/               # moduł zapisu danych wyjściowych
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
└── docs/
    ├── ARCHITECTURE.md
    ├── CONFIG.md
    └── USAGE.md
```

### Moduły i ich odpowiedzialności

#### 1. osom-config
- Parsowanie pliku konfiguracyjnego (TOML/JSON)
- Definicja struktury konfiguracji: źródło LLM, API key, schemat wyjściowy, destination
- Walidacja konfiguracji

#### 2. osom-parser
- Trait `InputParser` z metodą `parse(input: &[u8]) -> Result<ParsedData, Error>`
- Implementacje: `JsonParser`, `XmlParser`, `CsvParser`, `PdfParser`
- `ParsedData` – znormalizowana reprezentacja tekstowa do przekazania do LLM

#### 3. osom-llm
- Trait `LlmClient` z metodą `send(prompt: &str) -> Result<String, Error>`
- Implementacje:
  - `OllamaClient` (lokalny, HTTP do Ollama API)
  - `GeminiClient` (Google Gemini API)
  - `OpenAiClient` (GPT/ChatGPT)
  - `AnthropicClient` (Claude)
  - `CopilotClient` (GitHub Copilot)
- Prompt builder – konstrukcja promptu z danymi wejściowymi i schematem wyjściowym

#### 4. osom-schema
- Definicja `OutputSchema` – pola, typy, formaty dat, formaty liczb, wielkość liter
- `ValueFormatter` – formatowanie wartości zgodnie ze schematem
- `SchemaValidator` – walidacja surowej odpowiedzi LLM pod kątem schematu
- Konwersja odpowiedzi LLM (zazwyczaj JSON) do znormalizowanej postaci

#### 5. osom-writer
- Trait `OutputWriter` z metodą `write(data: &FormattedData) -> Result<(), Error>`
- Implementacje:
  - `JsonWriter`
  - `XmlWriter`
  - `DatabaseWriter` (SQLite jako domyślna, opcjonalnie PostgreSQL/MySQL)

#### 6. osom-api (main)
- Inicjalizacja konfiguracji
- Wybór parsera na podstawie rozszerzenia pliku wejściowego lub nagłówka MIME
- Wybór klienta LLM na podstawie konfiguracji
- Pipeline: parse → build prompt → LLM call → format → write
- CLI (clap) – argumenty: ścieżka do pliku wejściowego, ścieżka do configu

## Stack Technologiczny
- **serde** + **serde_json** – serializacja/deserializacja
- **toml** – plik konfiguracyjny
- **quick-xml** – parsowanie XML
- **csv** – parsowanie CSV
- **lopdf** lub **pdf-extract** – ekstrakcja tekstu z PDF
- **reqwest** – HTTP client dla API LLM
- **tokio** – async runtime
- **clap** – CLI
- **sqlx** lub **rusqlite** – baza danych
- **thiserror** / **anyhow** – obsługa błędów
- **tracing** – logowanie

## Etapy realizacji

### Etap 1 – Scaffold
- Inicjalizacja repo git
- Utworzenie workspace Cargo z 5 crate'ami
- Ustalenie wspólnych zależności

### Etap 2 – Implementacja modułów (równolegle)
- **Worker 1**: osom-config + osom-schema (zależność schema od config)
- **Worker 2**: osom-parser (JSON, XML, CSV, PDF)
- **Worker 3**: osom-llm (wszyscy klienci LLM)
- **Worker 4**: osom-writer (JSON, XML, DB)

### Etap 3 – Integracja
- osom-api main.rs – połączenie wszystkich modułów
- CLI, pipeline, error handling

### Etap 4 – Dokumentacja i testy
- README, ARCHITECTURE.md, CONFIG.md, USAGE.md
- Testy jednostkowe modułów
- Przykładowy plik konfiguracyjny

## Kontrakt między modułami

### osom-config → osom-schema
```rust
pub struct Config {
    pub llm: LlmConfig,
    pub output: OutputConfig,
}

pub enum LlmConfig {
    Ollama { model: String, url: String },
    Gemini { api_key: String, model: String },
    OpenAi { api_key: String, model: String },
    Anthropic { api_key: String, model: String },
    Copilot { api_key: String, model: String },
}

pub struct OutputConfig {
    pub schema: OutputSchema,
    pub destination: Destination,
}
```

### osom-parser → osom-llm
```rust
pub struct ParsedData {
    pub content: String,        // tekst do przekazania do LLM
    pub source_type: SourceType,
}
```

### osom-llm → osom-schema
```rust
// LLM zwraca String (JSON) który schema waliduje i formatuje
```

### osom-schema → osom-writer
```rust
pub struct FormattedData {
    pub records: Vec<Map<String, Value>>,
}
```
