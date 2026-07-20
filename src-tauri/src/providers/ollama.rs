use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::AppError;

use super::{
	ChatMessage, ChatProvider, ChatRequest, ChatResponse, ChatRole, ChatStreamChunk, ModelInfo,
	ProviderHealth, ToolCall, ToolDefinition, OLLAMA_PROVIDER_ID,
};

pub const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434";
const OLLAMA_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const OLLAMA_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

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
		let client = Client::builder()
			.connect_timeout(OLLAMA_CONNECT_TIMEOUT)
			.build()?;

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

	fn build_chat_request_body(
		request: ChatRequest,
		stream: bool,
		tools: Vec<ToolDefinition>,
	) -> OllamaChatRequest {
		OllamaChatRequest {
			model: request.model,
			messages: request.messages.into_iter().map(From::from).collect(),
			stream,
			tools,
			options: request
				.temperature
				.map(|temperature| OllamaChatOptions { temperature }),
		}
	}

	async fn chat_with_tool_definitions(
		&self,
		request: ChatRequest,
		tools: Vec<ToolDefinition>,
	) -> Result<ChatResponse, AppError> {
		let endpoint = self.endpoint("api/chat")?;
		let body = Self::build_chat_request_body(request, false, tools);
		let response = self
			.client
			.post(endpoint)
			.json(&body)
			.timeout(OLLAMA_REQUEST_TIMEOUT)
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
				"Ollama chat request failed with HTTP status {}. Ensure the selected model supports tool calling.",
				response.status()
			)));
		}
		let payload: OllamaChatResponse = response.json().await.map_err(|error| {
			AppError::ProviderProtocol(format!("Could not parse Ollama chat response: {error}"))
		})?;
		map_ollama_response(payload)
	}
}

#[async_trait]
impl ChatProvider for OllamaProvider {
	async fn health_check(&self) -> Result<ProviderHealth, AppError> {
		let endpoint = self.endpoint("api/tags")?;

		let response = self
			.client
			.get(endpoint)
			.timeout(OLLAMA_REQUEST_TIMEOUT)
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

		let response = self
			.client
			.get(endpoint)
			.timeout(OLLAMA_REQUEST_TIMEOUT)
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
		self.chat_with_tool_definitions(request, Vec::new()).await
	}

	async fn chat_with_tools(
		&self,
		request: ChatRequest,
		tools: Vec<ToolDefinition>,
	) -> Result<ChatResponse, AppError> {
		self.chat_with_tool_definitions(request, tools).await
	}

	async fn chat_stream(
		&self,
		request: ChatRequest,
		mut on_chunk: Box<dyn FnMut(ChatStreamChunk) -> Result<(), AppError> + Send>,
	) -> Result<ChatResponse, AppError> {
		let endpoint = self.endpoint("api/chat")?;
		let body = Self::build_chat_request_body(request, true, Vec::new());
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
				"Ollama streaming chat request failed with HTTP status {}.",
				response.status()
			)));
		}
		let mut stream = response.bytes_stream();
		let mut accumulator = OllamaStreamAccumulator::default();
		while let Some(next_chunk) = stream.next().await {
			let bytes = next_chunk.map_err(|error| {
				AppError::ProviderUnavailable(format!(
					"Streaming response from Ollama failed: {error}"
				))
			})?;
			accumulator.push(&bytes, on_chunk.as_mut())?;
		}
		accumulator.finish(on_chunk.as_mut())
	}
}

#[derive(Default)]
struct OllamaStreamAccumulator {
	buffer: Vec<u8>,
	content: String,
	last_chunk: Option<OllamaChatResponse>,
}

impl OllamaStreamAccumulator {
	fn push(
		&mut self,
		bytes: &[u8],
		on_chunk: &mut (dyn FnMut(ChatStreamChunk) -> Result<(), AppError> + Send),
	) -> Result<(), AppError> {
		self.buffer.extend_from_slice(bytes);
		while let Some(newline_index) = self.buffer.iter().position(|&byte| byte == b'\n') {
			let line_bytes: Vec<u8> = self.buffer.drain(..=newline_index).collect();
			self.consume_line(&line_bytes, on_chunk)?;
		}
		Ok(())
	}

	fn consume_line(
		&mut self,
		line_bytes: &[u8],
		on_chunk: &mut (dyn FnMut(ChatStreamChunk) -> Result<(), AppError> + Send),
	) -> Result<(), AppError> {
		let raw_line = std::str::from_utf8(line_bytes).map_err(|error| {
			AppError::ProviderProtocol(format!(
				"Could not decode Ollama stream line as UTF-8: {error}"
			))
		})?;
		let trimmed_line = raw_line.trim();
		if trimmed_line.is_empty() {
			return Ok(());
		}
		let parsed_chunk = parse_stream_line(trimmed_line)?;
		emit_chunk(&parsed_chunk, &mut self.content, on_chunk)?;
		self.last_chunk = Some(parsed_chunk);
		Ok(())
	}

	fn finish(
		mut self,
		on_chunk: &mut (dyn FnMut(ChatStreamChunk) -> Result<(), AppError> + Send),
	) -> Result<ChatResponse, AppError> {
		if !self.buffer.is_empty() {
			let trailing = std::mem::take(&mut self.buffer);
			self.consume_line(&trailing, on_chunk)?;
		}
		let final_chunk = self.last_chunk.ok_or_else(|| {
			AppError::ProviderProtocol("Ollama returned an empty stream response.".to_string())
		})?;
		let role = parse_chat_role(&final_chunk.message.role)?;
		Ok(ChatResponse {
			provider: OLLAMA_PROVIDER_ID.to_string(),
			model: final_chunk.model,
			message: ChatMessage {
				role,
				content: self.content,
				tool_calls: final_chunk.message.tool_calls,
				tool_name: final_chunk.message.tool_name,
			},
			done: final_chunk.done,
			done_reason: final_chunk.done_reason,
			created_at: final_chunk.created_at,
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
		"tool" => Ok(ChatRole::Tool),
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

fn map_ollama_response(payload: OllamaChatResponse) -> Result<ChatResponse, AppError> {
	let role = parse_chat_role(&payload.message.role)?;

	Ok(ChatResponse {
		provider: OLLAMA_PROVIDER_ID.to_string(),
		model: payload.model,
		message: ChatMessage {
			role,
			content: payload.message.content,
			tool_calls: payload.message.tool_calls,
			tool_name: payload.message.tool_name,
		},
		done: payload.done,
		done_reason: payload.done_reason,
		created_at: payload.created_at,
	})
}

fn parse_stream_line(line: &str) -> Result<OllamaChatResponse, AppError> {
	serde_json::from_str(line).map_err(|error| {
		AppError::ProviderProtocol(format!("Could not parse stream chunk: {error}"))
	})
}

fn emit_chunk(
	chunk: &OllamaChatResponse,
	accumulated_content: &mut String,
	on_chunk: &mut (dyn FnMut(ChatStreamChunk) -> Result<(), AppError> + Send),
) -> Result<(), AppError> {
	let delta = chunk.message.content.clone();
	accumulated_content.push_str(&delta);

	on_chunk(ChatStreamChunk {
		provider: OLLAMA_PROVIDER_ID.to_string(),
		model: chunk.model.clone(),
		delta,
		done: chunk.done,
		done_reason: chunk.done_reason.clone(),
		created_at: chunk.created_at.clone(),
	})
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
	#[serde(skip_serializing_if = "Vec::is_empty")]
	tools: Vec<ToolDefinition>,
	#[serde(skip_serializing_if = "Option::is_none")]
	options: Option<OllamaChatOptions>,
}

#[derive(Debug, Serialize)]
struct OllamaChatMessage {
	role: String,
	content: String,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	tool_calls: Vec<ToolCall>,
	#[serde(skip_serializing_if = "Option::is_none")]
	tool_name: Option<String>,
}

impl From<ChatMessage> for OllamaChatMessage {
	fn from(value: ChatMessage) -> Self {
		let role = match value.role {
			ChatRole::System => "system",
			ChatRole::User => "user",
			ChatRole::Assistant => "assistant",
			ChatRole::Tool => "tool",
		};
		Self {
			role: role.to_string(),
			content: value.content,
			tool_calls: value.tool_calls,
			tool_name: value.tool_name,
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
	#[serde(default)]
	content: String,
	#[serde(default)]
	tool_calls: Vec<ToolCall>,
	#[serde(default)]
	tool_name: Option<String>,
}

#[cfg(test)]
mod tests {
	use super::{
		map_models, map_ollama_response, resolve_base_url, OllamaChatResponse, OllamaProvider,
		OllamaStreamAccumulator, OllamaTagsResponse,
	};
	use crate::providers::{ChatProvider, ChatStreamChunk};

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

	#[test]
	fn maps_tool_calls_from_ollama_response() {
		let payload: OllamaChatResponse = serde_json::from_str(
			r#"{
				"model":"qwen3",
				"message":{
					"role":"assistant",
					"content":"",
					"tool_calls":[{
						"type":"function",
						"function":{"index":0,"name":"search_web","arguments":{"query":"Taurus app"}}
					}]
				},
				"done":true,
				"done_reason":"stop"
			}"#,
		)
		.expect("tool response should parse");
		let response = map_ollama_response(payload).expect("tool response should map");
		assert_eq!(response.message.tool_calls.len(), 1);
		assert_eq!(response.message.tool_calls[0].function.index, Some(0));
		assert_eq!(response.message.tool_calls[0].function.name, "search_web");
		assert_eq!(
			response.message.tool_calls[0].function.arguments["query"],
			"Taurus app"
		);
	}

	#[test]
	fn stream_parsing_handles_fragmented_utf8_and_multiple_lines() {
		let _chat_stream_ref = <OllamaProvider as ChatProvider>::chat_stream;
		let first_line = r#"{"model":"llama3.1:8b","created_at":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":"Olá "},"done":false,"done_reason":null}"#;
		let second_line = r#"{"model":"llama3.1:8b","created_at":"2026-01-01T00:00:01Z","message":{"role":"assistant","content":"世界"},"done":true,"done_reason":"stop"}"#;
		let payload = format!("{first_line}\n{second_line}\n");
		let bytes = payload.as_bytes();
		let split_index = bytes
			.iter()
			.position(|byte| *byte == 0xC3)
			.expect("payload should contain a multi-byte UTF-8 lead byte");
		let first_newline_index = bytes
			.iter()
			.position(|byte| *byte == b'\n')
			.expect("payload should contain a newline");
		let chunks: Vec<&[u8]> = vec![
			&bytes[..split_index + 1],
			&bytes[split_index + 1..first_newline_index - 2],
			&bytes[first_newline_index - 2..first_newline_index + 1],
			&bytes[first_newline_index + 1..],
		];
		let mut emitted_chunks: Vec<ChatStreamChunk> = Vec::new();
		let mut on_chunk = |chunk: ChatStreamChunk| -> Result<(), crate::error::AppError> {
			emitted_chunks.push(chunk);
			Ok(())
		};
		let mut accumulator = OllamaStreamAccumulator::default();
		for chunk in chunks {
			accumulator
				.push(chunk, &mut on_chunk)
				.expect("stream chunk should accumulate");
		}
		let final_response = accumulator
			.finish(&mut on_chunk)
			.expect("stream should finish");
		assert_eq!(final_response.message.content, "Olá 世界");
		assert_eq!(emitted_chunks.len(), 2);
		assert_eq!(emitted_chunks[0].delta, "Olá ");
		assert_eq!(emitted_chunks[1].delta, "世界");
	}
}
