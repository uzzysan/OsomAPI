use osom_config::{Config, Destination};
use osom_llm::client::build_llm_client_with_settings;
use osom_llm::prompt::PromptBuilder;
use osom_parser::parse_auto;
use osom_schema::formatter::{format_values, FormattedData};
use osom_schema::validator::validate;
use osom_writer::writer::{DatabaseWriter, JsonWriter, XmlWriter};
use std::path::Path;
use thiserror::Error;
use tracing::{info, warn};

/// Błąd potoku przetwarzania.
#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("Błąd konfiguracji: {0}")]
    Config(String),
    #[error("Błąd parsowania: {0}")]
    Parser(String),
    #[error("Błąd LLM: {0}")]
    Llm(String),
    #[error("Błąd walidacji schematu: {0}")]
    Validation(String),
    #[error("Błąd formatowania: {0}")]
    Formatting(String),
    #[error("Błąd zapisu: {0}")]
    Writer(String),
    #[error("Nieobsługiwane rozszerzenie pliku: {0}")]
    UnsupportedExtension(String),
    #[error("Brak danych wejściowych")]
    EmptyInput,
}

/// Opcje uruchomienia potoku przetwarzania.
#[derive(Debug, Clone, Default)]
pub struct PipelineOptions {
    /// Czy uruchomić w trybie dry-run (wypisanie promptu bez wywołania LLM)
    pub dry_run: bool,
    /// Opcjonalne nadpisanie ścieżki pliku docelowego
    pub output_override: Option<String>,
    /// Opcjonalny filtr rozszerzenia / wzorca przy przetwarzaniu katalogu
    pub pattern: Option<String>,
}

/// Wykonuje pełny potok przetwarzania z domyślnymi opcjami.
#[allow(dead_code)]
pub async fn run_pipeline(config: &Config, input_path: &str) -> Result<(), PipelineError> {
    run_pipeline_with_options(config, input_path, &PipelineOptions::default()).await
}

/// Wykonuje pełny potok przetwarzania: parse → LLM → validate → format → write.
/// Obsługuje zarówno pojedynczy plik, jak i cały katalog w trybie wsadowym (batch).
pub async fn run_pipeline_with_options(
    config: &Config,
    input_path: &str,
    options: &PipelineOptions,
) -> Result<(), PipelineError> {
    let p = Path::new(input_path);
    if !p.exists() {
        return Err(PipelineError::Config(format!(
            "Ścieżka wejściowa '{}' nie istnieje",
            input_path
        )));
    }

    if p.is_dir() {
        run_batch_pipeline(config, p, options).await
    } else {
        run_single_pipeline(config, p, options).await
    }
}

async fn run_single_pipeline(
    config: &Config,
    file_path: &Path,
    options: &PipelineOptions,
) -> Result<(), PipelineError> {
    let display_name = file_path.display().to_string();
    info!("Rozpoczynanie przetwarzania pliku: {}", display_name);

    let input_bytes = tokio::fs::read(file_path)
        .await
        .map_err(|e| PipelineError::Config(format!("Nie można odczytać pliku: {}", e)))?;

    if input_bytes.is_empty() {
        return Err(PipelineError::EmptyInput);
    }

    let ext = file_path.extension().and_then(|e| e.to_str());

    info!("Parsowanie danych wejściowych...");
    let parsed = parse_auto(&input_bytes, ext)
        .await
        .map_err(|e| PipelineError::Parser(e.to_string()))?;

    info!("Budowanie promptu LLM...");
    let prompt = PromptBuilder::build(&parsed.content, &config.output.schema);

    if options.dry_run {
        info!("--- TRYB DRY-RUN (wygenerowany prompt) ---");
        println!("{}", prompt);
        return Ok(());
    }

    info!("Wysyłanie zapytania do modelu LLM...");
    let client = build_llm_client_with_settings(&config.llm, &config.settings)
        .map_err(|e| PipelineError::Llm(e.to_string()))?;
    let llm_response = client
        .send(&prompt)
        .await
        .map_err(|e| PipelineError::Llm(e.to_string()))?;

    info!("Parsowanie odpowiedzi LLM...");
    let raw_json = extract_json_from_markdown(&llm_response)
        .map_err(|e| PipelineError::Llm(format!("Nie można sparsować odpowiedzi jako JSON: {}", e)))?;

    info!("Walidacja zgodności ze schematem...");
    validate(&raw_json, &config.output.schema)
        .map_err(|e| PipelineError::Validation(e.to_string()))?;

    info!("Formatowanie danych wyjściowych...");
    let formatted = format_values(&raw_json, &config.output.schema)
        .map_err(|e| PipelineError::Formatting(e.to_string()))?;

    if formatted.records.is_empty() {
        warn!("Ostrzeżenie: brak rekordów do zapisu");
    }

    info!("Zapisywanie wyników...");
    let destination = resolve_destination(config, options);
    write_destination(&destination, &formatted).await?;

    info!("Potok zakończony pomyślnie.");
    Ok(())
}

async fn run_batch_pipeline(
    config: &Config,
    dir_path: &Path,
    options: &PipelineOptions,
) -> Result<(), PipelineError> {
    info!(
        "Rozpoczynanie przetwarzania wsadowego katalogu: {}",
        dir_path.display()
    );

    let mut entries = tokio::fs::read_dir(dir_path)
        .await
        .map_err(|e| PipelineError::Config(format!("Nie można odczytać katalogu: {}", e)))?;

    let mut files = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| PipelineError::Config(format!("Błąd iteracji katalogu: {}", e)))?
    {
        let file_type = entry
            .file_type()
            .await
            .map_err(|e| PipelineError::Config(format!("Błąd sprawdzania typu pliku: {}", e)))?;
        if file_type.is_file() {
            let path = entry.path();
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if file_name.starts_with('.') {
                continue;
            }
            if let Some(ref pat) = options.pattern {
                let pat_clean = pat.trim_start_matches('*').trim_start_matches('.');
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or_default();
                if !ext.eq_ignore_ascii_case(pat_clean) && !file_name.contains(pat_clean) {
                    continue;
                }
            }
            files.push(path);
        }
    }

    files.sort();

    if files.is_empty() {
        return Err(PipelineError::Config(format!(
            "Brak pasujących plików do przetworzenia w katalogu: {:?}",
            dir_path
        )));
    }

    info!(
        "Znaleziono {} plików do przetworzenia w trybie wsadowym.",
        files.len()
    );
    let mut all_records = Vec::new();

    for file_path in &files {
        let display_name = file_path.display().to_string();
        info!("--- Przetwarzanie wsadowe pliku: {} ---", display_name);
        let input_bytes = tokio::fs::read(file_path)
            .await
            .map_err(|e| PipelineError::Config(format!("Nie można odczytać pliku {}: {}", display_name, e)))?;
        if input_bytes.is_empty() {
            warn!("Pominięcie pustego pliku: {}", display_name);
            continue;
        }

        let ext = file_path.extension().and_then(|e| e.to_str());
        let parsed = parse_auto(&input_bytes, ext)
            .await
            .map_err(|e| PipelineError::Parser(format!("{}: {}", display_name, e)))?;

        let prompt = PromptBuilder::build(&parsed.content, &config.output.schema);
        if options.dry_run {
            info!("--- DRY-RUN [{}] ---", display_name);
            println!("=== Plik: {} ===\n{}\n", display_name, prompt);
            continue;
        }

        let client = build_llm_client_with_settings(&config.llm, &config.settings)
            .map_err(|e| PipelineError::Llm(e.to_string()))?;
        let llm_response = client
            .send(&prompt)
            .await
            .map_err(|e| PipelineError::Llm(format!("{}: {}", display_name, e)))?;

        let raw_json = extract_json_from_markdown(&llm_response)
            .map_err(|e| PipelineError::Llm(format!("{}: {}", display_name, e)))?;

        validate(&raw_json, &config.output.schema)
            .map_err(|e| PipelineError::Validation(format!("{}: {}", display_name, e)))?;

        let formatted = format_values(&raw_json, &config.output.schema)
            .map_err(|e| PipelineError::Formatting(format!("{}: {}", display_name, e)))?;

        all_records.extend(formatted.records);
    }

    if options.dry_run {
        return Ok(());
    }

    let total_records = all_records.len();
    let final_data = FormattedData {
        records: all_records,
    };
    let destination = resolve_destination(config, options);
    write_destination(&destination, &final_data).await?;

    info!(
        "Potok wsadowy zakończony pomyślnie. Zapisano łącznie {} rekordów z {} plików.",
        total_records,
        files.len()
    );
    Ok(())
}

fn resolve_destination(config: &Config, options: &PipelineOptions) -> Destination {
    match (&options.output_override, &config.output.destination) {
        (Some(override_path), Destination::Json { pretty, .. }) => Destination::Json {
            path: override_path.clone(),
            pretty: *pretty,
        },
        (Some(override_path), Destination::Xml { pretty, .. }) => Destination::Xml {
            path: override_path.clone(),
            pretty: *pretty,
        },
        (Some(override_path), Destination::Database { table_name, .. }) => Destination::Database {
            connection: osom_config::DbConnection::Sqlite {
                path: override_path.clone(),
            },
            table_name: table_name.clone(),
        },
        _ => config.output.destination.clone(),
    }
}

async fn write_destination(
    destination: &Destination,
    data: &FormattedData,
) -> Result<(), PipelineError> {
    match destination {
        Destination::Json { path, pretty } => {
            let writer = JsonWriter::new(path.clone(), *pretty);
            writer
                .write(data)
                .await
                .map_err(|e| PipelineError::Writer(e.to_string()))?;
        }
        Destination::Xml { path, pretty } => {
            let writer = XmlWriter::new(path.clone(), *pretty);
            writer
                .write(data)
                .await
                .map_err(|e| PipelineError::Writer(e.to_string()))?;
        }
        Destination::Database {
            connection,
            table_name,
        } => {
            let writer = DatabaseWriter::new(connection.clone(), table_name.clone());
            writer
                .write(data)
                .await
                .map_err(|e| PipelineError::Writer(e.to_string()))?;
        }
    }
    Ok(())
}

/// Próbuje wyciągnąć JSON z bloku markdown lub zwraca sparsowany JSON.
fn extract_json_from_markdown(text: &str) -> Result<serde_json::Value, serde_json::Error> {
    // Szukaj bloku ```json ... ```
    if let Some(start) = text.find("```json") {
        let rest = &text[start + 7..];
        if let Some(end) = rest.find("```") {
            let json_str = rest[..end].trim();
            return serde_json::from_str(json_str);
        }
    }
    // Szukaj bloku ``` ... ```
    if let Some(start) = text.find("```") {
        let rest = &text[start + 3..];
        if let Some(end) = rest.find("```") {
            let json_str = rest[..end].trim();
            return serde_json::from_str(json_str);
        }
    }
    // Bezpośrednia próba parsowania
    serde_json::from_str(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_from_markdown_code_block() {
        let markdown = r#"Oto wynik:
```json
{"name": "test", "value": 42}
```
"#;
        let result = extract_json_from_markdown(markdown).unwrap();
        assert_eq!(result["name"], "test");
        assert_eq!(result["value"], 42);
    }

    #[test]
    fn test_extract_json_plain() {
        let json = r#"{"name": "test"}"#;
        let result = extract_json_from_markdown(json).unwrap();
        assert_eq!(result["name"], "test");
    }

    #[test]
    fn test_extract_json_array_from_markdown() {
        let markdown = r#"```json
[{"id": 1}, {"id": 2}]
```"#;
        let result = extract_json_from_markdown(markdown).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn test_extract_json_no_json_block() {
        let text = "To nie jest JSON";
        assert!(extract_json_from_markdown(text).is_err());
    }

    #[tokio::test]
    async fn test_pipeline_dry_run_success() {
        let config_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../config.example.toml");
        let input_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../sample_input.csv");
        let config = Config::from_toml_file(config_path).expect("Failed to load config");

        let options = PipelineOptions {
            dry_run: true,
            output_override: None,
            pattern: None,
        };

        let result = run_pipeline_with_options(&config, input_path, &options).await;
        assert!(result.is_ok());
    }
}
