use std::{collections::HashMap, sync::Arc};

use crate::{
	error::AppError,
	providers::{ollama::OllamaProvider, ChatProvider, OLLAMA_PROVIDER_ID},
};

pub struct AppState {
	providers: HashMap<String, Arc<dyn ChatProvider>>,
}

impl AppState {
	pub fn new() -> Result<Self, AppError> {
		let ollama_provider = Arc::new(OllamaProvider::from_env()?);

		let mut providers: HashMap<String, Arc<dyn ChatProvider>> = HashMap::new();
		providers.insert(OLLAMA_PROVIDER_ID.to_string(), ollama_provider);

		Ok(Self { providers })
	}

	pub fn provider(&self, provider_id: Option<&str>) -> Result<Arc<dyn ChatProvider>, AppError> {
		let normalized_provider_id = provider_id
			.map(str::trim)
			.filter(|value| !value.is_empty())
			.unwrap_or(OLLAMA_PROVIDER_ID)
			.to_ascii_lowercase();

		self.providers
			.get(&normalized_provider_id)
			.cloned()
			.ok_or(AppError::ProviderNotFound(normalized_provider_id))
	}
}
