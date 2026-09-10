use osom_config::DbConnection;
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

/// Zapisuje dane do bazy danych (SQLite, PostgreSQL, MySQL).
pub struct DatabaseWriter {
    connection: DbConnection,
    table_name: String,
}

impl DatabaseWriter {
    /// Tworzy nowego DatabaseWriter z dowolnym połączeniem
    pub fn new(connection: DbConnection, table_name: Option<String>) -> Self {
        Self {
            connection,
            table_name: table_name.unwrap_or_else(|| "osom_records".to_string()),
        }
    }

    /// Tworzy nowego DatabaseWriter dla bazy SQLite
    pub fn new_sqlite(path: String, table_name: Option<String>) -> Self {
        Self::new(DbConnection::Sqlite { path }, table_name)
    }

    /// Tworzy nowego DatabaseWriter dla bazy PostgreSQL
    pub fn new_postgres(
        host: String,
        port: u16,
        database: String,
        username: String,
        password: String,
        table_name: Option<String>,
    ) -> Self {
        Self::new(
            DbConnection::Postgres {
                host,
                port,
                database,
                username,
                password,
            },
            table_name,
        )
    }

    /// Tworzy nowego DatabaseWriter dla bazy MySQL
    pub fn new_mysql(
        host: String,
        port: u16,
        database: String,
        username: String,
        password: String,
        table_name: Option<String>,
    ) -> Self {
        Self::new(
            DbConnection::Mysql {
                host,
                port,
                database,
                username,
                password,
            },
            table_name,
        )
    }

    /// Zapisuje sformatowane dane do bazy danych.
    pub async fn write(&self, data: &FormattedData) -> Result<(), WriterError> {
        match &self.connection {
            DbConnection::Sqlite { path } => self.write_sqlite(path, data).await,
            DbConnection::Postgres {
                host,
                port,
                database,
                username,
                password,
            } => {
                self.write_postgres(host, *port, database, username, password, data)
                    .await
            }
            DbConnection::Mysql {
                host,
                port,
                database,
                username,
                password,
            } => {
                self.write_mysql(host, *port, database, username, password, data)
                    .await
            }
        }
    }

    async fn write_sqlite(&self, path: &str, data: &FormattedData) -> Result<(), WriterError> {
        use sqlx::sqlite::SqliteConnectOptions;
        use sqlx::SqlitePool;
        use std::str::FromStr;

        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", path))
            .map_err(|e| WriterError::Database(e.to_string()))?
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .map_err(|e| WriterError::Database(e.to_string()))?;

        if !data.records.is_empty() {
            let first = &data.records[0];
            let columns: Vec<String> = first
                .iter()
                .map(|(k, v)| {
                    let col_type = match v {
                        serde_json::Value::Number(n) if n.is_i64() => "INTEGER",
                        serde_json::Value::Number(_) => "REAL",
                        serde_json::Value::Bool(_) => "INTEGER",
                        _ => "TEXT",
                    };
                    format!("{} {}", escape_sql_identifier(k), col_type)
                })
                .collect();
            let has_id = first.keys().any(|k| k.eq_ignore_ascii_case("id"));
            let table_cols = if has_id {
                columns.join(", ")
            } else {
                format!("_id INTEGER PRIMARY KEY AUTOINCREMENT, {}", columns.join(", "))
            };
            let create_sql = format!(
                "CREATE TABLE IF NOT EXISTS {} ({})",
                escape_sql_identifier(&self.table_name),
                table_cols
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
            let placeholders = vec!["?"; keys.len()].join(", ");
            let insert_sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                escape_sql_identifier(&self.table_name),
                columns,
                placeholders
            );

            let mut query = sqlx::query(&insert_sql);
            for key in keys {
                match record.get(key) {
                    Some(serde_json::Value::String(s)) => {
                        query = query.bind(s.clone());
                    }
                    Some(serde_json::Value::Number(n)) => {
                        if let Some(i) = n.as_i64() {
                            query = query.bind(i);
                        } else if let Some(f) = n.as_f64() {
                            query = query.bind(f);
                        } else {
                            query = query.bind(n.to_string());
                        }
                    }
                    Some(serde_json::Value::Bool(b)) => {
                        query = query.bind(*b);
                    }
                    Some(serde_json::Value::Null) | None => {
                        query = query.bind(Option::<String>::None);
                    }
                    Some(other) => {
                        query = query.bind(other.to_string());
                    }
                }
            }

            query
                .execute(&pool)
                .await
                .map_err(|e| WriterError::Database(e.to_string()))?;
        }

        info!("Zapisano dane do SQLite: {}", path);
        Ok(())
    }

    async fn write_postgres(
        &self,
        host: &str,
        port: u16,
        database: &str,
        username: &str,
        password: &str,
        data: &FormattedData,
    ) -> Result<(), WriterError> {
        use sqlx::PgPool;

        let url = format!(
            "postgres://{}:{}@{}:{}/{}",
            urlencoding_encode(username),
            urlencoding_encode(password),
            host,
            port,
            database
        );
        let pool = PgPool::connect(&url)
            .await
            .map_err(|e| WriterError::Database(e.to_string()))?;

        if !data.records.is_empty() {
            let first = &data.records[0];
            let columns: Vec<String> = first
                .iter()
                .map(|(k, v)| {
                    let col_type = match v {
                        serde_json::Value::Number(n) if n.is_i64() => "BIGINT",
                        serde_json::Value::Number(_) => "DOUBLE PRECISION",
                        serde_json::Value::Bool(_) => "BOOLEAN",
                        _ => "TEXT",
                    };
                    format!("{} {}", escape_sql_identifier(k), col_type)
                })
                .collect();
            let has_id = first.keys().any(|k| k.eq_ignore_ascii_case("id"));
            let table_cols = if has_id {
                columns.join(", ")
            } else {
                format!("_id BIGSERIAL PRIMARY KEY, {}", columns.join(", "))
            };
            let create_sql = format!(
                "CREATE TABLE IF NOT EXISTS {} ({})",
                escape_sql_identifier(&self.table_name),
                table_cols
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
            let placeholders = (1..=keys.len())
                .map(|i| format!("${}", i))
                .collect::<Vec<_>>()
                .join(", ");
            let insert_sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                escape_sql_identifier(&self.table_name),
                columns,
                placeholders
            );

            let mut query = sqlx::query(&insert_sql);
            for key in keys {
                match record.get(key) {
                    Some(serde_json::Value::String(s)) => {
                        query = query.bind(s.clone());
                    }
                    Some(serde_json::Value::Number(n)) => {
                        if let Some(i) = n.as_i64() {
                            query = query.bind(i);
                        } else if let Some(f) = n.as_f64() {
                            query = query.bind(f);
                        } else {
                            query = query.bind(n.to_string());
                        }
                    }
                    Some(serde_json::Value::Bool(b)) => {
                        query = query.bind(*b);
                    }
                    Some(serde_json::Value::Null) | None => {
                        query = query.bind(Option::<String>::None);
                    }
                    Some(other) => {
                        query = query.bind(other.to_string());
                    }
                }
            }

            query
                .execute(&pool)
                .await
                .map_err(|e| WriterError::Database(e.to_string()))?;
        }

        info!("Zapisano dane do PostgreSQL: {}/{}", host, database);
        Ok(())
    }

    async fn write_mysql(
        &self,
        host: &str,
        port: u16,
        database: &str,
        username: &str,
        password: &str,
        data: &FormattedData,
    ) -> Result<(), WriterError> {
        use sqlx::MySqlPool;

        let url = format!(
            "mysql://{}:{}@{}:{}/{}",
            urlencoding_encode(username),
            urlencoding_encode(password),
            host,
            port,
            database
        );
        let pool = MySqlPool::connect(&url)
            .await
            .map_err(|e| WriterError::Database(e.to_string()))?;

        if !data.records.is_empty() {
            let first = &data.records[0];
            let columns: Vec<String> = first
                .iter()
                .map(|(k, v)| {
                    let col_type = match v {
                        serde_json::Value::Number(n) if n.is_i64() => "BIGINT",
                        serde_json::Value::Number(_) => "DOUBLE",
                        serde_json::Value::Bool(_) => "BOOLEAN",
                        _ => "TEXT",
                    };
                    format!("{} {}", escape_mysql_identifier(k), col_type)
                })
                .collect();
            let has_id = first.keys().any(|k| k.eq_ignore_ascii_case("id"));
            let table_cols = if has_id {
                columns.join(", ")
            } else {
                format!(
                    "_id BIGINT AUTO_INCREMENT PRIMARY KEY, {}",
                    columns.join(", ")
                )
            };
            let create_sql = format!(
                "CREATE TABLE IF NOT EXISTS {} ({})",
                escape_mysql_identifier(&self.table_name),
                table_cols
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
                .map(|k| escape_mysql_identifier(k))
                .collect::<Vec<_>>()
                .join(", ");
            let placeholders = vec!["?"; keys.len()].join(", ");
            let insert_sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                escape_mysql_identifier(&self.table_name),
                columns,
                placeholders
            );

            let mut query = sqlx::query(&insert_sql);
            for key in keys {
                match record.get(key) {
                    Some(serde_json::Value::String(s)) => {
                        query = query.bind(s.clone());
                    }
                    Some(serde_json::Value::Number(n)) => {
                        if let Some(i) = n.as_i64() {
                            query = query.bind(i);
                        } else if let Some(f) = n.as_f64() {
                            query = query.bind(f);
                        } else {
                            query = query.bind(n.to_string());
                        }
                    }
                    Some(serde_json::Value::Bool(b)) => {
                        query = query.bind(*b);
                    }
                    Some(serde_json::Value::Null) | None => {
                        query = query.bind(Option::<String>::None);
                    }
                    Some(other) => {
                        query = query.bind(other.to_string());
                    }
                }
            }

            query
                .execute(&pool)
                .await
                .map_err(|e| WriterError::Database(e.to_string()))?;
        }

        info!("Zapisano dane do MySQL: {}/{}", host, database);
        Ok(())
    }
}

fn escape_sql_identifier(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn escape_mysql_identifier(s: &str) -> String {
    format!("`{}`", s.replace('`', "``"))
}

fn urlencoding_encode(s: &str) -> String {
    let mut encoded = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{:02X}", b));
        }
    }
    encoded
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

    #[test]
    fn test_escape_mysql_identifier() {
        assert_eq!(escape_mysql_identifier("users"), "`users`");
        assert_eq!(escape_mysql_identifier("user`s"), "`user``s`");
    }

    #[test]
    fn test_urlencoding_encode() {
        assert_eq!(urlencoding_encode("hello world"), "hello%20world");
        assert_eq!(urlencoding_encode("p@ss!word"), "p%40ss%21word");
    }

    #[test]
    fn test_database_writer_constructors() {
        let pg_writer = DatabaseWriter::new_postgres(
            "localhost".to_string(),
            5432,
            "test_db".to_string(),
            "user".to_string(),
            "pass".to_string(),
            Some("my_table".to_string()),
        );
        assert_eq!(pg_writer.table_name, "my_table");

        let mysql_writer = DatabaseWriter::new_mysql(
            "localhost".to_string(),
            3306,
            "test_db".to_string(),
            "user".to_string(),
            "pass".to_string(),
            None,
        );
        assert_eq!(mysql_writer.table_name, "osom_records");
    }

    #[tokio::test]
    async fn test_sqlite_writer_parameterized() {
        use serde_json::json;
        let db_path = "/tmp/opencode/test_writer.db";
        let _ = tokio::fs::remove_file(db_path).await;

        let writer = DatabaseWriter::new_sqlite(db_path.to_string(), Some("test_table".to_string()));
        let mut map1 = serde_json::Map::new();
        map1.insert("title".to_string(), json!("Item 'with' quotes"));
        map1.insert("count".to_string(), json!(42));
        map1.insert("price".to_string(), json!(19.99));
        map1.insert("active".to_string(), json!(true));
        map1.insert("extra".to_string(), serde_json::Value::Null);

        let data = FormattedData {
            records: vec![map1],
        };

        writer.write(&data).await.expect("Failed to write to sqlite");

        // Verify with sqlx
        let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", db_path))
            .await
            .expect("Failed to connect");
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM test_table")
            .fetch_one(&pool)
            .await
            .expect("Failed to query count");
        assert_eq!(count.0, 1);

        let _ = tokio::fs::remove_file(db_path).await;
    }
}
