#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod api;
mod dto;
mod sheet;
mod state;

use state::AppState;

fn main() {
    tauri::Builder::default()
        .manage(AppState(Default::default()))
        .invoke_handler(tauri::generate_handler![
            // App
            api::new_document,
            api::open_documents,
            api::focus_document,
            api::close_document,
            api::save_current_document,
            api::focus_content_tab,
            // Document
            api::clear_selection,
            api::select_frame,
            api::select_animation,
            api::pan,
            api::edit_animation,
            api::rename_animation,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
