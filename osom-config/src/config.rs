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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "format", rename_all = "lowercase")]
pub enum FieldType {
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
    /// Nested object with its own schema
    Object {
        fields: Vec<FieldDef>,
    },
    /// Array of items
    Array {
        item_type: Box<FieldType>,
    },
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
