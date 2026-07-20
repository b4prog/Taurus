use tauri::State;

use crate::{
	app_state::AppState,
	error::ApiError,
	providers::{ProviderHealth, OLLAMA_PROVIDER_ID},
};

#[tauri::command]
pub async fn check_ollama_health(state: State<'_, AppState>) -> Result<ProviderHealth, ApiError> {
	let provider = state
		.provider(Some(OLLAMA_PROVIDER_ID))
		.map_err(ApiError::from)?;

	provider.health_check().await.map_err(ApiError::from)
}
