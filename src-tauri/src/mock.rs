use json_patch::Patch;
use std::{
    ops::Deref,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::{
    api::Api,
    dto,
    features::{self, texture_cache},
    state::{self, State},
    utils::{
        handle,
        paths::{self, Paths},
    },
    TigerApp,
};

#[derive(Clone)]
pub struct TigerAppMock {
    state: state::Handle,
    texture_cache: texture_cache::Handle,
    paths: paths::Handle,
    client_state: handle::Handle<dto::State>,
    events: handle::Handle<Vec<(String, serde_json::Value)>>,
    clipboard: handle::Handle<Option<String>>,
    closed: handle::Handle<bool>,
}

impl TigerAppMock {
    const PERIOD: Duration = Duration::from_millis(50);

    pub fn new() -> Self {
        let app = Self::new_uninitialized();
        app.init();
        app
    }

    pub fn new_uninitialized() -> Self {
        let paths = Paths::test_outputs();
        std::fs::remove_file(&paths.log_file).ok();
        std::fs::remove_file(&paths.recent_documents_file).ok();
        Self {
            state: state::Handle::default(),
            texture_cache: texture_cache::Handle::default(),
            paths: handle::Handle::new(paths),
            client_state: handle::Handle::new(State::default().to_dto(dto::StateTrim::Full)),
            events: handle::Handle::default(),
            clipboard: handle::Handle::default(),
            closed: handle::Handle::default(),
        }
    }

    pub fn init(&self) {
        self.texture_cache.init(self.clone(), Self::PERIOD);
        features::clipboard_analysis::init(self.clone(), Self::PERIOD);
        features::missing_textures::init(self.clone(), Self::PERIOD);
        features::recent_documents::init(self.clone());
        features::template_hot_reload::init(self.clone(), Self::PERIOD);
        features::texture_hot_reload::init(self.clone(), Self::PERIOD);
        self.replace_state();
    }

    pub fn wait_for_periodic_scans(&self) {
        std::thread::sleep(2 * Self::PERIOD);
    }

    pub fn client_state(&self) -> dto::State {
        self.client_state.lock().clone()
    }

    pub fn events(&self) -> Vec<(String, serde_json::Value)> {
        self.events.lock().clone()
    }

    pub fn assert_eventually<F: Fn() -> bool>(&self, test: F) {
        let start = std::time::Instant::now();
        while std::time::Instant::now().duration_since(start) < Duration::from_secs(5) {
            if test() {
                return;
            }
            self.wait_for_periodic_scans();
        }
        panic!("Assertion failed");
    }

    fn apply_patch(&self, patch: Patch) {
        let mut client_state = serde_json::to_value(self.client_state.lock().deref()).unwrap();
        json_patch::patch(&mut client_state, &patch).unwrap();
        *self.client_state.lock() = serde_json::from_value(client_state).unwrap();
    }
}

#[allow(dead_code)]
impl TigerAppMock {
    pub fn begin_drag_and_drop_frame<P: Into<PathBuf>>(&self, frame: P) {
        self.apply_patch(Api::begin_drag_and_drop_frame(self, frame).unwrap());
    }

    pub fn begin_export_as(&self) {
        self.apply_patch(Api::begin_export_as(self).unwrap());
    }

    pub fn close_document<P: AsRef<Path>>(&self, path: P) {
        self.apply_patch(Api::close_document(self, path).unwrap());
    }

    pub fn copy(&self) {
        self.apply_patch(Api::copy(self).unwrap());
    }

    pub fn cut(&self) {
        self.apply_patch(Api::cut(self).unwrap());
    }

    pub fn create_animation(&self) {
        self.apply_patch(Api::create_animation(self).unwrap());
    }

    pub fn create_hitbox(&self, position: Option<(i32, i32)>) {
        self.apply_patch(Api::create_hitbox(self, position).unwrap());
    }

    pub fn delete_frame<P: Into<PathBuf>>(&self, path: P) {
        self.apply_patch(Api::delete_frame(self, path).unwrap());
    }

    pub fn delete_hitbox<S: Into<String>>(&self, name: S) {
        self.apply_patch(Api::delete_hitbox(self, name).unwrap());
    }

    pub fn drop_frame_on_timeline(&self, direction: dto::Direction, index: usize) {
        self.apply_patch(Api::drop_frame_on_timeline(self, direction, index).unwrap());
    }

    pub fn edit_animation<S: Into<String>>(&self, name: S) {
        self.apply_patch(Api::edit_animation(self, name).unwrap());
    }

    pub async fn export(&self) {
        self.apply_patch(Api::export(self).await.unwrap());
    }

    pub fn focus_document<P: AsRef<Path>>(&self, path: P) {
        self.apply_patch(Api::focus_document(self, path).unwrap());
    }

    pub fn import_frames<P: Into<PathBuf>>(&self, paths: Vec<P>) {
        self.apply_patch(Api::import_frames(self, paths).unwrap());
    }

    pub fn new_document<P: Into<PathBuf>>(&self, path: P) {
        self.apply_patch(Api::new_document(self, path).unwrap());
    }

    pub async fn open_documents<P: Into<PathBuf> + Send + Sync>(&self, paths: Vec<P>) {
        self.apply_patch(Api::open_documents(self, paths).await.unwrap());
    }

    pub fn paste(&self) {
        self.apply_patch(Api::paste(self).unwrap());
    }

    pub fn reset_timeline_zoom(&self) {
        self.apply_patch(Api::reset_timeline_zoom(self).unwrap());
    }

    pub fn reset_workbench_zoom(&self) {
        self.apply_patch(Api::reset_workbench_zoom(self).unwrap());
    }

    pub fn select_animation<S: Into<String>>(&self, name: S, shift: bool, ctrl: bool) {
        self.apply_patch(Api::select_animation(self, name, shift, ctrl).unwrap());
    }

    pub fn select_frame<P: Into<PathBuf>>(&self, path: P, shift: bool, ctrl: bool) {
        self.apply_patch(Api::select_frame(self, path, shift, ctrl).unwrap());
    }

    pub fn select_hitbox<S: Into<String>>(&self, name: S, shift: bool, ctrl: bool) {
        self.apply_patch(Api::select_hitbox(self, name, shift, ctrl).unwrap());
    }

    pub fn select_keyframe(
        &self,
        direction: dto::Direction,
        index: usize,
        shift: bool,
        ctrl: bool,
    ) {
        self.apply_patch(Api::select_keyframe(self, direction, index, shift, ctrl).unwrap());
    }

    pub fn set_export_template_file<P: Into<PathBuf>>(&self, path: P) {
        self.apply_patch(Api::set_export_template_file(self, path).unwrap());
    }

    pub fn set_hitbox_height(&self, height: u32) {
        self.apply_patch(Api::set_hitbox_height(self, height).unwrap());
    }

    pub fn set_hitbox_position_x(&self, x: i32) {
        self.apply_patch(Api::set_hitbox_position_x(self, x).unwrap());
    }

    pub fn set_hitbox_position_y(&self, y: i32) {
        self.apply_patch(Api::set_hitbox_position_y(self, y).unwrap());
    }

    pub fn set_hitbox_width(&self, width: u32) {
        self.apply_patch(Api::set_hitbox_width(self, width).unwrap());
    }

    pub fn set_keyframe_duration(&self, duration_millis: u64) {
        self.apply_patch(Api::set_keyframe_duration(self, duration_millis).unwrap());
    }

    pub fn set_keyframe_offset_x(&self, x: i32) {
        self.apply_patch(Api::set_keyframe_offset_x(self, x).unwrap());
    }

    pub fn set_timeline_zoom_amount(&self, amount: f32) {
        self.apply_patch(Api::set_timeline_zoom_amount(self, amount).unwrap());
    }

    pub fn set_keyframe_offset_y(&self, y: i32) {
        self.apply_patch(Api::set_keyframe_offset_y(self, y).unwrap());
    }

    pub fn set_workbench_zoom_factor(&self, zoom_factor: u32) {
        self.apply_patch(Api::set_workbench_zoom_factor(self, zoom_factor).unwrap());
    }

    pub fn toggle_preserve_aspect_ratio(&self) {
        self.apply_patch(Api::toggle_preserve_aspect_ratio(self).unwrap());
    }

    pub fn zoom_in_timeline(&self) {
        self.apply_patch(Api::zoom_in_timeline(self).unwrap());
    }

    pub fn zoom_in_timeline_around(&self, fixed_point: f32) {
        self.apply_patch(Api::zoom_in_timeline_around(self, fixed_point).unwrap());
    }

    pub fn zoom_in_workbench(&self) {
        self.apply_patch(Api::zoom_in_workbench(self).unwrap());
    }

    pub fn zoom_out_timeline(&self) {
        self.apply_patch(Api::zoom_out_timeline(self).unwrap());
    }

    pub fn zoom_out_timeline_around(&self, fixed_point: f32) {
        self.apply_patch(Api::zoom_out_timeline_around(self, fixed_point).unwrap());
    }

    pub fn zoom_out_workbench(&self) {
        self.apply_patch(Api::zoom_out_workbench(self).unwrap());
    }

    pub fn zoom_in_workbench_around(&self, fixed_point: (f32, f32)) {
        self.apply_patch(Api::zoom_in_workbench_around(self, fixed_point).unwrap());
    }

    pub fn zoom_out_workbench_around(&self, fixed_point: (f32, f32)) {
        self.apply_patch(Api::zoom_out_workbench_around(self, fixed_point).unwrap());
    }
}

impl TigerApp for TigerAppMock {
    fn state(&self) -> state::Handle {
        self.state.clone()
    }

    fn texture_cache(&self) -> texture_cache::Handle {
        self.texture_cache.clone()
    }

    fn paths(&self) -> paths::Handle {
        self.paths.clone()
    }

    fn patch_state<F: FnOnce(&mut State)>(&self, state_trim: dto::StateTrim, operation: F) {
        let state_handle = self.state();
        let patch = state_handle.mutate(state_trim, operation);
        self.apply_patch(patch);
    }

    fn replace_state(&self) {
        let state_handle = self.state();
        let state = state_handle.lock();
        *self.client_state.lock() = state.to_dto(dto::StateTrim::Full);
    }

    fn emit_all<S: serde::Serialize + Clone>(&self, event: &str, payload: S) {
        self.events
            .lock()
            .push((event.to_owned(), serde_json::to_value(payload).unwrap()));
    }

    fn read_clipboard(&self) -> Option<String> {
        self.clipboard.lock().clone()
    }

    fn write_clipboard<S: Into<String>>(&self, content: S) {
        *self.clipboard.lock() = Some(content.into())
    }

    fn close_window(&self) {
        *self.closed.lock() = true;
    }
}
