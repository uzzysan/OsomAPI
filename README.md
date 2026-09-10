# OsomAPI – Universal API for LLM-Powered Data Processing

OsomAPI is a Rust-based application that enables data processing across multiple input formats (JSON, XML, CSV, PDF) using Large Language Models (LLMs). The application offers flexible LLM provider configuration, customizable output schema definitions, and versatile output formatting and storage options.

## Key Features

- **Multi-format Parsing** – Native support for JSON, XML, CSV, and PDF files with automatic format & magic-byte detection
- **Multi-Endpoint API Server & Web UI** – Built-in Axum HTTP server and responsive browser dashboard with visual schema building, input field inspection, and live testing
- **Flexible LLM Configuration** – Choose between local models (Ollama) and cloud providers (OpenAI GPT, Google Gemini, Anthropic Claude, GitHub Copilot) with exponential backoff retries
- **Customizable Output Schema** – Fine-grained control over structure, data types, date/decimal formats, and case normalization
- **Multiple Output Destinations** – Export to JSON, XML, SQLite, PostgreSQL, or MySQL databases
- **Batch Processing** – Process entire directories of incoming documents into a single consolidated output
- **Comprehensive Logging** – Powered by `tracing` for seamless observability and debugging

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable version)
- For local models: [Ollama](https://ollama.com/) installed and running
- For cloud models: A valid API key for the chosen provider

### Installation

```bash
git clone <repo-url>
cd osom-api
cargo build --release
```

### Running the Web UI & API Server

Launch the standalone HTTP server with embedded web dashboard:
```bash
cargo run --bin osom-api -- --serve
```
Then open [http://localhost:8080](http://localhost:8080) in your browser to visually manage endpoints, inspect sample files, build schemas, and test prompts.

### CLI Mode (Batch or Single File)

Process a file directly through the CLI:
```bash
cargo run --bin osom-api -- --input sample_input.csv --config config.toml
```

Process a directory of files in batch mode:
```bash
cargo run --bin osom-api -- --input ./incoming/ --config config.toml -p "*.csv" -o results.json
```

### Useful CLI Options

- `--serve`: Run the HTTP REST API server and embedded Web UI dashboard (default if `--input` omitted).
- `--host <HOST>`: Server bind host (default: `0.0.0.0`).
- `--port <PORT>`: Server listen port (default: `8080`).
- `--dry-run`: View the generated prompt without calling the LLM.
- `-o, --output <path>`: Override destination file path defined in configuration.
- `-p, --pattern <pattern>`: Filter files when scanning a directory (e.g. `*.csv`, `*.pdf`).

## Example Configuration

```toml
[llm]
provider = "ollama"
model = "llama3"
url = "http://localhost:11434"

[output]
[output.schema]
name = "invoices"
case_normalization = "lower"

[[output.schema.fields]]
name = "invoice_number"
type = "string"
required = true
description = "Invoice identification number"

[[output.schema.fields]]
name = "issue_date"
type = { type = "date", format = "%Y-%m-%d" }
required = true
description = "Invoice issue date"

[[output.schema.fields]]
name = "total_amount"
type = { type = "decimal", precision = 19, scale = 2 }
required = true
description = "Invoice total gross amount"

[[output.schema.fields]]
name = "contractor"
type = { type = "object", fields = [
    { name = "name", type = "string", required = true },
    { name = "tax_id", type = "string", required = true }
]}
required = true

[output.destination]
type = "json"
path = "results.json"
pretty = true

[settings]
request_timeout_secs = 120
max_retries = 3
log_level = "info"
```

## Project Structure

```
osom-api/
├── Cargo.toml              # Workspace root
├── osom-api/               # Main application binary (CLI + processing pipeline)
├── osom-config/            # Configuration management (TOML/JSON loading and validation)
├── osom-parser/            # Multi-format input parsers (JSON, XML, CSV, PDF)
├── osom-llm/               # LLM clients (Ollama, OpenAI, Gemini, Claude, Copilot)
├── osom-schema/            # Output schema definition, formatting, and validation
├── osom-writer/            # Output writers (JSON, XML, SQLite)
└── docs/                   # Detailed documentation
```

## Documentation

- [Architecture](docs/ARCHITECTURE.md) – Technical overview and architecture decisions
- [Configuration](docs/CONFIG.md) – Comprehensive configuration reference
- [Usage Guide](docs/USAGE.md) – User guide and execution examples

## License

MIT
