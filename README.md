# OsomAPI – Universal API for LLM-Powered Data Processing

OsomAPI is a Rust-based application that enables data processing across multiple input formats (JSON, XML, CSV, PDF) using Large Language Models (LLMs). The application offers flexible LLM provider configuration, customizable output schema definitions, and versatile output formatting and storage options.

## Key Features

- **Multi-format Parsing** – Native support for JSON, XML, CSV, and PDF files
- **Flexible LLM Configuration** – Choose between local models (Ollama) and cloud providers (OpenAI GPT, Google Gemini, Anthropic Claude, GitHub Copilot)
- **Customizable Output Schema** – Fine-grained control over structure, data types, date/decimal formats, and case normalization
- **Multiple Output Destinations** – Export to JSON, XML, or SQLite database
- **Asynchronous Architecture** – Built on Tokio for high-throughput performance
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

### First Run

1. Create a configuration file `config.toml` (see [Example Configuration](#example-configuration) or copy from `config.example.toml`):
   ```bash
   cp config.example.toml config.toml
   ```
2. Prepare an input file (e.g. `sample_input.csv`).
3. Run the application:
   ```bash
   cargo run --bin osom-api -- --input sample_input.csv --config config.toml
   ```
   Or using the compiled binary:
   ```bash
   ./target/release/osom-api --input sample_input.csv --config config.toml
   ```

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
