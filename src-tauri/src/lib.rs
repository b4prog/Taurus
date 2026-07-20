mod agent;
mod app_state;
mod commands;
mod error;
mod providers;
mod tools;

use app_state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
	let app_state = match AppState::new() {
		Ok(state) => state,
		Err(error) => {
			eprintln!("failed to initialize Taurus app state: {error}");
			return;
		}
	};

	let builder =
		tauri::Builder::default()
			.manage(app_state)
			.invoke_handler(tauri::generate_handler![
				commands::health::check_ollama_health,
				commands::models::list_ollama_models,
				commands::chat::send_chat_message,
				commands::chat::send_chat_message_stream
			]);

	if let Err(error) = builder.run(tauri::generate_context!()) {
		eprintln!("error while running Taurus application: {error}");
	}
}
