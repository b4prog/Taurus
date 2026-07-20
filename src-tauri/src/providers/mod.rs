pub mod ollama;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const OLLAMA_PROVIDER_ID: &str = "ollama";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
	System,
	User,
	Assistant,
	Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
	pub role: ChatRole,
	pub content: String,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub tool_calls: Vec<ToolCall>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub tool_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCall {
	#[serde(rename = "type", default = "function_tool_type")]
	pub tool_type: String,
	pub function: ToolFunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolFunctionCall {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub index: Option<usize>,
	pub name: String,
	pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
	#[serde(rename = "type")]
	pub tool_type: String,
	pub function: ToolFunctionDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunctionDefinition {
	pub name: String,
	pub description: String,
	pub parameters: serde_json::Value,
}

fn function_tool_type() -> String {
	"function".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
	pub provider: Option<String>,
	pub model: String,
	pub messages: Vec<ChatMessage>,
	pub temperature: Option<f32>,
	pub stream: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
	pub provider: String,
	pub model: String,
	pub message: ChatMessage,
	pub done: bool,
	pub done_reason: Option<String>,
	pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatStreamChunk {
	pub provider: String,
	pub model: String,
	pub delta: String,
	pub done: bool,
	pub done_reason: Option<String>,
	pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
	pub provider: String,
	pub healthy: bool,
	pub base_url: String,
	pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
	pub provider: String,
	pub id: String,
	pub display_name: String,
	pub size_bytes: Option<u64>,
	pub modified_at: Option<String>,
}

#[async_trait]
pub trait ChatProvider: Send + Sync {
	async fn health_check(&self) -> Result<ProviderHealth, AppError>;
	async fn list_models(&self) -> Result<Vec<ModelInfo>, AppError>;
	async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, AppError>;
	async fn chat_with_tools(
		&self,
		request: ChatRequest,
		tools: Vec<ToolDefinition>,
	) -> Result<ChatResponse, AppError>;
	async fn chat_stream(
		&self,
		request: ChatRequest,
		on_chunk: Box<dyn FnMut(ChatStreamChunk) -> Result<(), AppError> + Send>,
	) -> Result<ChatResponse, AppError>;
}
