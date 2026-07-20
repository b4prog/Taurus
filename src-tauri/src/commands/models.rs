use tauri::State;

use crate::{
	app_state::AppState,
	error::ApiError,
	providers::{ModelInfo, OLLAMA_PROVIDER_ID},
};

#[tauri::command]
pub async fn list_ollama_models(state: State<'_, AppState>) -> Result<Vec<ModelInfo>, ApiError> {
	let provider = state
		.provider(Some(OLLAMA_PROVIDER_ID))
		.map_err(ApiError::from)?;

	provider.list_models().await.map_err(ApiError::from)
}
