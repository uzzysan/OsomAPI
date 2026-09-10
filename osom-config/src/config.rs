use serde::{Deserialize, Serialize};

/// Root configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// LLM provider configuration
    pub llm: LlmConfig,
    /// Output configuration (schema + destination)
    pub output: OutputConfig,
    /// Optional application settings
    #[serde(default)]
    pub settings: AppSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_timeout")]
    pub request_timeout_secs: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            request_timeout_secs: default_timeout(),
            max_retries: default_max_retries(),
            log_level: default_log_level(),
        }
    }
}

fn default_timeout() -> u64 { 120 }
fn default_max_retries() -> u32 { 3 }
fn default_log_level() -> String { "info".to_string() }

/// LLM provider configuration variants
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "lowercase")]
pub enum LlmConfig {
    /// Local Ollama instance
    Ollama {
        model: String,
        #[serde(default = "default_ollama_url")]
        url: String,
    },
    /// Google Gemini
    Gemini {
        api_key: String,
        #[serde(default = "default_gemini_model")]
        model: String,
    },
    /// OpenAI (GPT)
    OpenAi {
        api_key: String,
        #[serde(default = "default_openai_model")]
        model: String,
    },
    /// Anthropic (Claude)
    Anthropic {
        api_key: String,
        #[serde(default = "default_claude_model")]
        model: String,
    },
    /// GitHub Copilot
    Copilot {
        api_key: String,
        #[serde(default = "default_copilot_model")]
        model: String,
    },
}

fn default_ollama_url() -> String { "http://localhost:11434".to_string() }
fn default_gemini_model() -> String { "gemini-1.5-flash".to_string() }
fn default_openai_model() -> String { "gpt-4o-mini".to_string() }
fn default_claude_model() -> String { "claude-3-5-sonnet-20241022".to_string() }
fn default_copilot_model() -> String { "gpt-4o".to_string() }

/// Output configuration: schema + destination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Desired output schema definition
    pub schema: OutputSchema,
    /// Where to write the results
    pub destination: Destination,
}

/// Schema definition for output data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputSchema {
    /// Name of the output entity / collection
    pub name: String,
    /// Field definitions
    pub fields: Vec<FieldDef>,
    /// Whether to normalize letter casing
    #[serde(default)]
    pub case_normalization: Option<CaseNormalization>,
}

/// Field definition within output schema
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    #[serde(rename = "type", alias = "field_type")]
    pub field_type: FieldType,
    /// Whether this field is required
    #[serde(default = "default_true")]
    pub required: bool,
    /// Description for LLM prompt
    #[serde(default)]
    pub description: String,
    /// Default value if extraction fails
    #[serde(default)]
    pub default: Option<serde_json::Value>,
}

fn default_true() -> bool { true }

/// Supported field types
#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    String,
    Integer,
    Decimal {
        precision: u32,
        scale: u32,
    },
    Boolean,
    Date {
        format: String,
    },
    DateTime {
        format: String,
    },
    /// Nested object with its own schema
    Object {
        fields: Vec<FieldDef>,
    },
    /// Array of items
    Array {
        item_type: Box<FieldType>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum FieldTypeHelper {
    Simple(String),
    Detailed(DetailedFieldType),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum DetailedFieldType {
    String,
    Integer,
    Decimal {
        #[serde(default = "default_decimal_precision")]
        precision: u32,
        #[serde(default = "default_decimal_scale")]
        scale: u32,
    },
    Boolean,
    Date {
        #[serde(default = "default_date_format")]
        format: String,
    },
    DateTime {
        #[serde(default = "default_datetime_format")]
        format: String,
    },
    Object {
        fields: Vec<FieldDef>,
    },
    Array {
        item_type: Box<FieldType>,
    },
}

impl Serialize for FieldType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            FieldType::String => DetailedFieldType::String.serialize(serializer),
            FieldType::Integer => DetailedFieldType::Integer.serialize(serializer),
            FieldType::Decimal { precision, scale } => DetailedFieldType::Decimal {
                precision: *precision,
                scale: *scale,
            }
            .serialize(serializer),
            FieldType::Boolean => DetailedFieldType::Boolean.serialize(serializer),
            FieldType::Date { format } => DetailedFieldType::Date {
                format: format.clone(),
            }
            .serialize(serializer),
            FieldType::DateTime { format } => DetailedFieldType::DateTime {
                format: format.clone(),
            }
            .serialize(serializer),
            FieldType::Object { fields } => DetailedFieldType::Object {
                fields: fields.clone(),
            }
            .serialize(serializer),
            FieldType::Array { item_type } => DetailedFieldType::Array {
                item_type: item_type.clone(),
            }
            .serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for FieldType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let helper = FieldTypeHelper::deserialize(deserializer)?;
        match helper {
            FieldTypeHelper::Simple(s) => match s.to_lowercase().as_str() {
                "string" => Ok(FieldType::String),
                "integer" | "int" => Ok(FieldType::Integer),
                "decimal" | "float" | "number" => Ok(FieldType::Decimal {
                    precision: default_decimal_precision(),
                    scale: default_decimal_scale(),
                }),
                "boolean" | "bool" => Ok(FieldType::Boolean),
                "date" => Ok(FieldType::Date {
                    format: default_date_format(),
                }),
                "datetime" => Ok(FieldType::DateTime {
                    format: default_datetime_format(),
                }),
                _ => Err(serde::de::Error::unknown_variant(
                    &s,
                    &[
                        "string", "integer", "decimal", "boolean", "date", "datetime", "object",
                        "array",
                    ],
                )),
            },
            FieldTypeHelper::Detailed(d) => match d {
                DetailedFieldType::String => Ok(FieldType::String),
                DetailedFieldType::Integer => Ok(FieldType::Integer),
                DetailedFieldType::Decimal { precision, scale } => {
                    Ok(FieldType::Decimal { precision, scale })
                }
                DetailedFieldType::Boolean => Ok(FieldType::Boolean),
                DetailedFieldType::Date { format } => Ok(FieldType::Date { format }),
                DetailedFieldType::DateTime { format } => Ok(FieldType::DateTime { format }),
                DetailedFieldType::Object { fields } => Ok(FieldType::Object { fields }),
                DetailedFieldType::Array { item_type } => Ok(FieldType::Array { item_type }),
            },
        }
    }
}

fn default_decimal_precision() -> u32 { 19 }
fn default_decimal_scale() -> u32 { 4 }
fn default_date_format() -> String { "%Y-%m-%d".to_string() }
fn default_datetime_format() -> String { "%Y-%m-%dT%H:%M:%S".to_string() }

/// Letter case normalization options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaseNormalization {
    Lower,
    Upper,
    Title,
    Snake,
    Camel,
    Pascal,
}

/// Output destination variants
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Destination {
    Json {
        path: String,
        #[serde(default = "default_pretty")]
        pretty: bool,
    },
    Xml {
        path: String,
        #[serde(default = "default_pretty")]
        pretty: bool,
    },
    Database {
        #[serde(flatten)]
        connection: DbConnection,
        #[serde(default)]
        table_name: Option<String>,
    },
}

fn default_pretty() -> bool { true }

/// Database connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "driver", rename_all = "lowercase")]
pub enum DbConnection {
    Sqlite {
        path: String,
    },
    Postgres {
        host: String,
        #[serde(default = "default_pg_port")]
        port: u16,
        database: String,
        username: String,
        password: String,
    },
    Mysql {
        host: String,
        #[serde(default = "default_mysql_port")]
        port: u16,
        database: String,
        username: String,
        password: String,
    },
}

fn default_pg_port() -> u16 { 5432 }
fn default_mysql_port() -> u16 { 3306 }

impl Config {
    /// Load configuration from a TOML file
    pub fn from_toml_file(path: &str) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Load configuration from a JSON file
    pub fn from_json_file(path: &str) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Ensure at least one field is defined in schema
        if self.output.schema.fields.is_empty() {
            return Err(ConfigError::Validation("Output schema must define at least one field".into()));
        }
        // Validate API keys are present for cloud providers
        match &self.llm {
            LlmConfig::Ollama { .. } => {}
            LlmConfig::Gemini { api_key, .. }
            | LlmConfig::OpenAi { api_key, .. }
            | LlmConfig::Anthropic { api_key, .. }
            | LlmConfig::Copilot { api_key, .. } => {
                if api_key.trim().is_empty() {
                    return Err(ConfigError::Validation("API key cannot be empty for cloud LLM providers".into()));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Validation error: {0}")]
    Validation(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_TOML: &str = r#"
[llm]
provider = "ollama"
model = "llama3"
url = "http://localhost:11434"

[output]
[output.schema]
name = "test_schema"
case_normalization = "snake"

[[output.schema.fields]]
name = "id"
type = "integer"
required = true

[[output.schema.fields]]
name = "name"
type = "string"
required = false
default = "anonymous"

[output.destination]
type = "json"
path = "out.json"
pretty = true

[settings]
request_timeout_secs = 60
max_retries = 5
log_level = "debug"
"#;

    #[test]
    fn test_parse_valid_toml() {
        let config: Config = toml::from_str(VALID_TOML).expect("Failed to parse valid TOML");
        config.validate().expect("Validation failed");

        assert_eq!(config.settings.request_timeout_secs, 60);
        assert_eq!(config.settings.max_retries, 5);
        assert_eq!(config.settings.log_level, "debug");

        match &config.llm {
            LlmConfig::Ollama { model, url } => {
                assert_eq!(model, "llama3");
                assert_eq!(url, "http://localhost:11434");
            }
            _ => panic!("Expected Ollama config"),
        }

        assert_eq!(config.output.schema.fields.len(), 2);
        assert_eq!(config.output.schema.fields[0].name, "id");
        assert!(config.output.schema.fields[0].required);
    }

    #[test]
    fn test_parse_valid_json() {
        let json_str = r#"{
            "llm": {
                "provider": "openai",
                "api_key": "sk-test12345",
                "model": "gpt-4o"
            },
            "output": {
                "schema": {
                    "name": "json_schema",
                    "fields": [
                        { "name": "title", "type": "string" }
                    ]
                },
                "destination": {
                    "type": "xml",
                    "path": "out.xml"
                }
            }
        }"#;

        let config: Config = serde_json::from_str(json_str).expect("Failed to parse JSON");
        config.validate().expect("Validation failed");

        // Settings should have defaults
        assert_eq!(config.settings.request_timeout_secs, 120);
        assert_eq!(config.settings.max_retries, 3);
        assert_eq!(config.settings.log_level, "info");

        match &config.llm {
            LlmConfig::OpenAi { api_key, model } => {
                assert_eq!(api_key, "sk-test12345");
                assert_eq!(model, "gpt-4o");
            }
            _ => panic!("Expected OpenAI config"),
        }
    }

    #[test]
    fn test_validation_fails_empty_fields() {
        let toml_str = r#"
[llm]
provider = "ollama"
model = "llama3"

[output]
[output.schema]
name = "empty_fields"
fields = []

[output.destination]
type = "json"
path = "out.json"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let err = config.validate().unwrap_err();
        match err {
            ConfigError::Validation(msg) => {
                assert!(msg.contains("at least one field"));
            }
            _ => panic!("Expected Validation error"),
        }
    }

    #[test]
    fn test_validation_fails_empty_cloud_api_key() {
        let toml_str = r#"
[llm]
provider = "gemini"
api_key = "   "
model = "gemini-1.5-flash"

[output]
[output.schema]
name = "schema"
[[output.schema.fields]]
name = "id"
type = "integer"

[output.destination]
type = "json"
path = "out.json"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let err = config.validate().unwrap_err();
        match err {
            ConfigError::Validation(msg) => {
                assert!(msg.contains("API key cannot be empty"));
            }
            _ => panic!("Expected Validation error"),
        }
    }

    #[test]
    fn test_all_llm_provider_variants() {
        let providers = vec![
            (
                r#"[llm]
provider = "anthropic"
api_key = "key"
[output]
[output.schema]
name = "s"
[[output.schema.fields]]
name = "f"
type = "string"
[output.destination]
type = "json"
path = "p""#,
                "anthropic",
            ),
            (
                r#"[llm]
provider = "copilot"
api_key = "key"
[output]
[output.schema]
name = "s"
[[output.schema.fields]]
name = "f"
type = "string"
[output.destination]
type = "json"
path = "p""#,
                "copilot",
            ),
        ];

        for (toml_content, expected) in providers {
            let config: Config = toml::from_str(toml_content).unwrap();
            config.validate().unwrap();
            match (&config.llm, expected) {
                (LlmConfig::Anthropic { .. }, "anthropic") => {}
                (LlmConfig::Copilot { .. }, "copilot") => {}
                _ => panic!("Provider mismatch"),
            }
        }
    }

    #[test]
    fn test_database_destinations() {
        let toml_sqlite = r#"
[llm]
provider = "ollama"
model = "llama3"
[output]
[output.schema]
name = "s"
[[output.schema.fields]]
name = "f"
type = "string"
[output.destination]
type = "database"
driver = "sqlite"
path = "test.db"
table_name = "records"
"#;
        let config: Config = toml::from_str(toml_sqlite).unwrap();
        match &config.output.destination {
            Destination::Database { connection, table_name } => {
                assert_eq!(table_name.as_deref(), Some("records"));
                match connection {
                    DbConnection::Sqlite { path } => assert_eq!(path, "test.db"),
                    _ => panic!("Expected SQLite"),
                }
            }
            _ => panic!("Expected Database destination"),
        }
    }

    #[test]
    fn test_parse_config_example_toml() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../config.example.toml");
        let config = Config::from_toml_file(path).expect("Failed to parse config.example.toml");
        assert_eq!(config.output.schema.name, "produkty");
        assert_eq!(config.output.schema.fields.len(), 6);
        assert_eq!(config.settings.request_timeout_secs, 120);
        assert_eq!(config.settings.max_retries, 3);
        assert_eq!(config.settings.log_level, "info");
    }
}
