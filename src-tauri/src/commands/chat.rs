use tauri::{ipc::Channel, State};

use crate::{
	app_state::AppState,
	error::{ApiError, AppError},
	providers::{ChatRequest, ChatResponse, ChatStreamChunk},
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
) -> Result<ChatResponse, ApiError> {
	let mut request = validate_chat_request(request).map_err(ApiError::from)?;
	request.stream = Some(true);

	let provider = state
		.provider(request.provider.as_deref())
		.map_err(ApiError::from)?;

	provider
		.chat_stream(
			request,
			Box::new(move |chunk| {
				on_chunk.send(chunk).map_err(|error| {
					AppError::EventEmit(format!(
						"Failed to send stream chunk over IPC channel: {error}"
					))
				})
			}),
		)
		.await
		.map_err(ApiError::from)
}

fn validate_chat_request(mut request: ChatRequest) -> Result<ChatRequest, AppError> {
	request.model = request.model.trim().to_string();
	if request.model.is_empty() {
		return Err(AppError::Validation(
			"A model is required for chat requests.".to_string(),
		));
	}

	if request.messages.is_empty() {
		return Err(AppError::Validation(
			"At least one chat message is required.".to_string(),
		));
	}

	for message in &mut request.messages {
		message.content = message.content.trim().to_string();
		if message.content.is_empty() {
			return Err(AppError::Validation(
				"Message content cannot be empty.".to_string(),
			));
		}
	}

	if let Some(provider) = &mut request.provider {
		*provider = provider.trim().to_ascii_lowercase();
		if provider.is_empty() {
			request.provider = None;
		}
	}

	if let Some(temperature) = request.temperature {
		if !(0.0..=2.0).contains(&temperature) {
			return Err(AppError::Validation(
				"Temperature must be between 0.0 and 2.0.".to_string(),
			));
		}
	}

	Ok(request)
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
		}];

		let error = validate_chat_request(request)
			.expect_err("whitespace-only message content should fail");
		assert!(error
			.to_string()
			.contains("Message content cannot be empty"));
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
