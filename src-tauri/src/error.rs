use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
	#[error("Validation error: {0}")]
	Validation(String),
	#[error("Unknown provider: {0}")]
	ProviderNotFound(String),
	#[error("Provider is unavailable: {0}")]
	ProviderUnavailable(String),
	#[error("Provider request failed")]
	HttpClient(#[from] reqwest::Error),
	#[error("Provider returned invalid data: {0}")]
	ProviderProtocol(String),
	#[error("Invalid configuration: {0}")]
	Config(String),
	#[error("Internal stream delivery error: {0}")]
	EventEmit(String),
	#[error("Web tool failed: {0}")]
	WebTool(String),
}

#[derive(Debug, Serialize)]
pub struct ApiError {
	pub code: String,
	pub message: String,
}

impl From<AppError> for ApiError {
	fn from(error: AppError) -> Self {
		match error {
			AppError::Validation(message) => Self {
				code: "validation_error".to_string(),
				message,
			},
			AppError::ProviderNotFound(provider) => Self {
				code: "provider_not_found".to_string(),
				message: format!("Provider '{provider}' is not configured."),
			},
			AppError::ProviderUnavailable(message) => Self {
				code: "provider_unavailable".to_string(),
				message,
			},
			AppError::HttpClient(http_error) => api_http_client_error(http_error),
			AppError::ProviderProtocol(message) => Self {
				code: "provider_protocol_error".to_string(),
				message,
			},
			AppError::Config(message) => Self {
				code: "config_error".to_string(),
				message,
			},
			AppError::EventEmit(message) => Self {
				code: "stream_event_error".to_string(),
				message,
			},
			AppError::WebTool(message) => Self {
				code: "web_tool_error".to_string(),
				message,
			},
		}
	}
}

fn api_http_client_error(error: reqwest::Error) -> ApiError {
	let message = if error.is_connect() {
		"Could not reach the provider. Check that it is running and the URL is correct.".to_string()
	} else if error.is_timeout() {
		"The provider request timed out. Try again in a moment.".to_string()
	} else {
		"The provider request failed. Check provider logs for more details.".to_string()
	};
	ApiError {
		code: "provider_request_error".to_string(),
		message,
	}
}
