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