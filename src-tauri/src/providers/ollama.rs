use async_trait::async_trait;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

use super::{
	ChatMessage, ChatProvider, ChatRequest, ChatResponse, ChatRole, ModelInfo, ProviderHealth,
	OLLAMA_PROVIDER_ID,
};

pub const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434";

#[derive(Clone)]
pub struct OllamaProvider {
	client: Client,
	base_url: Url,
}

impl OllamaProvider {
	pub fn from_env() -> Result<Self, AppError> {
		let configured_url = std::env::var("TAURUS_OLLAMA_BASE_URL").ok();
		Self::new(configured_url.as_deref())
	}

	pub fn new(base_url: Option<&str>) -> Result<Self, AppError> {
		let resolved_url = resolve_base_url(base_url.unwrap_or(DEFAULT_OLLAMA_BASE_URL))?;
		let client = Client::builder().build()?;

		Ok(Self {
			client,
			base_url: resolved_url,
		})
	}

	fn endpoint(&self, path: &str) -> Result<Url, AppError> {
		self.base_url.join(path).map_err(|error| {
			AppError::Config(format!(
				"Could not construct provider endpoint URL: {error}"
			))
		})
	}
}

#[async_trait]
impl ChatProvider for OllamaProvider {
	async fn health_check(&self) -> Result<ProviderHealth, AppError> {
		let endpoint = self.endpoint("api/tags")?;

		let response = self.client.get(endpoint).send().await.map_err(|error| {
			AppError::ProviderUnavailable(format!(
				"Could not reach Ollama at '{}': {error}",
				self.base_url
			))
		})?;

		if !response.status().is_success() {
			return Err(AppError::ProviderUnavailable(format!(
				"Ollama health check failed with HTTP status {}.",
				response.status()
			)));
		}

		Ok(ProviderHealth {
			provider: OLLAMA_PROVIDER_ID.to_string(),
			healthy: true,
			base_url: self.base_url.to_string(),
			message: Some("Ollama is reachable.".to_string()),
		})
	}

	async fn list_models(&self) -> Result<Vec<ModelInfo>, AppError> {
		let endpoint = self.endpoint("api/tags")?;

		let response = self.client.get(endpoint).send().await.map_err(|error| {
			AppError::ProviderUnavailable(format!(
				"Could not reach Ollama at '{}': {error}",
				self.base_url
			))
		})?;

		if !response.status().is_success() {
			return Err(AppError::ProviderUnavailable(format!(
				"Ollama model listing failed with HTTP status {}.",
				response.status()
			)));
		}

		let payload: OllamaTagsResponse = response.json().await.map_err(|error| {
			AppError::ProviderProtocol(format!("Could not parse Ollama model response: {error}"))
		})?;

		Ok(map_models(payload))
	}

	async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, AppError> {
		let endpoint = self.endpoint("api/chat")?;

		let body = OllamaChatRequest {
			model: request.model,
			messages: request.messages.into_iter().map(From::from).collect(),
			stream: request.stream.unwrap_or(false),
			options: request
				.temperature
				.map(|temperature| OllamaChatOptions { temperature }),
		};

		let response = self
			.client
			.post(endpoint)
			.json(&body)
			.send()
			.await
			.map_err(|error| {
				AppError::ProviderUnavailable(format!(
					"Could not reach Ollama at '{}': {error}",
					self.base_url
				))
			})?;

		if !response.status().is_success() {
			return Err(AppError::ProviderUnavailable(format!(
				"Ollama chat request failed with HTTP status {}.",
				response.status()
			)));
		}

		let payload: OllamaChatResponse = response.json().await.map_err(|error| {
			AppError::ProviderProtocol(format!("Could not parse Ollama chat response: {error}"))
		})?;

		let role = parse_chat_role(&payload.message.role)?;

		Ok(ChatResponse {
			provider: OLLAMA_PROVIDER_ID.to_string(),
			model: payload.model,
			message: ChatMessage {
				role,
				content: payload.message.content,
			},
			done: payload.done,
			done_reason: payload.done_reason,
			created_at: payload.created_at,
		})
	}
}

fn resolve_base_url(base_url: &str) -> Result<Url, AppError> {
	let trimmed = base_url.trim();
	if trimmed.is_empty() {
		return Err(AppError::Config(
			"TAURUS_OLLAMA_BASE_URL cannot be empty.".to_string(),
		));
	}

	let normalized = if trimmed.ends_with('/') {
		trimmed.to_string()
	} else {
		format!("{trimmed}/")
	};

	let parsed = Url::parse(&normalized).map_err(|error| {
		AppError::Config(format!(
			"TAURUS_OLLAMA_BASE_URL is not a valid URL ('{trimmed}'): {error}"
		))
	})?;

	match parsed.scheme() {
		"http" | "https" => Ok(parsed),
		other => Err(AppError::Config(format!(
			"TAURUS_OLLAMA_BASE_URL must use http or https, found '{other}'."
		))),
	}
}

fn parse_chat_role(role: &str) -> Result<ChatRole, AppError> {
	match role {
		"system" => Ok(ChatRole::System),
		"user" => Ok(ChatRole::User),
		"assistant" => Ok(ChatRole::Assistant),
		_ => Err(AppError::ProviderProtocol(format!(
			"Unsupported role returned by provider: '{role}'."
		))),
	}
}

fn map_models(payload: OllamaTagsResponse) -> Vec<ModelInfo> {
	payload
		.models
		.into_iter()
		.map(|model| ModelInfo {
			provider: OLLAMA_PROVIDER_ID.to_string(),
			id: model.model.clone().unwrap_or_else(|| model.name.clone()),
			display_name: model.name,
			size_bytes: model.size,
			modified_at: model.modified_at,
		})
		.collect()
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
	models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
	name: String,
	model: Option<String>,
	modified_at: Option<String>,
	size: Option<u64>,
}

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
	model: String,
	messages: Vec<OllamaChatMessage>,
	stream: bool,
	#[serde(skip_serializing_if = "Option::is_none")]
	options: Option<OllamaChatOptions>,
}

#[derive(Debug, Serialize)]
struct OllamaChatMessage {
	role: String,
	content: String,
}

impl From<ChatMessage> for OllamaChatMessage {
	fn from(value: ChatMessage) -> Self {
		let role = match value.role {
			ChatRole::System => "system",
			ChatRole::User => "user",
			ChatRole::Assistant => "assistant",
		};

		Self {
			role: role.to_string(),
			content: value.content,
		}
	}
}

#[derive(Debug, Serialize)]
struct OllamaChatOptions {
	temperature: f32,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
	model: String,
	created_at: Option<String>,
	message: OllamaChatMessageResponse,
	done: bool,
	done_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaChatMessageResponse {
	role: String,
	content: String,
}

#[cfg(test)]
mod tests {
	use super::{map_models, resolve_base_url, OllamaTagsResponse};

	#[test]
	fn resolve_base_url_accepts_http_without_trailing_slash() {
		let resolved = resolve_base_url("http://localhost:11434").expect("url should parse");
		assert_eq!(resolved.as_str(), "http://localhost:11434/");
	}

	#[test]
	fn resolve_base_url_rejects_empty_values() {
		let error = resolve_base_url("   ").expect_err("empty URLs must fail");
		assert!(error.to_string().contains("cannot be empty"));
	}

	#[test]
	fn resolve_base_url_rejects_unsupported_scheme() {
		let error = resolve_base_url("ftp://localhost:11434").expect_err("scheme must fail");
		assert!(error.to_string().contains("http or https"));
	}

	#[test]
	fn map_models_prefers_model_field_when_present() {
		let payload: OllamaTagsResponse = serde_json::from_str(
			r#"{
                "models": [
                    {
                        "name": "llama3.1:8b",
                        "model": "llama3.1:8b-q4",
                        "modified_at": "2026-01-01T00:00:00Z",
                        "size": 123
                    }
                ]
            }"#,
		)
		.expect("json should parse");

		let models = map_models(payload);
		assert_eq!(models.len(), 1);
		assert_eq!(models[0].display_name, "llama3.1:8b");
		assert_eq!(models[0].id, "llama3.1:8b-q4");
		assert_eq!(models[0].size_bytes, Some(123));
	}
}
