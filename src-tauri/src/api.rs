use async_trait::async_trait;
use json_patch::Patch;
use log::error;
use std::path::PathBuf;
use std::time::Duration;
use tauri::ClipboardManager;

use crate::app::{App, AppState};
use crate::document::{Command, Document, DocumentResult};
use crate::dto::{self, AppTrim, ToFileName};
use crate::export::export_sheet;
use crate::features::texture_cache;
use crate::sheet::{Absolute, Sheet};
use crate::utils::handle::Handle;
use crate::TigerApp;

impl AppState {
    pub fn mutate<F>(&self, app_trim: AppTrim, operation: F) -> Patch
    where
        F: FnOnce(&mut App),
    {
        let mut app = self.0.lock();

        let old_state: dto::App = app.to_dto(app_trim);
        operation(&mut app);
        let new_state: dto::App = app.to_dto(app_trim);

        let old_json = serde_json::to_value(old_state);
        let new_json = serde_json::to_value(new_state);

        match (old_json, new_json) {
            (Ok(o), Ok(n)) => json_patch::diff(&o, &n),
            _ => {
                error!("App state serialization error");
                Patch(Vec::new())
            }
        }
    }
}

#[async_trait]
pub trait Api {
    fn delete_frame(&self, path: PathBuf) -> Result<Patch, ()>;
    async fn export(&self) -> Result<Patch, ()>;
    fn import_frames(&self, paths: Vec<PathBuf>) -> Result<Patch, ()>;
    fn new_document(&self, path: PathBuf) -> Result<Patch, ()>;
    async fn open_documents(&self, paths: Vec<PathBuf>) -> Result<Patch, ()>;
}

#[async_trait]
impl<T: TigerApp + Sync> Api for T {
    fn delete_frame(&self, path: PathBuf) -> Result<Patch, ()> {
        Ok(self.app_state().mutate(AppTrim::Full, |app| {
            if let Some(document) = app.current_document_mut() {
                document.process_command(Command::DeleteFrame(path)).ok();
            }
        }))
    }

    async fn export(&self) -> Result<Patch, ()> {
        let (sheet, document_name) = {
            let app_state = self.app_state();
            let app = app_state.0.lock();
            match app.current_document() {
                Some(d) => (d.sheet().clone(), d.path().to_file_name()),
                _ => return Ok(Patch(Vec::new())),
            }
        };

        match tauri::async_runtime::spawn_blocking({
            let texture_cache = self.texture_cache();
            move || export_sheet(&sheet, texture_cache)
        })
        .await
        .unwrap()
        {
            Ok(_) => Ok(Patch(Vec::new())),
            Err(e) => Ok(self.app_state().mutate(AppTrim::Full, |app| {
                app.show_error_message(
                    "Export Error".to_owned(),
                    format!(
                        "An error occured while trying to export `{}`",
                        document_name.to_file_name(),
                    ),
                    e.to_string(),
                )
            })),
        }
    }

    fn import_frames(&self, paths: Vec<PathBuf>) -> Result<Patch, ()> {
        Ok(self.app_state().mutate(AppTrim::Full, |app| {
            if let Some(document) = app.current_document_mut() {
                document.process_command(Command::ImportFrames(paths)).ok();
            }
        }))
    }

    fn new_document(&self, path: PathBuf) -> Result<Patch, ()> {
        Ok(self.app_state().mutate(AppTrim::Full, |app| {
            app.new_document(path);
        }))
    }

    async fn open_documents(&self, paths: Vec<PathBuf>) -> Result<Patch, ()> {
        let mut documents: Vec<(PathBuf, DocumentResult<Document>)> = Vec::new();
        for path in &paths {
            let open_path = path.to_owned();
            documents.push((
                open_path.clone(),
                tauri::async_runtime::spawn_blocking(move || Document::open(open_path))
                    .await
                    .unwrap(),
            ));
        }

        Ok(self.app_state().mutate(AppTrim::Full, |app| {
            for document in documents {
                match document {
                    (_, Ok(d)) => {
                        app.open_document(d);
                    }
                    (path, Err(e)) => {
                        app.show_error_message(
                            "Error".to_owned(),
                            format!(
                                "An error occured while trying to open `{}`",
                                path.to_file_name()
                            ),
                            e.to_string(),
                        );
                    }
                }
            }
        }))
    }
}

#[tauri::command]
pub fn get_state(app_state: tauri::State<'_, AppState>) -> Result<dto::App, ()> {
    let app = app_state.0.lock();
    Ok(app.to_dto(AppTrim::Full))
}

#[tauri::command]
pub fn show_error_message(
    app_state: tauri::State<'_, AppState>,
    title: String,
    summary: String,
    details: String,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        app.show_error_message(title, summary, details);
    }))
}

#[tauri::command]
pub fn acknowledge_error(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        app.acknowledge_error();
    }))
}

#[tauri::command]
pub fn new_document(app: tauri::AppHandle, path: PathBuf) -> Result<Patch, ()> {
    app.new_document(path)
}

#[tauri::command]
pub async fn open_documents(app: tauri::AppHandle, paths: Vec<PathBuf>) -> Result<Patch, ()> {
    app.open_documents(paths).await
}

#[tauri::command]
pub fn focus_document(app_state: tauri::State<'_, AppState>, path: PathBuf) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        app.focus_document(&path).ok();
    }))
}

#[tauri::command]
pub fn close_document(
    window: tauri::Window,
    app_state: tauri::State<'_, AppState>,
    path: PathBuf,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.document_mut(&path) {
            document.request_close();
        }
        app.advance_exit();
        if app.should_exit() {
            window.close().ok();
        }
    }))
}

#[tauri::command]
pub fn close_current_document(
    window: tauri::Window,
    app_state: tauri::State<'_, AppState>,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.request_close();
        }
        app.advance_exit();
        if app.should_exit() {
            window.close().ok();
        }
    }))
}

#[tauri::command]
pub fn close_all_documents(
    window: tauri::Window,
    app_state: tauri::State<'_, AppState>,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        for document in app.documents_iter_mut() {
            document.request_close();
        }
        app.advance_exit();
        if app.should_exit() {
            window.close().ok();
        }
    }))
}

#[tauri::command]
pub fn request_exit(
    window: tauri::Window,
    app_state: tauri::State<'_, AppState>,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        app.request_exit();
        if app.should_exit() {
            window.close().ok();
        }
    }))
}

#[tauri::command]
pub fn cancel_exit(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        app.cancel_exit();
    }))
}

#[tauri::command]
pub fn reveal_in_explorer(path: PathBuf) {
    // For future improvements, see https://github.com/tauri-apps/tauri/issues/4062
    #[cfg(windows)]
    std::process::Command::new("explorer")
        .args(["/select,", path.to_string_lossy().as_ref()]) // The comma after select is not a typo
        .spawn()
        .unwrap();
}

#[tauri::command]
pub fn close_without_saving(
    window: tauri::Window,
    app_state: tauri::State<'_, AppState>,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        let path = app.current_document().map(|d| d.path().to_owned());
        if let Some(path) = path {
            app.close_document(path);
            app.advance_exit();
            if app.should_exit() {
                window.close().ok();
            }
        }
    }))
}

struct DocumentToSave {
    sheet: Sheet<Absolute>,
    source: PathBuf,
    destination: PathBuf,
    version: i32,
}

async fn save_documents(
    window: tauri::Window,
    app_state: tauri::State<'_, AppState>,
    mut documents: Vec<DocumentToSave>,
) -> Result<Patch, ()> {
    let mut work = Vec::new();
    for document in &mut documents {
        let sheet = std::mem::take(&mut document.sheet);
        let write_destination = document.destination.clone();
        work.push(tauri::async_runtime::spawn_blocking(move || {
            sheet.write(&write_destination)
        }));
    }
    let results = futures::future::join_all(work)
        .await
        .into_iter()
        .map(|r| r.unwrap());

    Ok(app_state.mutate(AppTrim::Full, |app| {
        for (document, result) in documents.iter().zip(results) {
            match result {
                Ok(_) => {
                    app.relocate_document(&document.source, &document.destination);
                    if let Some(d) = app.document_mut(&document.destination) {
                        d.mark_as_saved(document.version);
                    }
                }
                Err(e) => app.show_error_message(
                    "Error".to_owned(),
                    format!(
                        "An error occured while trying to save `{}`",
                        document.destination.to_file_name()
                    ),
                    e.to_string(),
                ),
            }
        }

        app.advance_exit();
        if app.should_exit() {
            window.close().ok();
        }
    }))
}

#[tauri::command]
pub async fn save(
    window: tauri::Window,
    app_state: tauri::State<'_, AppState>,
) -> Result<Patch, ()> {
    let documents_to_save: Vec<DocumentToSave> = {
        let app = app_state.0.lock();
        let Some(document) = app.current_document() else {
            return Ok(Patch(Vec::new()))
        };
        vec![DocumentToSave {
            sheet: document.sheet().clone(),
            source: document.path().to_owned(),
            destination: document.path().to_owned(),
            version: document.version(),
        }]
    };
    save_documents(window, app_state, documents_to_save).await
}

#[tauri::command]
pub async fn save_as(
    window: tauri::Window,
    app_state: tauri::State<'_, AppState>,
    new_path: PathBuf,
) -> Result<Patch, ()> {
    let documents_to_save: Vec<DocumentToSave> = {
        let app = app_state.0.lock();
        let Some(document) = app.current_document() else {
            return Ok(Patch(Vec::new()))
        };
        vec![DocumentToSave {
            sheet: document.sheet().clone(),
            source: document.path().to_owned(),
            destination: new_path,
            version: document.version(),
        }]
    };
    save_documents(window, app_state, documents_to_save).await
}

#[tauri::command]
pub async fn save_all(
    window: tauri::Window,
    app_state: tauri::State<'_, AppState>,
) -> Result<Patch, ()> {
    let documents_to_save: Vec<DocumentToSave> = {
        let app = app_state.0.lock();
        app.documents_iter()
            .map(|d| DocumentToSave {
                sheet: d.sheet().clone(),
                source: d.path().to_owned(),
                destination: d.path().to_owned(),
                version: d.version(),
            })
            .collect()
    };
    save_documents(window, app_state, documents_to_save).await
}

#[tauri::command]
pub fn undo(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::Undo).ok();
        }
    }))
}

#[tauri::command]
pub fn redo(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::Redo).ok();
        }
    }))
}

#[tauri::command]
pub fn cut(
    tauri_app: tauri::AppHandle,
    app_state: tauri::State<'_, AppState>,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(data) = app.current_document().and_then(|d| d.copy()) {
            if let Ok(serialized) = serde_json::to_string(&data) {
                let mut clipboard = tauri_app.clipboard_manager();
                if clipboard.write_text(serialized).is_ok() {
                    app.set_clipboard_manifest(Some(data.manifest()));
                }
            }
        }
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::DeleteSelection).ok();
        }
    }))
}

#[tauri::command]
pub fn copy(
    tauri_app: tauri::AppHandle,
    app_state: tauri::State<'_, AppState>,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(data) = app.current_document().and_then(|d| d.copy()) {
            if let Ok(serialized) = serde_json::to_string(&data) {
                let mut clipboard = tauri_app.clipboard_manager();
                if clipboard.write_text(serialized).is_ok() {
                    app.set_clipboard_manifest(Some(data.manifest()));
                }
            }
        }
    }))
}

#[tauri::command]
pub fn paste(
    tauri_app: tauri::AppHandle,
    app_state: tauri::State<'_, AppState>,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        let clipboard = tauri_app.clipboard_manager();
        if let Ok(Some(serialized)) = clipboard.read_text() {
            if let Ok(data) = serde_json::from_str(&serialized) {
                if let Some(document) = app.current_document_mut() {
                    document.process_command(Command::Paste(data)).ok();
                }
            }
        }
    }))
}

#[tauri::command]
pub fn set_frames_list_mode(
    app_state: tauri::State<'_, AppState>,
    list_mode: dto::ListMode,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetFramesListMode(list_mode.into()))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn set_frames_list_offset(
    app_state: tauri::State<'_, AppState>,
    offset: u32,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetFramesListOffset(offset))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn set_hitboxes_list_offset(
    app_state: tauri::State<'_, AppState>,
    offset: u32,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetHitboxesListOffset(offset))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn filter_frames(
    app_state: tauri::State<'_, AppState>,
    search_query: String,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::FilterFrames(search_query))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn filter_animations(
    app_state: tauri::State<'_, AppState>,
    search_query: String,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::FilterAnimations(search_query))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn set_animations_list_offset(
    app_state: tauri::State<'_, AppState>,
    offset: u32,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetAnimationsListOffset(offset))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn import_frames(app: tauri::AppHandle, paths: Vec<PathBuf>) -> Result<Patch, ()> {
    app.import_frames(paths)
}

#[tauri::command]
pub fn begin_relocate_frames(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::BeginRelocateFrames).ok();
        }
    }))
}

#[tauri::command]
pub fn relocate_frame(
    app_state: tauri::State<'_, AppState>,
    from: PathBuf,
    to: PathBuf,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::RelocateFrame(from, to))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn cancel_relocate_frames(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::CancelRelocateFrames).ok();
        }
    }))
}

#[tauri::command]
pub fn end_relocate_frames(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::EndRelocateFrames).ok();
        }
    }))
}

#[tauri::command]
pub fn delete_frame(app: tauri::AppHandle, path: PathBuf) -> Result<Patch, ()> {
    app.delete_frame(path)
}

#[tauri::command]
pub fn delete_selected_frames(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::DeleteSelectedFrames).ok();
        }
    }))
}

#[tauri::command]
pub fn delete_selection(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::DeleteSelection).ok();
        }
    }))
}

#[tauri::command]
pub fn nudge_selection(
    app_state: tauri::State<'_, AppState>,
    direction: dto::NudgeDirection,
    large_nudge: bool,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::NudgeSelection(direction.into(), large_nudge))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn browse_selection(
    app_state: tauri::State<'_, AppState>,
    direction: dto::BrowseDirection,
    shift: bool,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::BrowseSelection(direction.into(), shift))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn browse_to_end(app_state: tauri::State<'_, AppState>, shift: bool) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::BrowseToEnd(shift)).ok();
        }
    }))
}

#[tauri::command]
pub fn browse_to_start(app_state: tauri::State<'_, AppState>, shift: bool) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::BrowseToStart(shift)).ok();
        }
    }))
}

#[tauri::command]
pub fn clear_selection(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::ClearSelection).ok();
        }
    }))
}

#[tauri::command]
pub fn select_frame(
    app_state: tauri::State<'_, AppState>,
    path: PathBuf,
    shift: bool,
    ctrl: bool,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SelectFrame(path, shift, ctrl))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn select_all(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::SelectAll).ok();
        }
    }))
}

#[tauri::command]
pub fn select_animation(
    app_state: tauri::State<'_, AppState>,
    name: String,
    shift: bool,
    ctrl: bool,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SelectAnimation(name, shift, ctrl))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn select_keyframe(
    app_state: tauri::State<'_, AppState>,
    direction: dto::Direction,
    index: usize,
    shift: bool,
    ctrl: bool,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SelectKeyframe(
                    direction.into(),
                    index,
                    shift,
                    ctrl,
                ))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn select_hitbox(
    app_state: tauri::State<'_, AppState>,
    name: String,
    shift: bool,
    ctrl: bool,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SelectHitbox(name, shift, ctrl))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn pan(app_state: tauri::State<'_, AppState>, delta: (f32, f32)) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::OnlyWorkbench, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::Pan(delta.into())).ok();
        }
    }))
}

#[tauri::command]
pub fn center_workbench(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::CenterWorkbench).ok();
        }
    }))
}

#[tauri::command]
pub fn zoom_in_workbench(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::ZoomInWorkbench).ok();
        }
    }))
}

#[tauri::command]
pub fn zoom_out_workbench(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::ZoomOutWorkbench).ok();
        }
    }))
}

#[tauri::command]
pub fn zoom_in_workbench_around(
    app_state: tauri::State<'_, AppState>,
    fixed_point: (f32, f32),
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::ZoomInWorkbenchAround(fixed_point.into()))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn zoom_out_workbench_around(
    app_state: tauri::State<'_, AppState>,
    fixed_point: (f32, f32),
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::ZoomOutWorkbenchAround(fixed_point.into()))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn set_workbench_zoom_factor(
    app_state: tauri::State<'_, AppState>,
    zoom_factor: u32,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetWorkbenchZoomFactor(zoom_factor))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn reset_workbench_zoom(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::ResetWorkbenchZoom).ok();
        }
    }))
}

#[tauri::command]
pub fn enable_sprite_darkening(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::EnableSpriteDarkening)
                .ok();
        }
    }))
}

#[tauri::command]
pub fn disable_sprite_darkening(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::DisableSpriteDarkening)
                .ok();
        }
    }))
}

#[tauri::command]
pub fn hide_sprite(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::HideSprite).ok();
        }
    }))
}

#[tauri::command]
pub fn show_sprite(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::ShowSprite).ok();
        }
    }))
}

#[tauri::command]
pub fn hide_hitboxes(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::HideHitboxes).ok();
        }
    }))
}

#[tauri::command]
pub fn show_hitboxes(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::ShowHitboxes).ok();
        }
    }))
}

#[tauri::command]
pub fn hide_origin(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::HideOrigin).ok();
        }
    }))
}

#[tauri::command]
pub fn show_origin(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::ShowOrigin).ok();
        }
    }))
}

#[tauri::command]
pub fn create_animation(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::CreateAnimation).ok();
        }
    }))
}

#[tauri::command]
pub fn edit_animation(app_state: tauri::State<'_, AppState>, name: String) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::EditAnimation(name)).ok();
        }
    }))
}

#[tauri::command]
pub fn begin_rename_animation(
    app_state: tauri::State<'_, AppState>,
    animation_name: String,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::BeginRenameAnimation(animation_name))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn begin_rename_hitbox(
    app_state: tauri::State<'_, AppState>,
    hitbox_name: String,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::BeginRenameHitbox(hitbox_name))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn begin_rename_selection(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::BeginRenameSelection).ok();
        }
    }))
}

#[tauri::command]
pub fn cancel_rename(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::CancelRename).ok();
        }
    }))
}

#[tauri::command]
pub fn end_rename_animation(
    app_state: tauri::State<'_, AppState>,
    new_name: String,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::EndRenameAnimation(new_name))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn end_rename_hitbox(
    app_state: tauri::State<'_, AppState>,
    new_name: String,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::EndRenameHitbox(new_name))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn delete_animation(app_state: tauri::State<'_, AppState>, name: String) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::DeleteAnimation(name))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn delete_selected_animations(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::DeleteSelectedAnimations)
                .ok();
        }
    }))
}

#[tauri::command]
pub fn tick(app_state: tauri::State<'_, AppState>, delta_time_millis: f64) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::OnlyCurrentDocument, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::Tick(Duration::from_nanos(
                    (delta_time_millis * 1_000_000.0) as u64,
                )))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn play(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::Play).ok();
        }
    }))
}

#[tauri::command]
pub fn pause(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::Pause).ok();
        }
    }))
}

#[tauri::command]
pub fn scrub_timeline(
    app_state: tauri::State<'_, AppState>,
    time_millis: u64,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::ScrubTimeline(Duration::from_millis(time_millis)))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn jump_to_animation_start(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::JumpToAnimationStart).ok();
        }
    }))
}

#[tauri::command]
pub fn jump_to_animation_end(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::JumpToAnimationEnd).ok();
        }
    }))
}

#[tauri::command]
pub fn jump_to_previous_frame(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::JumpToPreviousFrame).ok();
        }
    }))
}

#[tauri::command]
pub fn jump_to_next_frame(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::JumpToNextFrame).ok();
        }
    }))
}

#[tauri::command]
pub fn set_snap_keyframe_durations(
    app_state: tauri::State<'_, AppState>,
    snap: bool,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetSnapKeyframeDurations(snap))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn set_snap_keyframes_to_other_keyframes(
    app_state: tauri::State<'_, AppState>,
    snap: bool,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetSnapKeyframeToOtherKeyframes(snap))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn set_snap_keyframes_to_multiples_of_duration(
    app_state: tauri::State<'_, AppState>,
    snap: bool,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetSnapKeyframeToMultiplesOfDuration(snap))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn set_keyframe_snapping_base_duration(
    app_state: tauri::State<'_, AppState>,
    duration_millis: u64,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetKeyframeSnappingBaseDuration(
                    Duration::from_millis(duration_millis),
                ))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn zoom_in_timeline(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::ZoomInTimeline).ok();
        }
    }))
}

#[tauri::command]
pub fn zoom_out_timeline(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::ZoomOutTimeline).ok();
        }
    }))
}

#[tauri::command]
pub fn zoom_in_timeline_around(
    app_state: tauri::State<'_, AppState>,
    fixed_point: f32,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::ZoomInTimelineAround(Duration::from_secs_f32(
                    fixed_point.max(0.0) / 1_000.0,
                )))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn zoom_out_timeline_around(
    app_state: tauri::State<'_, AppState>,
    fixed_point: f32,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::ZoomOutTimelineAround(Duration::from_secs_f32(
                    fixed_point.max(0.0) / 1_000.0,
                )))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn set_timeline_zoom_amount(
    app_state: tauri::State<'_, AppState>,
    amount: f32,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetTimelineZoomAmount(amount))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn reset_timeline_zoom(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::ResetTimelineZoom).ok();
        }
    }))
}

#[tauri::command]
pub fn set_timeline_offset(
    app_state: tauri::State<'_, AppState>,
    offset_millis: f32,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::OnlyWorkbench, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetTimelineOffset(Duration::from_secs_f32(
                    offset_millis.max(0.0) / 1_000.0,
                )))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn pan_timeline(app_state: tauri::State<'_, AppState>, delta: f32) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::OnlyWorkbench, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::PanTimeline(delta)).ok();
        }
    }))
}

#[tauri::command]
pub fn set_animation_looping(
    app_state: tauri::State<'_, AppState>,
    is_looping: bool,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetAnimationLooping(is_looping))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn apply_direction_preset(
    app_state: tauri::State<'_, AppState>,
    preset: dto::DirectionPreset,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::ApplyDirectionPreset(preset.into()))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn select_direction(
    app_state: tauri::State<'_, AppState>,
    direction: dto::Direction,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SelectDirection(direction.into()))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn begin_drag_and_drop_frame(
    app_state: tauri::State<'_, AppState>,
    frame: PathBuf,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::BeginDragAndDropFrame(frame))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn drop_frame_on_timeline(
    app_state: tauri::State<'_, AppState>,
    direction: dto::Direction,
    index: usize,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::DropFrameOnTimeline(direction.into(), index))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn end_drag_and_drop_frame(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::EndDragAndDropFrame).ok();
        }
    }))
}

#[tauri::command]
pub fn delete_selected_keyframes(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::DeleteSelectedKeyframes)
                .ok();
        }
    }))
}

#[tauri::command]
pub fn set_keyframe_duration(
    app_state: tauri::State<'_, AppState>,
    duration_millis: u64,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetKeyframeDuration(Duration::from_millis(
                    duration_millis,
                )))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn set_keyframe_offset_x(app_state: tauri::State<'_, AppState>, x: i32) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetKeyframeOffsetX(x))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn set_keyframe_offset_y(app_state: tauri::State<'_, AppState>, y: i32) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetKeyframeOffsetY(y))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn begin_drag_and_drop_keyframe(
    app_state: tauri::State<'_, AppState>,
    direction: dto::Direction,
    index: usize,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::BeginDragAndDropKeyframe(direction.into(), index))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn drop_keyframe_on_timeline(
    app_state: tauri::State<'_, AppState>,
    direction: dto::Direction,
    index: usize,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::DropKeyframeOnTimeline(direction.into(), index))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn end_drag_and_drop_keyframe(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::EndDragAndDropKeyframe)
                .ok();
        }
    }))
}

#[tauri::command]
pub fn begin_drag_keyframe_duration(
    app_state: tauri::State<'_, AppState>,
    direction: dto::Direction,
    index: usize,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::BeginDragKeyframeDuration(direction.into(), index))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn update_drag_keyframe_duration(
    app_state: tauri::State<'_, AppState>,
    delta_millis: i64,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::OnlyWorkbench, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::UpdateDragKeyframeDuration(delta_millis))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn end_drag_keyframe_duration(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::EndDragKeyframeDuration())
                .ok();
        }
    }))
}

#[tauri::command]
pub fn begin_nudge_keyframe(
    app_state: tauri::State<'_, AppState>,
    direction: dto::Direction,
    index: usize,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::BeginNudgeKeyframe(direction.into(), index))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn update_nudge_keyframe(
    app_state: tauri::State<'_, AppState>,
    displacement: (i32, i32),
    both_axis: bool,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::OnlyWorkbench, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::UpdateNudgeKeyframe(displacement.into(), both_axis))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn end_nudge_keyframe(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::EndNudgeKeyframe()).ok();
        }
    }))
}

#[tauri::command]
pub fn create_hitbox(
    app_state: tauri::State<'_, AppState>,
    position: Option<(i32, i32)>,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::CreateHitbox(position.map(|p| p.into())))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn delete_hitbox(app_state: tauri::State<'_, AppState>, name: String) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::DeleteHitbox(name)).ok();
        }
    }))
}

#[tauri::command]
pub fn delete_selected_hitboxes(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::DeleteSelectedHitboxes)
                .ok();
        }
    }))
}

#[tauri::command]
pub fn lock_hitboxes(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::LockHitboxes).ok();
        }
    }))
}

#[tauri::command]
pub fn unlock_hitboxes(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::UnlockHitboxes).ok();
        }
    }))
}

#[tauri::command]
pub fn set_hitbox_position_x(app_state: tauri::State<'_, AppState>, x: i32) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetHitboxPositionX(x))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn set_hitbox_position_y(app_state: tauri::State<'_, AppState>, y: i32) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetHitboxPositionY(y))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn set_hitbox_width(app_state: tauri::State<'_, AppState>, width: u32) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetHitboxWidth(width))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn set_hitbox_height(app_state: tauri::State<'_, AppState>, height: u32) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetHitboxHeight(height))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn toggle_preserve_aspect_ratio(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::TogglePreserveAspectRatio)
                .ok();
        }
    }))
}

#[tauri::command]
pub fn begin_nudge_hitbox(
    app_state: tauri::State<'_, AppState>,
    name: String,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::BeginNudgeHitbox(name))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn update_nudge_hitbox(
    app_state: tauri::State<'_, AppState>,
    displacement: (i32, i32),
    both_axis: bool,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::OnlyWorkbench, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::UpdateNudgeHitbox(displacement.into(), both_axis))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn end_nudge_hitbox(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::EndNudgeHitbox).ok();
        }
    }))
}

#[tauri::command]
pub fn begin_resize_hitbox(
    app_state: tauri::State<'_, AppState>,
    name: String,
    axis: dto::ResizeAxis,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::BeginResizeHitbox(name, axis.into()))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn update_resize_hitbox(
    app_state: tauri::State<'_, AppState>,
    displacement: (i32, i32),
    preserve_aspect_ratio: bool,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::OnlyWorkbench, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::UpdateResizeHitbox(
                    displacement.into(),
                    preserve_aspect_ratio,
                ))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn end_resize_hitbox(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::EndResizeHitbox).ok();
        }
    }))
}

#[tauri::command]
pub async fn export(app: tauri::AppHandle) -> Result<Patch, ()> {
    app.export().await
}

#[tauri::command]
pub fn begin_export_as(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::BeginExportAs).ok();
        }
    }))
}

#[tauri::command]
pub fn set_export_template_file(
    app_state: tauri::State<'_, AppState>,
    file: PathBuf,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetExportTemplateFile(file))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn set_export_texture_file(
    app_state: tauri::State<'_, AppState>,
    file: PathBuf,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetExportTextureFile(file))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn set_export_metadata_file(
    app_state: tauri::State<'_, AppState>,
    file: PathBuf,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetExportMetadataFile(file))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn set_export_metadata_paths_root(
    app_state: tauri::State<'_, AppState>,
    directory: PathBuf,
) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document
                .process_command(Command::SetExportMetadataPathsRoot(directory))
                .ok();
        }
    }))
}

#[tauri::command]
pub fn cancel_export_as(app_state: tauri::State<'_, AppState>) -> Result<Patch, ()> {
    Ok(app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::CancelExportAs).ok();
        }
    }))
}

#[tauri::command]
pub async fn end_export_as(
    app_state: tauri::State<'_, AppState>,
    texture_cache: tauri::State<'_, texture_cache::Handle>,
) -> Result<Patch, ()> {
    let mut patch = app_state.mutate(AppTrim::Full, |app| {
        if let Some(document) = app.current_document_mut() {
            document.process_command(Command::EndExportAs).ok();
        }
    });

    let (sheet, document_name) = {
        let app = app_state.0.lock();
        match app.current_document() {
            Some(d) => (d.sheet().clone(), d.path().to_file_name()),
            _ => return Ok(patch),
        }
    };

    let result = tauri::async_runtime::spawn_blocking({
        let texture_cache = texture_cache.0.clone();
        move || export_sheet(&sheet, Handle(texture_cache))
    })
    .await
    .unwrap();

    let mut additional_patch = app_state.mutate(AppTrim::Full, |app| {
        if let Err(e) = result {
            app.show_error_message(
                "Export Error".to_owned(),
                format!(
                    "An error occured while trying to export `{}`",
                    document_name.to_file_name(),
                ),
                e.to_string(),
            );
        }
    });

    patch.0.append(&mut additional_patch.0);
    Ok(patch)
}
