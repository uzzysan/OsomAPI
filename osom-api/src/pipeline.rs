use osom_config::{Config, Destination};
use osom_llm::client::build_llm_client;
use osom_llm::prompt::PromptBuilder;
use osom_parser::{SourceType, parse_by_source_type};
use osom_schema::formatter::format_values;
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

/// Wykonuje pełny potok przetwarzania: parse → LLM → validate → format → write.
pub async fn run_pipeline(config: &Config, input_path: &str) -> Result<(), PipelineError> {
    info!("Rozpoczynanie potoku przetwarzania dla: {}", input_path);

    // 1. Odczyt pliku wejściowego
    let input_bytes = tokio::fs::read(input_path)
        .await
        .map_err(|e| PipelineError::Config(format!("Nie można odczytać pliku: {}", e)))?;

    if input_bytes.is_empty() {
        return Err(PipelineError::EmptyInput);
    }

    // 2. Wybór parsera na podstawie rozszerzenia
    let ext = Path::new(input_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let source_type = match ext.as_str() {
        "json" => SourceType::Json,
        "xml" => SourceType::Xml,
        "csv" => SourceType::Csv,
        "pdf" => SourceType::Pdf,
        _ => {
            return Err(PipelineError::UnsupportedExtension(ext.to_string()));
        }
    };

    info!("Parsowanie danych wejściowych (typ: {})...", ext);
    let parsed = parse_by_source_type(source_type, &input_bytes)
        .await
        .map_err(|e| PipelineError::Parser(e.to_string()))?;

    // 3. Budowanie promptu
    info!("Budowanie promptu LLM...");
    let prompt = PromptBuilder::build(&parsed.content, &config.output.schema);

    // 4. Wywołanie LLM
    info!("Wysyłanie zapytania do modelu LLM...");
    let client = build_llm_client(&config.llm)
        .map_err(|e| PipelineError::Llm(e.to_string()))?;
    let llm_response = client
        .send(&prompt)
        .await
        .map_err(|e| PipelineError::Llm(e.to_string()))?;

    // 5. Parsowanie odpowiedzi LLM jako JSON
    info!("Parsowanie odpowiedzi LLM...");
    let raw_json = extract_json_from_markdown(&llm_response)
        .map_err(|e| PipelineError::Llm(format!("Nie można sparsować odpowiedzi jako JSON: {}", e)))?;

    // 6. Walidacja schematu
    info!("Walidacja zgodności ze schematem...");
    validate(&raw_json, &config.output.schema)
        .map_err(|e| PipelineError::Validation(e.to_string()))?;

    // 7. Formatowanie wartości
    info!("Formatowanie danych wyjściowych...");
    let formatted = format_values(&raw_json, &config.output.schema)
        .map_err(|e| PipelineError::Formatting(e.to_string()))?;

    if formatted.records.is_empty() {
        warn!("Ostrzeżenie: brak rekordów do zapisu");
    }

    // 8. Zapis wyników
    info!("Zapisywanie wyników...");
    match &config.output.destination {
        Destination::Json { path, pretty } => {
            let writer = JsonWriter::new(path.clone(), *pretty);
            writer
                .write(&formatted)
                .await
                .map_err(|e| PipelineError::Writer(e.to_string()))?;
        }
        Destination::Xml { path, pretty } => {
            let writer = XmlWriter::new(path.clone(), *pretty);
            writer
                .write(&formatted)
                .await
                .map_err(|e| PipelineError::Writer(e.to_string()))?;
        }
        Destination::Database {
            connection,
            table_name,
        } => match connection {
            osom_config::DbConnection::Sqlite { path } => {
                let writer = DatabaseWriter::new_sqlite(path.clone(), table_name.clone());
                writer
                    .write(&formatted)
                    .await
                    .map_err(|e| PipelineError::Writer(e.to_string()))?;
            }
            _ => {
                return Err(PipelineError::Writer(
                    "W tej wersji obsługiwany jest tylko SQLite".to_string(),
                ));
            }
        },
    }

    info!("Potok zakończony pomyślnie.");
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
}
