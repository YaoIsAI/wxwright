//! wxwright GUI (Tauri 2): thin shell over wxwright-core. All correctness
//! logic lives in the engine crate; this host only marshals UI calls.

mod ai;
mod articles;
mod comfy;
mod commands;
mod extract;
mod jobs;
mod net;
mod social;

use tauri::{DragDropEvent, Emitter, WindowEvent};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_themes,
            commands::convert_preview,
            commands::copy_rich,
            commands::export_html,
            commands::read_text_file,
            commands::load_sample,
            commands::agent_card_markdown,
            commands::copy_agent_card,
            commands::mcp_install,
            commands::list_articles,
            commands::library_dir,
            commands::read_article,
            commands::save_article,
            commands::delete_article,
            commands::ai_settings,
            commands::ai_save_provider,
            commands::ai_save_provider_with_key,
            commands::ai_delete_provider,
            commands::ai_set_active,
            commands::ai_test,
            commands::ai_chat,
            commands::ai_stop,
            commands::write_file_base64,
            commands::list_assets,
            commands::asset_data_uri,
            commands::asset_thumb,
            commands::delete_asset,
            commands::extract_document_text,
            commands::read_binary_file,
            commands::comfy_status,
            commands::comfy_save_config,
            commands::comfy_launch,
            commands::wx_bind_status,
            commands::wx_draft_list,
            commands::wx_draft_delete,
            commands::wx_bind,
            commands::wx_unbind,
            commands::ai_job_start,
            commands::ai_job_stop,
            commands::list_platforms,
            commands::platform_export_text,
            commands::platform_validate,
            commands::platform_preview,
            commands::ai_save_image_model,
            commands::import_image_from_path,
            commands::import_image_bytes,
            commands::copy_text_plain,
            commands::wx_push_draft,
            social::social_bind_status,
            social::social_save_config,
            social::social_unbind,
            social::social_oauth_start,
            social::social_oauth_cancel,
            social::social_post_article,
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::DragDrop(DragDropEvent::Drop { paths, .. }) = event {
                let names: Vec<String> = paths
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();
                let _ = window.emit("dropped-files", names);
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running wxwright");
}
