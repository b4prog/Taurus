use tauri::{ipc::Channel, State};

use crate::{
	agent::{run_agent_stream, AgentStepEvent},
	app_state::AppState,
	error::{ApiError, AppError},
	providers::{ChatMessage, ChatRequest, ChatResponse, ChatRole, ChatStreamChunk},
};

#[tauri::command]
pub async fn send_chat_message(
	state: State<'_, AppState>,
	request: ChatRequest,
) -> Result<ChatResponse, ApiError> {
	let mut request = validate_chat_request(request).map_err(ApiError::from)?;
	request.stream = Some(false);

	let provider = state
		.provider(request.provider.as_deref())
		.map_err(ApiError::from)?;

	provider.chat(request).await.map_err(ApiError::from)
}

#[tauri::command]
pub async fn send_chat_message_stream(
	state: State<'_, AppState>,
	request: ChatRequest,
	on_chunk: Channel<ChatStreamChunk>,
	on_step: Channel<AgentStepEvent>,
) -> Result<ChatResponse, ApiError> {
	let mut request = validate_chat_request(request).map_err(ApiError::from)?;
	request.stream = Some(true);
	let provider = state
		.provider(request.provider.as_deref())
		.map_err(ApiError::from)?;
	let tool_executor = state.tool_executor();
	run_agent_stream(
		provider,
		tool_executor,
		request,
		Box::new(move |chunk| {
			on_chunk.send(chunk).map_err(|error| {
				AppError::EventEmit(format!(
					"Failed to send stream chunk over IPC channel: {error}"
				))
			})
		}),
		Box::new(move |step| {
			on_step.send(step).map_err(|error| {
				AppError::EventEmit(format!(
					"Failed to send agent step over IPC channel: {error}"
				))
			})
		}),
	)
	.await
	.map_err(ApiError::from)
}

fn validate_chat_request(mut request: ChatRequest) -> Result<ChatRequest, AppError> {
	request.model = validate_model(&request.model)?;
	validate_messages(&mut request.messages)?;
	request.provider = normalize_provider(request.provider);
	validate_temperature(request.temperature)?;
	Ok(request)
}

fn validate_model(model: &str) -> Result<String, AppError> {
	let model = model.trim();
	if model.is_empty() {
		return Err(AppError::Validation(
			"A model is required for chat requests.".to_string(),
		));
	}
	Ok(model.to_string())
}

fn validate_messages(messages: &mut [ChatMessage]) -> Result<(), AppError> {
	if messages.is_empty() {
		return Err(AppError::Validation(
			"At least one chat message is required.".to_string(),
		));
	}
	for message in messages {
		validate_message(message)?;
	}
	Ok(())
}

fn validate_message(message: &mut ChatMessage) -> Result<(), AppError> {
	if message.role == ChatRole::Tool
		|| !message.tool_calls.is_empty()
		|| message.tool_name.is_some()
	{
		return Err(AppError::Validation(
			"Tool messages and tool calls are managed internally by the agent.".to_string(),
		));
	}
	message.content = message.content.trim().to_string();
	if message.content.is_empty() {
		return Err(AppError::Validation(
			"Message content cannot be empty.".to_string(),
		));
	}
	Ok(())
}

fn normalize_provider(provider: Option<String>) -> Option<String> {
	provider
		.map(|value| value.trim().to_ascii_lowercase())
		.filter(|value| !value.is_empty())
}

fn validate_temperature(temperature: Option<f32>) -> Result<(), AppError> {
	if temperature.is_some_and(|value| !(0.0..=2.0).contains(&value)) {
		return Err(AppError::Validation(
			"Temperature must be between 0.0 and 2.0.".to_string(),
		));
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::validate_chat_request;
	use crate::providers::{ChatMessage, ChatRequest, ChatRole};

	fn valid_request() -> ChatRequest {
		ChatRequest {
			provider: Some("ollama".to_string()),
			model: "llama3".to_string(),
			messages: vec![ChatMessage {
				role: ChatRole::User,
				content: "Hello".to_string(),
				tool_calls: Vec::new(),
				tool_name: None,
			}],
			temperature: Some(0.7),
			stream: Some(false),
		}
	}

	#[test]
	fn validate_rejects_empty_model() {
		let mut request = valid_request();
		request.model = "  ".to_string();

		let error = validate_chat_request(request).expect_err("empty model should fail");
		assert!(error.to_string().contains("model is required"));
	}

	#[test]
	fn validate_accepts_stream_flag() {
		let mut request = valid_request();
		request.stream = Some(true);

		let result = validate_chat_request(request);
		assert!(result.is_ok());
	}

	#[test]
	fn validate_rejects_temperature_out_of_range() {
		let mut low_request = valid_request();
		low_request.temperature = Some(-0.1);
		let low_error =
			validate_chat_request(low_request).expect_err("temperature below range should fail");
		assert!(low_error.to_string().contains("between 0.0 and 2.0"));

		let mut high_request = valid_request();
		high_request.temperature = Some(2.1);
		let high_error =
			validate_chat_request(high_request).expect_err("temperature above range should fail");
		assert!(high_error.to_string().contains("between 0.0 and 2.0"));
	}

	#[test]
	fn validate_rejects_empty_message_content() {
		let mut request = valid_request();
		request.messages = vec![ChatMessage {
			role: ChatRole::User,
			content: "   ".to_string(),
			tool_calls: Vec::new(),
			tool_name: None,
		}];

		let error = validate_chat_request(request)
			.expect_err("whitespace-only message content should fail");
		assert!(error
			.to_string()
			.contains("Message content cannot be empty"));
	}

	#[test]
	fn validate_rejects_frontend_tool_messages() {
		let mut request = valid_request();
		request.messages[0].role = ChatRole::Tool;
		request.messages[0].tool_name = Some("fetch_web_page".to_string());
		let error = validate_chat_request(request).expect_err("tool messages should fail");
		assert!(error.to_string().contains("managed internally"));
	}

	#[test]
	fn validate_normalizes_and_clears_provider() {
		let mut mixed_case_provider_request = valid_request();
		mixed_case_provider_request.provider = Some("  OLLama  ".to_string());
		let normalized = validate_chat_request(mixed_case_provider_request)
			.expect("mixed-case provider should normalize");
		assert_eq!(normalized.provider.as_deref(), Some("ollama"));

		let mut empty_provider_request = valid_request();
		empty_provider_request.provider = Some("   ".to_string());
		let cleared =
			validate_chat_request(empty_provider_request).expect("empty provider should clear");
		assert!(cleared.provider.is_none());
	}
}
