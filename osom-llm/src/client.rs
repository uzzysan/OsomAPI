use async_trait::async_trait;
use osom_config::LlmConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error, info, instrument};

/// Błąd operacji LLM
#[derive(Debug, Error)]
pub enum LlmError {
    /// Błąd żądania HTTP
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    /// Błąd serializacji/deserializacji JSON
    #[error("JSON serialization/deserialization failed: {0}")]
    Json(#[from] serde_json::Error),
    /// Błąd zwrócony przez API providera
    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },
    /// Pusta odpowiedź od modelu LLM
    #[error("Empty response from LLM")]
    EmptyResponse,
    /// Nieprawidłowa konfiguracja
    #[error("Invalid configuration: {0}")]
    Config(String),
    /// Nieznany provider LLM
    #[error("Unknown LLM provider")]
    UnknownProvider,
}

/// Trait dla klientów LLM
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Wysyła prompt do modelu LLM i zwraca odpowiedź jako tekst
    async fn send(&self, prompt: &str) -> Result<String, LlmError>;
}

/// Tworzy klienta LLM na podstawie konfiguracji
pub fn build_llm_client(config: &LlmConfig) -> Result<Box<dyn LlmClient>, LlmError> {
    match config {
        LlmConfig::Ollama { model, url } => {
            Ok(Box::new(OllamaClient::new(url.clone(), model.clone())))
        }
        LlmConfig::Gemini { api_key, model } => {
            Ok(Box::new(GeminiClient::new(api_key.clone(), model.clone())))
        }
        LlmConfig::OpenAi { api_key, model } => {
            Ok(Box::new(OpenAiClient::new(api_key.clone(), model.clone())))
        }
        LlmConfig::Anthropic { api_key, model } => {
            Ok(Box::new(AnthropicClient::new(api_key.clone(), model.clone())))
        }
        LlmConfig::Copilot { api_key, model } => {
            Ok(Box::new(CopilotClient::new(api_key.clone(), model.clone())))
        }
    }
}

// ============== Ollama ==============

/// Klient dla lokalnej instancji Ollama
pub struct OllamaClient {
    client: Client,
    base_url: String,
    model: String,
}

impl OllamaClient {
    /// Tworzy nowego klienta Ollama
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("Failed to build HTTP client"),
            base_url,
            model,
        }
    }
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

#[async_trait]
impl LlmClient for OllamaClient {
    #[instrument(skip(self, prompt), fields(model = %self.model))]
    async fn send(&self, prompt: &str) -> Result<String, LlmError> {
        let url = format!("{}/api/generate", self.base_url);
        let body = OllamaRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
        };

        debug!(url = %url, "Sending request to Ollama");

        let response = self.client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            error!(status = status, message = %message, "Ollama API error");
            return Err(LlmError::Api { status, message });
        }

        let data: OllamaResponse = response.json().await?;
        info!("Received response from Ollama");
        Ok(data.response)
    }
}

// ============== OpenAI ==============

/// Klient dla API OpenAI (GPT/ChatGPT)
pub struct OpenAiClient {
    client: Client,
    api_key: String,
    model: String,
}

impl OpenAiClient {
    /// Tworzy nowego klienta OpenAI
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("Failed to build HTTP client"),
            api_key,
            model,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
}

#[async_trait]
impl LlmClient for OpenAiClient {
    #[instrument(skip(self, prompt), fields(model = %self.model))]
    async fn send(&self, prompt: &str) -> Result<String, LlmError> {
        let url = "https://api.openai.com/v1/chat/completions";
        let body = OpenAiRequest {
            model: self.model.clone(),
            messages: vec![OpenAiMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        debug!("Sending request to OpenAI");

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            error!(status = status, message = %message, "OpenAI API error");
            return Err(LlmError::Api { status, message });
        }

        let data: OpenAiResponse = response.json().await?;
        let content = data.choices.into_iter().next().map(|c| c.message.content);
        match content {
            Some(text) if !text.is_empty() => {
                info!("Received response from OpenAI");
                Ok(text)
            }
            _ => Err(LlmError::EmptyResponse),
        }
    }
}

// ============== Gemini ==============

/// Klient dla API Google Gemini
pub struct GeminiClient {
    client: Client,
    api_key: String,
    model: String,
}

impl GeminiClient {
    /// Tworzy nowego klienta Gemini
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("Failed to build HTTP client"),
            api_key,
            model,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct GeminiPart {
    text: String,
}

#[derive(Serialize, Deserialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}

#[async_trait]
impl LlmClient for GeminiClient {
    #[instrument(skip(self, prompt), fields(model = %self.model))]
    async fn send(&self, prompt: &str) -> Result<String, LlmError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );
        let body = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart {
                    text: prompt.to_string(),
                }],
            }],
        };

        debug!("Sending request to Gemini");

        let response = self.client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            error!(status = status, message = %message, "Gemini API error");
            return Err(LlmError::Api { status, message });
        }

        let data: GeminiResponse = response.json().await?;
        let text = data
            .candidates
            .into_iter()
            .next()
            .and_then(|c| c.content.parts.into_iter().next())
            .map(|p| p.text);

        match text {
            Some(t) if !t.is_empty() => {
                info!("Received response from Gemini");
                Ok(t)
            }
            _ => Err(LlmError::EmptyResponse),
        }
    }
}

// ============== Anthropic ==============

/// Klient dla API Anthropic (Claude)
pub struct AnthropicClient {
    client: Client,
    api_key: String,
    model: String,
}

impl AnthropicClient {
    /// Tworzy nowego klienta Anthropic
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("Failed to build HTTP client"),
            api_key,
            model,
        }
    }
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
}

#[derive(Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
}

#[async_trait]
impl LlmClient for AnthropicClient {
    #[instrument(skip(self, prompt), fields(model = %self.model))]
    async fn send(&self, prompt: &str) -> Result<String, LlmError> {
        let url = "https://api.anthropic.com/v1/messages";
        let body = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        debug!("Sending request to Anthropic");

        let response = self
            .client
            .post(url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            error!(status = status, message = %message, "Anthropic API error");
            return Err(LlmError::Api { status, message });
        }

        let data: AnthropicResponse = response.json().await?;
        let text = data
            .content
            .into_iter()
            .find(|b| b.block_type == "text")
            .map(|b| b.text);

        match text {
            Some(t) if !t.is_empty() => {
                info!("Received response from Anthropic");
                Ok(t)
            }
            _ => Err(LlmError::EmptyResponse),
        }
    }
}

// ============== Copilot ==============

/// Klient dla GitHub Copilot Chat API
pub struct CopilotClient {
    client: Client,
    api_key: String,
    model: String,
}

impl CopilotClient {
    /// Tworzy nowego klienta Copilot
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("Failed to build HTTP client"),
            api_key,
            model,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct CopilotMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct CopilotRequest {
    model: String,
    messages: Vec<CopilotMessage>,
}

#[derive(Deserialize)]
struct CopilotChoice {
    message: CopilotMessage,
}

#[derive(Deserialize)]
struct CopilotResponse {
    choices: Vec<CopilotChoice>,
}

#[async_trait]
impl LlmClient for CopilotClient {
    #[instrument(skip(self, prompt), fields(model = %self.model))]
    async fn send(&self, prompt: &str) -> Result<String, LlmError> {
        let url = "https://api.githubcopilot.com/chat/completions";
        let body = CopilotRequest {
            model: self.model.clone(),
            messages: vec![CopilotMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        debug!("Sending request to Copilot");

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            error!(status = status, message = %message, "Copilot API error");
            return Err(LlmError::Api { status, message });
        }

        let data: CopilotResponse = response.json().await?;
        let content = data.choices.into_iter().next().map(|c| c.message.content);
        match content {
            Some(text) if !text.is_empty() => {
                info!("Received response from Copilot");
                Ok(text)
            }
            _ => Err(LlmError::EmptyResponse),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_llm_client_ollama() {
        let config = LlmConfig::Ollama {
            model: "llama3".to_string(),
            url: "http://localhost:11434".to_string(),
        };
        let client = build_llm_client(&config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_build_llm_client_gemini() {
        let config = LlmConfig::Gemini {
            api_key: "test-key".to_string(),
            model: "gemini-1.5-flash".to_string(),
        };
        let client = build_llm_client(&config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_build_llm_client_openai() {
        let config = LlmConfig::OpenAi {
            api_key: "test-key".to_string(),
            model: "gpt-4o-mini".to_string(),
        };
        let client = build_llm_client(&config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_build_llm_client_anthropic() {
        let config = LlmConfig::Anthropic {
            api_key: "test-key".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
        };
        let client = build_llm_client(&config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_build_llm_client_copilot() {
        let config = LlmConfig::Copilot {
            api_key: "test-key".to_string(),
            model: "gpt-4o".to_string(),
        };
        let client = build_llm_client(&config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_llm_error_display() {
        let err = LlmError::EmptyResponse;
        assert_eq!(err.to_string(), "Empty response from LLM");

        let err = LlmError::Config("bad config".to_string());
        assert_eq!(err.to_string(), "Invalid configuration: bad config");
    }

    #[test]
    fn test_llm_error_api() {
        let err = LlmError::Api {
            status: 401,
            message: "Unauthorized".to_string(),
        };
        assert!(err.to_string().contains("401"));
        assert!(err.to_string().contains("Unauthorized"));
    }
}
