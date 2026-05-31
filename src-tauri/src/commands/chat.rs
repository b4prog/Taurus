use tauri::State;

use crate::{
	app_state::AppState,
	error::{ApiError, AppError},
	providers::ChatRequest,
};

#[tauri::command]
pub async fn send_chat_message(
	state: State<'_, AppState>,
	request: ChatRequest,
) -> Result<crate::providers::ChatResponse, ApiError> {
	let request = validate_chat_request(request).map_err(ApiError::from)?;

	let provider = state
		.provider(request.provider.as_deref())
		.map_err(ApiError::from)?;

	provider.chat(request).await.map_err(ApiError::from)
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

	if request.stream.unwrap_or(false) {
		return Err(AppError::Validation(
			"Streaming is planned but not implemented yet. Set 'stream' to false for now."
				.to_string(),
		));
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
	fn validate_rejects_streaming_for_now() {
		let mut request = valid_request();
		request.stream = Some(true);

		let error = validate_chat_request(request).expect_err("stream should fail");
		assert!(error.to_string().contains("not implemented"));
	}
}
