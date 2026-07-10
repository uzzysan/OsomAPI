use osom_schema::formatter::FormattedData;
use thiserror::Error;
use tracing::info;

/// Błąd zapisu danych wyjściowych.
#[derive(Debug, Error)]
pub enum WriterError {
    #[error("Błąd wejścia/wyjścia: {0}")]
    Io(#[from] std::io::Error),
    #[error("Błąd serializacji: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Błąd XML: {0}")]
    Xml(String),
    #[error("Błąd bazy danych: {0}")]
    Database(String),
}

/// Zapisuje dane do pliku JSON.
pub struct JsonWriter {
    path: String,
    pretty: bool,
}

impl JsonWriter {
    pub fn new(path: String, pretty: bool) -> Self {
        Self { path, pretty }
    }

    /// Zapisuje sformatowane dane do pliku JSON.
    pub async fn write(&self, data: &FormattedData) -> Result<(), WriterError> {
        let content = if self.pretty {
            serde_json::to_string_pretty(&data.records)?
        } else {
            serde_json::to_string(&data.records)?
        };
        tokio::fs::write(&self.path, content).await?;
        info!("Zapisano dane do JSON: {}", self.path);
        Ok(())
    }
}

/// Zapisuje dane do pliku XML.
pub struct XmlWriter {
    path: String,
    pretty: bool,
}

impl XmlWriter {
    pub fn new(path: String, pretty: bool) -> Self {
        Self { path, pretty }
    }

    /// Zapisuje sformatowane dane do pliku XML.
    pub async fn write(&self, data: &FormattedData) -> Result<(), WriterError> {
        let mut xml = String::new();
        xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        xml.push('\n');
        xml.push_str("<records>\n");

        for record in &data.records {
            xml.push_str("  <record>\n");
            for (key, value) in record {
                let value_str = match value {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                let tag = escape_xml(key);
                let val = escape_xml(&value_str);
                if self.pretty {
                    xml.push_str(&format!("    <{}>{}</{}>\n", tag, val, tag));
                } else {
                    xml.push_str(&format!("<{}>{}</{}>", tag, val, tag));
                }
            }
            xml.push_str("  </record>\n");
        }

        xml.push_str("</records>\n");
        tokio::fs::write(&self.path, xml).await?;
        info!("Zapisano dane do XML: {}", self.path);
        Ok(())
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Zapisuje dane do bazy danych SQLite.
pub struct DatabaseWriter {
    connection_string: String,
    table_name: String,
    driver: String,
}

impl DatabaseWriter {
    pub fn new_sqlite(path: String, table_name: Option<String>) -> Self {
        Self {
            connection_string: path,
            table_name: table_name.unwrap_or_else(|| "osom_records".to_string()),
            driver: "sqlite".to_string(),
        }
    }

    /// Zapisuje sformatowane dane do bazy danych.
    pub async fn write(&self, data: &FormattedData) -> Result<(), WriterError> {
        if self.driver != "sqlite" {
            return Err(WriterError::Database(format!(
                "Nieobsługiwany sterownik bazy danych: {}",
                self.driver
            )));
        }
        self.write_sqlite(data).await
    }

    async fn write_sqlite(&self, data: &FormattedData) -> Result<(), WriterError> {
        use sqlx::sqlite::SqliteConnectOptions;
        use sqlx::SqlitePool;
        use std::str::FromStr;

        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", self.connection_string))
            .map_err(|e| WriterError::Database(e.to_string()))?
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .map_err(|e| WriterError::Database(e.to_string()))?;

        if !data.records.is_empty() {
            let first = &data.records[0];
            let columns: Vec<String> = first
                .keys()
                .map(|k| format!("{} TEXT", escape_sql_identifier(k)))
                .collect();
            let create_sql = format!(
                "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY AUTOINCREMENT, {})",
                escape_sql_identifier(&self.table_name),
                columns.join(", ")
            );
            sqlx::query(&create_sql)
                .execute(&pool)
                .await
                .map_err(|e| WriterError::Database(e.to_string()))?;
        }

        for record in &data.records {
            let keys: Vec<&String> = record.keys().collect();
            let columns = keys
                .iter()
                .map(|k| escape_sql_identifier(k))
                .collect::<Vec<_>>()
                .join(", ");
            let values: Vec<String> = keys
                .iter()
                .map(|k| {
                    let v = match record.get(*k).unwrap() {
                        serde_json::Value::String(s) => s.replace('\'', "''"),
                        other => other.to_string().replace('\'', "''"),
                    };
                    format!("'{}'", v)
                })
                .collect();
            let insert_sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                escape_sql_identifier(&self.table_name),
                columns,
                values.join(", ")
            );
            sqlx::query(&insert_sql)
                .execute(&pool)
                .await
                .map_err(|e| WriterError::Database(e.to_string()))?;
        }

        info!("Zapisano dane do SQLite: {}", self.connection_string);
        Ok(())
    }
}

fn escape_sql_identifier(s: &str) -> String {
    // SQLite identifiers should not contain special chars; wrap in quotes if needed
    // Simple approach: wrap all in double quotes and escape embedded double quotes
    format!("\"{}\"", s.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_writer_new() {
        let writer = JsonWriter::new("out.json".to_string(), true);
        assert_eq!(writer.path, "out.json");
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(escape_xml("a & b"), "a &amp; b");
        assert_eq!(escape_xml("<tag>"), "&lt;tag&gt;");
    }

    #[test]
    fn test_escape_sql_identifier() {
        assert_eq!(escape_sql_identifier("users"), "\"users\"");
        assert_eq!(escape_sql_identifier("user\"s"), "\"user\"\"s\"");
    }
}
