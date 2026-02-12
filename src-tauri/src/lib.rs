pub mod commands;
pub mod config;
pub mod models;
pub mod db;
pub mod crypto;
pub mod pipeline;
pub mod intelligence; // L2-01 through L2-03: RAG + Safety + Coherence
pub mod home; // L3-02: Home & Document Feed
pub mod chat; // L3-03: Chat Interface
pub mod review; // L3-04: Review Screen
// pub mod export;      // L4-02: Appointment Prep PDF
// pub mod transfer;    // L4-03: WiFi Transfer

use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("coheara=info")),
        )
        .init();

    tracing::info!("Coheara starting v{}", config::APP_VERSION);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(commands::state::AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::health_check,
            commands::profile::list_profiles,
            commands::profile::create_profile,
            commands::profile::unlock_profile,
            commands::profile::lock_profile,
            commands::profile::recover_profile,
            commands::profile::is_profile_active,
            commands::profile::delete_profile,
            commands::profile::check_inactivity,
            commands::profile::update_activity,
            commands::home::get_home_data,
            commands::home::get_more_documents,
            commands::home::dismiss_alert,
            commands::chat::start_conversation,
            commands::chat::send_chat_message,
            commands::chat::get_conversation_messages,
            commands::chat::list_conversations,
            commands::chat::delete_conversation,
            commands::chat::set_message_feedback,
            commands::chat::get_prompt_suggestions,
            commands::review::get_review_data,
            commands::review::get_original_file,
            commands::review::update_extracted_field,
            commands::review::confirm_review,
            commands::review::reject_review,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Coheara");
}
