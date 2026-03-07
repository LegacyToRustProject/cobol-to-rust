use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Trait for LLM providers used in code generation.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send a prompt and get a response.
    async fn generate(&self, request: &LlmRequest) -> Result<LlmResponse>;

    /// Provider name for logging.
    fn name(&self) -> &str;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    pub system_prompt: String,
    pub user_prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: String,
    pub tokens_used: Option<u32>,
}

/// Claude API implementation.
pub struct ClaudeProvider {
    api_key: String,
    model: String,
}

impl ClaudeProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "claude-sonnet-4-20250514".to_string(),
        }
    }

    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }
}

#[async_trait]
impl LlmProvider for ClaudeProvider {
    async fn generate(&self, request: &LlmRequest) -> Result<LlmResponse> {
        let client = reqwest::Client::new();

        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": request.max_tokens,
            "temperature": request.temperature,
            "system": request.system_prompt,
            "messages": [
                {
                    "role": "user",
                    "content": request.user_prompt
                }
            ]
        });

        let resp = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            anyhow::bail!("Claude API error ({}): {}", status, text);
        }

        let json: serde_json::Value = serde_json::from_str(&text)?;

        let content = json["content"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|block| block["text"].as_str())
            .unwrap_or("")
            .to_string();

        let tokens_used = json["usage"]["output_tokens"].as_u64().map(|t| t as u32);

        Ok(LlmResponse {
            content,
            tokens_used,
        })
    }

    fn name(&self) -> &str {
        "Claude"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_request_serialization() {
        let req = LlmRequest {
            system_prompt: "You are a COBOL expert.".to_string(),
            user_prompt: "Convert this COBOL to Rust.".to_string(),
            max_tokens: 4096,
            temperature: 0.0,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("COBOL expert"));
    }
}
