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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
	pub role: ChatRole,
	pub content: String,
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
}
