pub mod web;

use async_trait::async_trait;

use crate::{
	error::AppError,
	providers::{ToolCall, ToolDefinition},
};

#[derive(Debug)]
pub struct ToolExecution {
	pub content: String,
	pub detail: String,
	pub follow_up_calls: Vec<ToolCall>,
}

#[async_trait]
pub trait ToolExecutor: Send + Sync {
	fn definitions(&self) -> Vec<ToolDefinition>;
	fn describe(&self, call: &ToolCall) -> (String, String);
	async fn execute(&self, call: &ToolCall) -> Result<ToolExecution, AppError>;
}
