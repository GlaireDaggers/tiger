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
            api::open_documents,
            api::focus_document,
            api::close_document,
            api::save_current_document,
            api::focus_content_tab,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
