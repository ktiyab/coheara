pub mod api; // M0-01: Mobile API Router
pub mod commands;
pub mod config;
pub mod core_state; // ME-01: Transport-agnostic state
pub mod device_manager; // ME-02: Multi-Device Session Manager
pub mod pairing; // M0-02: Device Pairing Protocol
pub mod tls_cert; // M0-02: TLS Certificate Management
pub mod models;
pub mod db;
pub mod crypto;
pub mod pipeline;
pub mod intelligence; // L2-01 through L2-03: RAG + Safety + Coherence
pub mod home; // L3-02: Home & Document Feed
pub mod chat; // L3-03: Chat Interface
pub mod review; // L3-04: Review Screen
pub mod medications; // L3-05: Medication List
pub mod journal; // L4-01: Symptom Journal
pub mod appointment; // L4-02: Appointment Prep
pub mod wifi_transfer; // L4-03: WiFi Transfer
pub mod distribution; // ADS: App Distribution Server
pub mod timeline; // L4-04: Timeline View
pub mod sync; // M0-04: Sync Engine
pub mod trust; // L5-01: Trust & Safety

use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(config::default_log_filter())),
        )
        .init();

    tracing::info!("Coheara starting v{}", config::APP_VERSION);

    // SEC-02-G08: Clean orphaned staging files from previous crashes
    crypto::cleanup_orphaned_staging(&config::profiles_dir());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Arc::new(core_state::CoreState::new()))
        .setup(|app| {
            // LP-01: Start background batch extraction scheduler
            pipeline::batch_extraction::background::start_background_scheduler(
                app.handle().clone(),
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health_check,
            commands::check_ai_status,
            commands::verify_ai_status,
            commands::profile::list_profiles,
            commands::profile::create_profile,
            commands::profile::unlock_profile,
            commands::profile::lock_profile,
            commands::profile::change_profile_password,
            commands::profile::recover_profile,
            commands::profile::is_profile_active,
            commands::profile::get_active_profile_name,
            commands::profile::get_active_profile_info,
            commands::profile::get_caregiver_summaries,
            commands::profile::delete_profile,
            commands::profile::check_inactivity,
            commands::profile::update_activity,
            commands::home::get_home_data,
            commands::home::get_more_documents,
            commands::home::get_document_detail,
            commands::home::dismiss_alert,
            commands::home::search_documents,
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
            commands::medications::get_medications,
            commands::medications::get_medication_detail,
            commands::medications::add_otc_medication,
            commands::medications::get_dose_history,
            commands::medications::search_medication_alias,
            commands::journal::record_symptom,
            commands::journal::get_symptom_history,
            commands::journal::resolve_symptom,
            commands::journal::delete_symptom,
            commands::journal::check_journal_nudge,
            commands::journal::get_symptom_categories,
            commands::appointment::list_professionals,
            commands::appointment::prepare_appointment,
            commands::appointment::export_prep_pdf,
            commands::appointment::save_appointment_notes,
            commands::appointment::list_appointments,
            commands::timeline::get_timeline_data,
            commands::transfer::start_wifi_transfer,
            commands::transfer::stop_wifi_transfer,
            commands::transfer::get_transfer_status,
            commands::transfer::process_staged_files,
            commands::trust::get_critical_alerts,
            commands::trust::dismiss_critical,
            commands::trust::check_dose,
            commands::trust::create_backup,
            commands::trust::preview_backup_file,
            commands::trust::restore_from_backup,
            commands::trust::erase_profile_data,
            commands::trust::get_privacy_info_cmd,
            commands::trust::open_data_folder,
            commands::trust::check_data_consistency,
            commands::trust::repair_data_consistency,
            commands::devices::list_paired_devices,
            commands::devices::unpair_device,
            commands::devices::get_device_count,
            commands::devices::get_inactive_warnings,
            commands::pairing::start_pairing,
            commands::pairing::cancel_pairing,
            commands::pairing::get_pending_approval,
            commands::pairing::approve_pairing,
            commands::pairing::deny_pairing,
            commands::distribution::start_distribution,
            commands::distribution::stop_distribution,
            commands::distribution::get_distribution_status,
            commands::distribution::get_install_qr,
            commands::import::import_document,
            commands::import::import_documents_batch,
            commands::import::process_document,
            commands::import::process_documents_batch,
            commands::import::delete_document,
            commands::import::reprocess_document,
            commands::coherence::run_coherence_scan,
            commands::coherence::run_coherence_scan_document,
            commands::coherence::get_coherence_alerts,
            commands::coherence::dismiss_coherence_alert,
            commands::coherence::dismiss_critical_coherence_alert,
            commands::coherence::get_coherence_emergency_actions,
            commands::mobile_api::start_mobile_api,
            commands::mobile_api::stop_mobile_api,
            commands::mobile_api::get_mobile_api_status,
            commands::sync::get_sync_versions,
            commands::sync::reset_sync_versions,
            commands::sync::get_sync_summary,
            // L6-01: Ollama Integration
            commands::ai_setup::ollama_health_check,
            commands::ai_setup::list_ollama_models,
            commands::ai_setup::show_ollama_model,
            commands::ai_setup::pull_ollama_model,
            commands::ai_setup::cancel_model_pull,
            commands::ai_setup::delete_ollama_model,
            commands::ai_setup::get_recommended_models,
            // L6-04: Model Preferences
            commands::ai_setup::set_active_model,
            commands::ai_setup::get_active_model,
            commands::ai_setup::clear_active_model,
            commands::ai_setup::set_user_preference_cmd,
            commands::ai_setup::get_user_preference_cmd,
            // L6-03: AI Setup Wizard
            commands::ai_setup::verify_ai_model,
            // LP-01: Night Batch Extraction Pipeline
            commands::extraction::get_pending_extractions,
            commands::extraction::get_pending_extraction_count,
            commands::extraction::confirm_extraction,
            commands::extraction::confirm_extraction_with_edits,
            commands::extraction::dismiss_extraction,
            commands::extraction::dismiss_all_extractions,
            commands::extraction::trigger_extraction_batch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Coheara");
}
