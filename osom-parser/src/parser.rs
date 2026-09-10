//! Moduł parserów dla OsomAPI.
//!
//! Dostarcza trait [`InputParser`] oraz konkretne implementacje
//! dla formatów JSON, XML, CSV i PDF.
//! Wynikiem parsowania jest znormalizowana reprezentacja tekstowa
//! ([`ParsedData`]), gotowa do przekazania do modułu LLM.

use serde_json::{Map, Value};
use thiserror::Error;
use tracing::{debug, instrument};

/// Typ źródła danych wejściowych.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    /// Dane w formacie JSON.
    Json,
    /// Dane w formacie XML.
    Xml,
    /// Dane w formacie CSV.
    Csv,
    /// Dokument PDF.
    Pdf,
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceType::Json => write!(f, "json"),
            SourceType::Xml => write!(f, "xml"),
            SourceType::Csv => write!(f, "csv"),
            SourceType::Pdf => write!(f, "pdf"),
        }
    }
}

/// Znormalizowane dane wyjściowe z parsera.
///
/// Zawiera treść tekstową przeznaczoną dla modelu LLM
/// oraz informację o typie źródła.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedData {
    /// Treść tekstowa do przekazania do LLM.
    pub content: String,
    /// Typ źródła, z którego pochodziły dane.
    pub source_type: SourceType,
}

/// Błędy mogące wystąpić podczas parsowania danych wejściowych.
#[derive(Debug, Error)]
pub enum ParserError {
    /// Błąd parsowania JSON.
    #[error("Błąd parsowania JSON: {0}")]
    Json(#[from] serde_json::Error),

    /// Błąd parsowania XML.
    #[error("Błąd parsowania XML: {0}")]
    Xml(String),

    /// Błąd parsowania CSV.
    #[error("Błąd parsowania CSV: {0}")]
    Csv(#[from] csv::Error),

    /// Błąd ekstrakcji tekstu z PDF.
    #[error("Błąd ekstrakcji PDF: {0}")]
    Pdf(String),

    /// Nieznany lub nieobsługiwany typ źródła.
    #[error("Nieznany lub nieobsługiwany typ źródła")]
    UnknownSource,

    /// Nieprawidłowe dane wejściowe.
    #[error("Nieprawidłowe dane wejściowe: {0}")]
    InvalidInput(String),

    /// Błąd konwersji UTF-8.
    #[error("Błąd kodowania UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
}

/// Główny trait parsera.
///
/// Wszystkie implementacje są bezpieczne do użycia współbieżnego
/// (`Send + Sync`), a metoda `parse` jest asynchroniczna.
#[allow(async_fn_in_trait)]
pub trait InputParser: Send + Sync {
    /// Parsuje surowe bajty do znormalizowanej postaci tekstowej.
    async fn parse(&self, input: &[u8]) -> Result<ParsedData, ParserError>;
}

/// Pomocnicza funkcja wybierająca parser na podstawie typu źródła.
///
/// Nie wymaga dynamicznego dispatchu (`dyn InputParser`),
/// co ułatwia użycie w potoku `osom-api`.
pub async fn parse_by_source_type(
    source_type: SourceType,
    input: &[u8],
) -> Result<ParsedData, ParserError> {
    match source_type {
        SourceType::Json => JsonParser.parse(input).await,
        SourceType::Xml => XmlParser.parse(input).await,
        SourceType::Csv => CsvParser.parse(input).await,
        SourceType::Pdf => PdfParser.parse(input).await,
    }
}

/// Automatycznie wykrywa format pliku na podstawie rozszerzenia oraz sygnatur/magicznych bajtów.
pub fn detect_source_type(
    input: &[u8],
    extension: Option<&str>,
) -> Result<SourceType, ParserError> {
    if let Some(ext) = extension {
        match ext.trim_start_matches('.').to_lowercase().as_str() {
            "json" => return Ok(SourceType::Json),
            "xml" => return Ok(SourceType::Xml),
            "csv" => return Ok(SourceType::Csv),
            "pdf" => return Ok(SourceType::Pdf),
            _ => {}
        }
    }

    // Wykrywanie sygnatury binarnej PDF
    if input.starts_with(b"%PDF-") {
        return Ok(SourceType::Pdf);
    }

    // Wykrywanie formatów tekstowych
    if let Ok(text) = std::str::from_utf8(input) {
        let trimmed = text.trim();
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            return Ok(SourceType::Json);
        }
        if trimmed.starts_with("<?xml")
            || (trimmed.starts_with('<') && trimmed.ends_with('>'))
        {
            return Ok(SourceType::Xml);
        }
        // Heurystyka CSV: obecność delimiterów i poprawny nagłówek
        if trimmed.contains(',') || trimmed.contains(';') || trimmed.contains('\t') {
            let mut reader = csv::ReaderBuilder::new()
                .has_headers(true)
                .from_reader(input);
            if let Ok(headers) = reader.headers() {
                if headers.len() > 1 {
                    return Ok(SourceType::Csv);
                }
            }
        }
    }

    Err(ParserError::UnknownSource)
}

/// Automatycznie wykrywa typ danych i parsuje je do [`ParsedData`].
pub async fn parse_auto(
    input: &[u8],
    extension: Option<&str>,
) -> Result<ParsedData, ParserError> {
    let source_type = detect_source_type(input, extension)?;
    parse_by_source_type(source_type, input).await
}

// ─────────────────────────────────────────────
// JsonParser
// ─────────────────────────────────────────────

/// Parser dla formatu JSON.
///
/// Parsuje bajty JSON do [`serde_json::Value`], a następnie
/// serializuje z powrotem do sformatowanego tekstu JSON.
#[derive(Debug, Clone, Default)]
pub struct JsonParser;

impl InputParser for JsonParser {
    #[instrument(skip(input), fields(parser = "json"))]
    async fn parse(&self, input: &[u8]) -> Result<ParsedData, ParserError> {
        debug!("Rozpoczynam parsowanie JSON");
        let value: Value = serde_json::from_slice(input)?;
        let content = serde_json::to_string_pretty(&value)?;
        debug!(dlugosc = content.len(), "Zakończono parsowanie JSON");
        Ok(ParsedData {
            content,
            source_type: SourceType::Json,
        })
    }
}

// ─────────────────────────────────────────────
// XmlParser
// ─────────────────────────────────────────────

/// Parser dla formatu XML.
///
/// Konwertuje strukturę XML do zagnieżdżonego obiektu JSON,
/// zachowując tekst węzłów, atrybuty oraz hierarchię elementów.
#[derive(Debug, Clone, Default)]
pub struct XmlParser;

/// Wewnętrzna reprezentacja węzła XML podczas parsowania.
#[derive(Debug)]
struct XmlNode {
    /// Tekst bezpośrednio wewnątrz elementu.
    text: String,
    /// Atrybuty elementu.
    attrs: Vec<(String, String)>,
    /// Dzieci elementu.
    children: Vec<(String, XmlNode)>,
}

impl XmlNode {
    /// Konwertuje węzeł do [`serde_json::Value`].
    fn to_json(&self) -> Value {
        let mut map = Map::new();

        for (k, v) in &self.attrs {
            map.insert(format!("@{k}"), Value::String(v.clone()));
        }

        if !self.text.is_empty() {
            map.insert("#text".to_string(), Value::String(self.text.trim().to_string()));
        }

        for (name, child) in &self.children {
            let child_json = child.to_json();
            if let Some(existing) = map.get_mut(name) {
                // Jeśli istnieje już element o tej samej nazwie,
                // konwertujemy go na tablicę lub dodajemy do istniejącej.
                if let Value::Array(arr) = existing {
                    arr.push(child_json);
                } else {
                    let old = map.remove(name).unwrap();
                    map.insert(name.clone(), Value::Array(vec![old, child_json]));
                }
            } else {
                map.insert(name.clone(), child_json);
            }
        }

        Value::Object(map)
    }
}

impl InputParser for XmlParser {
    #[instrument(skip(input), fields(parser = "xml"))]
    async fn parse(&self, input: &[u8]) -> Result<ParsedData, ParserError> {
        debug!("Rozpoczynam parsowanie XML");
        let value = xml_to_value(input)?;
        let content = serde_json::to_string_pretty(&value)?;
        debug!(dlugosc = content.len(), "Zakończono parsowanie XML");
        Ok(ParsedData {
            content,
            source_type: SourceType::Xml,
        })
    }
}

/// Parsuje bajty XML do [`serde_json::Value`] przy użyciu `quick-xml`.
fn xml_to_value(input: &[u8]) -> Result<Value, ParserError> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_reader(input);
    reader.config_mut().trim_text(true);

    let mut stack: Vec<(String, XmlNode)> = Vec::new();
    let mut text_buf = String::new();
    let mut root: Option<(String, XmlNode)> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attrs = Vec::new();
                for attr in e.attributes() {
                    let attr = attr.map_err(|e| ParserError::Xml(e.to_string()))?;
                    let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                    let value = attr
                        .unescape_value()
                        .unwrap_or_default()
                        .to_string();
                    attrs.push((key, value));
                }

                if !text_buf.is_empty() {
                    if let Some((_, parent)) = stack.last_mut() {
                        parent.text.push_str(&text_buf);
                    }
                    text_buf.clear();
                }

                stack.push((name, XmlNode {
                    text: String::new(),
                    attrs,
                    children: Vec::new(),
                }));
            }
            Ok(Event::Text(e)) => {
                text_buf.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::CData(e)) => {
                text_buf.push_str(&String::from_utf8_lossy(e.as_ref()));
            }
            Ok(Event::End(_)) => {
                let (name, mut node) = stack
                    .pop()
                    .ok_or_else(|| ParserError::Xml("Nieoczekiwany tag zamykający".to_string()))?;
                node.text.push_str(&text_buf);
                text_buf.clear();

                if let Some((_, parent)) = stack.last_mut() {
                    parent.children.push((name, node));
                } else {
                    root = Some((name, node));
                }
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attrs = Vec::new();
                for attr in e.attributes() {
                    let attr = attr.map_err(|e| ParserError::Xml(e.to_string()))?;
                    let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                    let value = attr
                        .unescape_value()
                        .unwrap_or_default()
                        .to_string();
                    attrs.push((key, value));
                }
                let node = XmlNode {
                    text: String::new(),
                    attrs,
                    children: Vec::new(),
                };

                if let Some((_, parent)) = stack.last_mut() {
                    parent.children.push((name, node));
                } else {
                    root = Some((name, node));
                }
            }
            Ok(Event::Decl(_))
            | Ok(Event::Comment(_))
            | Ok(Event::PI(_))
            | Ok(Event::DocType(_)) => {}
            Ok(Event::Eof) => break,
            Err(e) => return Err(ParserError::Xml(e.to_string())),
        }
    }

    if !stack.is_empty() {
        return Err(ParserError::Xml(
            "Nieprawidłowa struktura XML: niezamknięte tagi".to_string(),
        ));
    }

    if let Some((name, node)) = root {
        let mut map = Map::new();
        map.insert(name, node.to_json());
        Ok(Value::Object(map))
    } else {
        Ok(Value::Null)
    }
}

// ─────────────────────────────────────────────
// CsvParser
// ─────────────────────────────────────────────

/// Parser dla formatu CSV.
///
/// Zakłada, że pierwszy wiersz zawiera nagłówki.
/// Dane konwertowane są do tablicy obiektów JSON.
#[derive(Debug, Clone, Default)]
pub struct CsvParser;

impl InputParser for CsvParser {
    #[instrument(skip(input), fields(parser = "csv"))]
    async fn parse(&self, input: &[u8]) -> Result<ParsedData, ParserError> {
        debug!("Rozpoczynam parsowanie CSV");

        let mut reader = csv::Reader::from_reader(input);
        let headers = reader
            .headers()?
            .iter()
            .map(|h| h.to_string())
            .collect::<Vec<_>>();

        if headers.is_empty() {
            return Err(ParserError::InvalidInput(
                "Plik CSV nie zawiera nagłówków".to_string(),
            ));
        }

        let mut records = Vec::new();
        for result in reader.records() {
            let record = result?;
            let mut map = Map::new();
            for (header, value) in headers.iter().zip(record.iter()) {
                map.insert(header.clone(), Value::String(value.to_string()));
            }
            records.push(Value::Object(map));
        }

        let row_count = records.len();
        let value = Value::Array(records);
        let content = serde_json::to_string_pretty(&value)?;
        debug!(dlugosc = content.len(), wiersze = row_count, "Zakończono parsowanie CSV");
        Ok(ParsedData {
            content,
            source_type: SourceType::Csv,
        })
    }
}

// ─────────────────────────────────────────────
// PdfParser
// ─────────────────────────────────────────────

/// Parser dla dokumentów PDF.
///
/// Wyodrębnia tekst z dokumentu PDF przy użyciu crate'u `pdf-extract`.
#[derive(Debug, Clone, Default)]
pub struct PdfParser;

impl InputParser for PdfParser {
    #[instrument(skip(input), fields(parser = "pdf"))]
    async fn parse(&self, input: &[u8]) -> Result<ParsedData, ParserError> {
        debug!("Rozpoczynam ekstrakcję tekstu z PDF");
        let content = extract_pdf_text(input)?;
        debug!(dlugosc = content.len(), "Zakończono ekstrakcję PDF");
        Ok(ParsedData {
            content,
            source_type: SourceType::Pdf,
        })
    }
}

/// Wyodrębnia tekst z bajtów PDF.
fn extract_pdf_text(input: &[u8]) -> Result<String, ParserError> {
    // pdf-extract w wersji 0.7 udostępnia funkcję `extract_text_from_mem`,
    // która bezpośrednio przyjmuje bufor bajtów.
    pdf_extract::extract_text_from_mem(input)
        .map_err(|e| ParserError::Pdf(format!("{e:?}")))
}

// ─────────────────────────────────────────────
// Testy jednostkowe
// ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ─── JsonParser ───

    #[tokio::test]
    async fn json_parser_parses_valid_json() {
        let input = br#"{"name": "Alice", "age": 30}"#;
        let parser = JsonParser;
        let result = parser.parse(input).await.unwrap();

        assert_eq!(result.source_type, SourceType::Json);
        assert!(result.content.contains("Alice"));
        assert!(result.content.contains("30"));
    }

    #[tokio::test]
    async fn json_parser_returns_error_for_invalid_json() {
        let input = b"not json at all";
        let parser = JsonParser;
        let result = parser.parse(input).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ParserError::Json(_)));
    }

    #[tokio::test]
    async fn json_parser_preserves_structure() {
        let input = br#"{"items": [1, 2, 3], "nested": {"key": "val"}}"#;
        let parser = JsonParser;
        let result = parser.parse(input).await.unwrap();

        let parsed_back: Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(parsed_back["items"][1], 2);
        assert_eq!(parsed_back["nested"]["key"], "val");
    }

    // ─── XmlParser ───

    #[tokio::test]
    async fn xml_parser_parses_simple_xml() {
        let input = br#"<root><name>Alice</name><age>30</age></root>"#;
        let parser = XmlParser;
        let result = parser.parse(input).await.unwrap();

        assert_eq!(result.source_type, SourceType::Xml);
        assert!(result.content.contains("Alice"));
        assert!(result.content.contains("30"));
    }

    #[tokio::test]
    async fn xml_parser_preserves_attributes() {
        let input = br#"<root><item id="1" active="true">Text</item></root>"#;
        let parser = XmlParser;
        let result = parser.parse(input).await.unwrap();

        assert!(result.content.contains("@id"));
        assert!(result.content.contains("1"));
    }

    #[tokio::test]
    async fn xml_parser_handles_repeated_elements_as_array() {
        let input = br#"<root><item>a</item><item>b</item></root>"#;
        let parser = XmlParser;
        let result = parser.parse(input).await.unwrap();

        let parsed: Value = serde_json::from_str(&result.content).unwrap();
        let items = &parsed["root"]["item"];
        assert!(items.is_array());
        assert_eq!(items.as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn xml_parser_returns_error_for_malformed_xml() {
        let input = b"<root><unclosed>";
        let parser = XmlParser;
        let result = parser.parse(input).await;
        assert!(result.is_err());
    }

    // ─── CsvParser ───

    #[tokio::test]
    async fn csv_parser_parses_simple_csv() {
        let input = b"name,age\nAlice,30\nBob,25\n";
        let parser = CsvParser;
        let result = parser.parse(input).await.unwrap();

        assert_eq!(result.source_type, SourceType::Csv);
        assert!(result.content.contains("Alice"));
        assert!(result.content.contains("Bob"));
    }

    #[tokio::test]
    async fn csv_parser_produces_json_array() {
        let input = b"a,b\n1,2\n3,4\n";
        let parser = CsvParser;
        let result = parser.parse(input).await.unwrap();

        let parsed: Value = serde_json::from_str(&result.content).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["a"], "1");
        assert_eq!(arr[1]["b"], "4");
    }

    #[tokio::test]
    async fn csv_parser_returns_error_when_empty() {
        let input = b"";
        let parser = CsvParser;
        let result = parser.parse(input).await;
        assert!(result.is_err());
    }

    // ─── PdfParser ───

    #[tokio::test]
    async fn pdf_parser_handles_empty_pdf() {
        // Minimalny nieprawidłowy PDF – parser powinien zwrócić błąd
        // lub pusty tekst w zależności od zachowania crate'u.
        let input = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\n";
        let parser = PdfParser;
        let result = parser.parse(input).await;

        // Oczekujemy błędu, ponieważ dane nie są pełnym dokumentem PDF
        // zawierającym strony z tekstem.
        assert!(result.is_err());
    }

    // ─── parse_by_source_type ───

    #[tokio::test]
    async fn helper_dispatches_to_correct_parser() {
        let json = br#"{"ok": true}"#;
        let result = parse_by_source_type(SourceType::Json, json).await.unwrap();
        assert_eq!(result.source_type, SourceType::Json);

        let xml = br#"<root/>"#;
        let result = parse_by_source_type(SourceType::Xml, xml).await.unwrap();
        assert_eq!(result.source_type, SourceType::Xml);
    }

    #[tokio::test]
    async fn test_detect_source_type_by_extension() {
        assert_eq!(
            detect_source_type(b"", Some("csv")).unwrap(),
            SourceType::Csv
        );
        assert_eq!(
            detect_source_type(b"", Some(".json")).unwrap(),
            SourceType::Json
        );
        assert_eq!(
            detect_source_type(b"", Some("xml")).unwrap(),
            SourceType::Xml
        );
        assert_eq!(
            detect_source_type(b"", Some(".pdf")).unwrap(),
            SourceType::Pdf
        );
    }

    #[tokio::test]
    async fn test_detect_source_type_magic_bytes_and_content() {
        // PDF magic bytes
        assert_eq!(
            detect_source_type(b"%PDF-1.7 ...", None).unwrap(),
            SourceType::Pdf
        );

        // JSON without extension
        assert_eq!(
            detect_source_type(b"  {\"name\": \"test\"}", None).unwrap(),
            SourceType::Json
        );
        assert_eq!(
            detect_source_type(b"  [1, 2, 3]", None).unwrap(),
            SourceType::Json
        );

        // XML without extension
        assert_eq!(
            detect_source_type(b"<?xml version=\"1.0\"?><items></items>", None).unwrap(),
            SourceType::Xml
        );
        assert_eq!(
            detect_source_type(b"<items><item>1</item></items>", None).unwrap(),
            SourceType::Xml
        );

        // CSV without extension
        let csv_data = b"col1,col2,col3\nval1,val2,val3\n";
        assert_eq!(
            detect_source_type(csv_data, None).unwrap(),
            SourceType::Csv
        );

        // Unknown
        assert!(detect_source_type(b"some random plain text", None).is_err());
    }

    #[tokio::test]
    async fn test_parse_auto_without_extension() {
        let json_data = br#"{"key": "value"}"#;
        let parsed = parse_auto(json_data, None).await.unwrap();
        assert_eq!(parsed.source_type, SourceType::Json);
        assert!(parsed.content.contains("\"key\": \"value\""));
    }

    // ─── SourceType Display ───

    #[test]
    fn source_type_display_formats_correctly() {
        assert_eq!(SourceType::Json.to_string(), "json");
        assert_eq!(SourceType::Xml.to_string(), "xml");
        assert_eq!(SourceType::Csv.to_string(), "csv");
        assert_eq!(SourceType::Pdf.to_string(), "pdf");
    }
}
