use euclid::default::*;
use euclid::{point2, vec2};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

use crate::sheet::*;
use crate::state::*;

#[derive(Error, Debug)]
pub enum DocumentError {
    #[error("Cannot perform undo operation")]
    UndoOperationNowAllowed,
    #[error("Requested frame is not in document")]
    FrameNotInDocument,
    #[error("Requested animation is not in document")]
    AnimationNotInDocument,
    #[error("Frame does not have a hitbox with the requested name")]
    InvalidHitboxName,
    #[error("Animation does not have a frame at the requested index")]
    InvalidKeyframeIndex,
    #[error("No keyframe found for requested time")]
    NoKeyframeForThisTime,
    #[error("Expected a hitbox to be selected")]
    NoHitboxSelected,
    #[error("Expected an keyframe to be selected")]
    NoKeyframeSelected,
    #[error("A hitbox with this name already exists")]
    HitboxAlreadyExists,
    #[error("An animation with this name already exists")]
    AnimationAlreadyExists,
    #[error("Not currently editing any frame")]
    NotEditingAnyFrame,
    #[error("Not currently editing any animation")]
    NotEditingAnyAnimation,
    #[error("Not currently adjusting export settings")]
    NotExporting,
    #[error("Not currently renaming an item")]
    NotRenaming,
    #[error("Not currently adjusting keyframe position")]
    NotAdjustingKeyframePosition,
    #[error("Not currently adjusting hitbox size")]
    NotAdjustingHitboxSize,
    #[error("Not currently adjusting hitbox position")]
    NotAdjustingHitboxPosition,
    #[error("Not currently adjusting keyframe duration")]
    NotAdjustingKeyframeDuration,
    #[error("Missing data while adjusting hitbox size")]
    MissingHitboxSizeData,
    #[error("Missing data while adjusting hitbox position")]
    MissingHitboxPositionData,
    #[error("Missing data while adjusting keyframe position")]
    MissingKeyframePositionData,
    #[error("Missing data while adjusting keyframe duration")]
    MissingKeyframeDurationData,
    #[error("Invalid sheet operation")]
    InvalidSheetOperation(#[from] SheetError),
}

#[derive(Debug, Default)]
struct HistoryEntry {
    last_command: Option<DocumentCommand>,
    sheet: Sheet,
    view: View,
    version: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CloseState {
    Requested,
    Saving,
    Allowed,
}

#[derive(Clone, Debug, Default)]
pub struct Persistent {
    pub export_settings_edit: Option<ExportSettings>,
    pub close_state: Option<CloseState>,
    timeline_is_playing: bool,
    disk_version: i32,
}

#[derive(Debug)]
pub struct Document {
    pub source: PathBuf,
    pub sheet: Sheet, // Sheet being edited, fully recorded in history
    pub view: View,   // View state, collapsed and recorded in history
    pub transient: Option<Transient>, // State preventing undo actions when not default, not recorded in history
    pub persistent: Persistent,       // Other state, not recorded in history
    next_version: i32,
    history: Vec<HistoryEntry>,
    history_index: usize,
}

impl Document {
    pub fn new<T: AsRef<Path>>(path: T) -> Document {
        let history_entry: HistoryEntry = Default::default();
        let sheet = history_entry.sheet.clone();
        let view = history_entry.view.clone();
        let next_version = history_entry.version;
        Document {
            source: path.as_ref().to_owned(),
            history: vec![history_entry],
            sheet: sheet,
            view: view,
            transient: None,
            persistent: Default::default(),
            next_version: next_version,
            history_index: 0,
        }
    }

    pub fn open<T: AsRef<Path>>(path: T) -> anyhow::Result<Document> {
        let mut document = Document::new(&path);

        let mut directory = path.as_ref().to_owned();
        directory.pop();
        let sheet: Sheet = compat::read_sheet(path.as_ref())?;
        document.sheet = sheet.with_absolute_paths(&directory);
        document.history[0].sheet = document.sheet.clone();
        document.persistent.disk_version = document.next_version;

        Ok(document)
    }

    pub fn save<T: AsRef<Path>>(sheet: &Sheet, to: T) -> anyhow::Result<()> {
        let mut directory = to.as_ref().to_owned();
        directory.pop();
        let sheet = sheet.with_relative_paths(directory)?;
        compat::write_sheet(to, &sheet)?;
        Ok(())
    }

    pub fn is_saved(&self) -> bool {
        self.persistent.disk_version == self.get_version()
    }

    pub fn get_version(&self) -> i32 {
        self.history[self.history_index].version
    }

    pub fn get_display_name(&self) -> String {
        self.source
            .file_name()
            .and_then(|f| Some(f.to_string_lossy().into_owned()))
            .unwrap_or("???".to_owned())
    }

    pub fn tick(&mut self, delta: Duration) {
        self.advance_timeline(delta);
        self.try_close();
    }

    fn advance_timeline(&mut self, delta: Duration) {
        if self.persistent.timeline_is_playing {
            self.view.timeline_clock += delta;
            if let Some(WorkbenchItem::Animation(animation_name)) = &self.view.workbench_item {
                if let Some(animation) = self.sheet.get_animation(animation_name) {
                    match animation.get_duration() {
                        Some(d) if d > 0 => {
                            let clock_ms = self.view.timeline_clock.as_millis();
                            // Loop animation
                            if animation.is_looping() {
                                self.view.timeline_clock =
                                    Duration::from_millis((clock_ms % u128::from(d)) as u64)

                            // Stop playhead at the end of animation
                            } else if clock_ms >= u128::from(d) {
                                self.persistent.timeline_is_playing = false;
                                self.view.timeline_clock = Duration::from_millis(u64::from(d))
                            }
                        }

                        // Reset playhead
                        _ => {
                            self.persistent.timeline_is_playing = false;
                            self.view.timeline_clock = Duration::new(0, 0);
                        }
                    };
                }
            }
        }
    }

    fn try_close(&mut self) {
        if self.persistent.close_state == Some(CloseState::Saving) {
            if self.is_saved() {
                self.persistent.close_state = Some(CloseState::Allowed);
            }
        }
    }

    fn push_undo_state(&mut self, entry: HistoryEntry) {
        self.history.truncate(self.history_index + 1);
        self.history.push(entry);
        self.history_index = self.history.len() - 1;

        while self.history.len() > 100 {
            self.history.remove(0);
            self.history_index -= 1;
        }
    }

    fn can_use_undo_system(&self) -> bool {
        self.transient.is_none()
    }

    fn record_command(&mut self, command: DocumentCommand) {
        if !self.can_use_undo_system() {
            return;
        }

        let has_sheet_changes = &self.history[self.history_index].sheet != &self.sheet;

        if has_sheet_changes {
            self.next_version += 1;
        }

        let new_undo_state = HistoryEntry {
            sheet: self.sheet.clone(),
            view: self.view.clone(),
            last_command: Some(command),
            version: self.next_version,
        };

        if has_sheet_changes {
            self.push_undo_state(new_undo_state);
        } else if &self.history[self.history_index].view != &new_undo_state.view {
            let merge = self.history_index > 0
                && self.history[self.history_index - 1].sheet
                    == self.history[self.history_index].sheet;
            if merge {
                self.history[self.history_index].view = new_undo_state.view;
            } else {
                self.push_undo_state(new_undo_state);
            }
        }
    }

    pub fn undo(&mut self) -> Result<(), DocumentError> {
        if !self.can_use_undo_system() {
            return Err(DocumentError::UndoOperationNowAllowed.into());
        }
        if self.history_index > 0 {
            self.history_index -= 1;
            self.sheet = self.history[self.history_index].sheet.clone();
            self.view = self.history[self.history_index].view.clone();
            self.persistent.timeline_is_playing = false;
        }
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), DocumentError> {
        if !self.can_use_undo_system() {
            return Err(DocumentError::UndoOperationNowAllowed.into());
        }
        if self.history_index < self.history.len() - 1 {
            self.history_index += 1;
            self.sheet = self.history[self.history_index].sheet.clone();
            self.view = self.history[self.history_index].view.clone();
            self.persistent.timeline_is_playing = false;
        }
        Ok(())
    }

    pub fn get_undo_command(&self) -> Option<&DocumentCommand> {
        self.history[self.history_index].last_command.as_ref()
    }

    pub fn get_redo_command(&self) -> Option<&DocumentCommand> {
        if self.history_index < self.history.len() - 1 {
            self.history[self.history_index + 1].last_command.as_ref()
        } else {
            None
        }
    }

    fn get_workbench_frame(&self) -> Result<&Frame, DocumentError> {
        match &self.view.workbench_item {
            Some(WorkbenchItem::Frame(path)) => Some(
                self.sheet
                    .get_frame(path)
                    .ok_or(DocumentError::FrameNotInDocument)?,
            ),
            _ => None,
        }
        .ok_or_else(|| DocumentError::NotEditingAnyFrame.into())
    }

    fn get_workbench_frame_mut(&mut self) -> Result<&mut Frame, DocumentError> {
        match &self.view.workbench_item {
            Some(WorkbenchItem::Frame(path)) => Some(
                self.sheet
                    .get_frame_mut(path)
                    .ok_or(DocumentError::FrameNotInDocument)?,
            ),
            _ => None,
        }
        .ok_or_else(|| DocumentError::NotEditingAnyFrame.into())
    }

    fn get_workbench_animation(&self) -> Result<&Animation, DocumentError> {
        match &self.view.workbench_item {
            Some(WorkbenchItem::Animation(n)) => Some(
                self.sheet
                    .get_animation(n)
                    .ok_or(DocumentError::AnimationNotInDocument)?,
            ),
            _ => None,
        }
        .ok_or_else(|| DocumentError::NotEditingAnyAnimation.into())
    }

    fn get_workbench_animation_mut(&mut self) -> Result<&mut Animation, DocumentError> {
        match &self.view.workbench_item {
            Some(WorkbenchItem::Animation(n)) => Some(
                self.sheet
                    .get_animation_mut(n)
                    .ok_or(DocumentError::AnimationNotInDocument)?,
            ),
            _ => None,
        }
        .ok_or_else(|| DocumentError::NotEditingAnyAnimation.into())
    }

    pub fn is_dragging_content_frames(&self) -> bool {
        self.transient == Some(Transient::ContentFramesDrag)
    }

    pub fn is_dragging_timeline_frames(&self) -> bool {
        self.transient == Some(Transient::TimelineFrameDrag)
    }

    pub fn is_positioning_hitbox(&self) -> bool {
        match &self.transient {
            Some(Transient::HitboxPosition(_)) => true,
            _ => false,
        }
    }

    pub fn is_sizing_hitbox(&self) -> bool {
        match &self.transient {
            Some(Transient::HitboxSize(_)) => true,
            _ => false,
        }
    }

    pub fn is_scrubbing_timeline(&self) -> bool {
        self.transient == Some(Transient::TimelineScrub)
    }

    pub fn is_adjusting_frame_duration(&self) -> bool {
        match &self.transient {
            Some(Transient::KeyframeDuration(_)) => true,
            _ => false,
        }
    }

    pub fn is_moving_keyframe(&self) -> bool {
        match &self.transient {
            Some(Transient::KeyframePosition(_)) => true,
            _ => false,
        }
    }

    pub fn is_frame_selected(&self, frame: &Frame) -> bool {
        self.view
            .selection
            .as_ref()
            .map_or(false, |s| s.is_frame_selected(frame.get_source()))
    }

    pub fn is_animation_selected(&self, animation: &Animation) -> bool {
        self.view
            .selection
            .as_ref()
            .map_or(false, |s| s.is_animation_selected(animation.get_name()))
    }

    pub fn is_hitbox_selected(&self, hitbox: &Hitbox) -> bool {
        self.view
            .selection
            .as_ref()
            .map_or(false, |s| s.is_hitbox_selected(hitbox.get_name()))
    }

    pub fn is_keyframe_selected(&self, keyframe_index: usize) -> bool {
        self.view
            .selection
            .as_ref()
            .map_or(false, |s| s.is_keyframe_selected(keyframe_index))
    }

    pub fn clear_selection(&mut self) {
        self.view.selection = None;
    }

    pub fn select_frames(&mut self, paths: &MultiSelection<PathBuf>) -> Result<(), DocumentError> {
        for path in paths.items.iter() {
            if !self.sheet.has_frame(path) {
                return Err(DocumentError::FrameNotInDocument.into());
            }
        }
        if paths.items.is_empty() {
            self.clear_selection();
        } else {
            self.view.selection = Some(Selection::Frame(paths.clone()));
        }
        Ok(())
    }

    pub fn select_animations(
        &mut self,
        names: &MultiSelection<String>,
    ) -> Result<(), DocumentError> {
        for name in names.items.iter() {
            if !self.sheet.has_animation(name) {
                return Err(DocumentError::AnimationNotInDocument.into());
            }
        }
        if names.items.is_empty() {
            self.clear_selection();
        } else {
            self.view.selection = Some(Selection::Animation(names.clone()));
        }
        Ok(())
    }

    pub fn select_hitboxes(&mut self, names: &MultiSelection<String>) -> Result<(), DocumentError> {
        let frame_path = match &self.view.workbench_item {
            Some(WorkbenchItem::Frame(p)) => Some(p.to_owned()),
            _ => None,
        }
        .ok_or(DocumentError::NotEditingAnyFrame)?;
        let frame = self
            .sheet
            .get_frame(&frame_path)
            .ok_or(DocumentError::FrameNotInDocument)?;
        for name in names.items.iter() {
            let _hitbox = frame
                .get_hitbox(name)
                .ok_or(DocumentError::InvalidHitboxName)?;
        }
        if names.items.is_empty() {
            self.clear_selection();
        } else {
            self.view.selection = Some(Selection::Hitbox(names.clone()));
        }
        Ok(())
    }

    pub fn select_keyframes(
        &mut self,
        frame_indexes: &MultiSelection<usize>,
    ) -> Result<(), DocumentError> {
        if frame_indexes.items.is_empty() {
            self.clear_selection();
        } else {
            self.view.selection = Some(Selection::Keyframe(frame_indexes.clone()));

            let animation = self.get_workbench_animation()?;

            let keyframe_index = frame_indexes.last_touched_in_range;

            let frame_times = animation.get_frame_times();
            let frame_start_time = *frame_times
                .get(keyframe_index)
                .ok_or(DocumentError::InvalidKeyframeIndex)?;

            let keyframe = animation
                .get_frame(keyframe_index)
                .ok_or(DocumentError::InvalidKeyframeIndex)?;
            let duration = keyframe.get_duration() as u64;

            let clock = self.view.timeline_clock.as_millis() as u64;
            let is_playhead_in_frame = clock >= frame_start_time
                && (clock < (frame_start_time + duration)
                    || keyframe_index == animation.get_num_frames() - 1);
            if !self.persistent.timeline_is_playing && !is_playhead_in_frame {
                self.view.timeline_clock = Duration::from_millis(frame_start_time);
            }
        }
        Ok(())
    }

    pub fn edit_frame<T: AsRef<Path>>(&mut self, path: T) -> Result<(), DocumentError> {
        if !self.sheet.has_frame(&path) {
            return Err(DocumentError::FrameNotInDocument.into());
        }
        self.view.workbench_item = Some(WorkbenchItem::Frame(path.as_ref().to_owned()));
        self.view.workbench_offset = Vector2D::zero();
        Ok(())
    }

    pub fn edit_animation<T: AsRef<str>>(&mut self, name: T) -> Result<(), DocumentError> {
        if !self.sheet.has_animation(&name) {
            return Err(DocumentError::AnimationNotInDocument.into());
        }
        self.view.workbench_item = Some(WorkbenchItem::Animation(name.as_ref().to_owned()));
        self.view.workbench_offset = Vector2D::zero();
        self.view.timeline_clock = Duration::new(0, 0);
        self.persistent.timeline_is_playing = false;
        Ok(())
    }

    fn begin_rename<T: AsRef<str>>(&mut self, old_name: T) {
        self.transient = Some(Transient::Rename(Rename {
            new_name: old_name.as_ref().to_owned(),
        }));
    }

    pub fn create_animation(&mut self) -> Result<(), DocumentError> {
        let animation_name = {
            let animation = self.sheet.add_animation();
            let animation_name = animation.get_name().to_owned();
            animation_name
        };
        self.select_animations(&MultiSelection::new(vec![animation_name.clone()]))?;
        self.begin_rename(&animation_name);
        self.edit_animation(animation_name)
    }

    pub fn insert_keyframes_before<T: AsRef<Path>>(
        &mut self,
        paths: Vec<T>,
        next_frame_index: usize,
    ) -> Result<(), DocumentError> {
        let animation_name = match &self.view.workbench_item {
            Some(WorkbenchItem::Animation(animation_name)) => Some(animation_name.to_owned()),
            _ => None,
        }
        .ok_or(DocumentError::NotEditingAnyAnimation)?;
        for path in paths.iter().rev() {
            self.sheet
                .get_animation_mut(&animation_name)
                .ok_or(DocumentError::AnimationNotInDocument)?
                .create_frame(path, next_frame_index)?;
        }
        let new_frame_indices = next_frame_index..(next_frame_index + paths.len());
        self.select_keyframes(&MultiSelection::new(new_frame_indices.collect()))
    }

    pub fn reorder_keyframes(&mut self, new_index: usize) -> Result<(), DocumentError> {
        let selection = match &self.view.selection {
            Some(Selection::Keyframe(i)) => Some(i.clone()),
            _ => None,
        }
        .ok_or(DocumentError::NoKeyframeSelected)?;

        let mut frame_indexes: Vec<usize> = selection.items.clone().into_iter().collect();
        frame_indexes.sort();

        let animation = self.get_workbench_animation_mut()?;

        let mut affected_frames = Vec::with_capacity(frame_indexes.len());
        for index in frame_indexes.iter().rev() {
            affected_frames.push(animation.take_frame(*index)?);
        }

        let num_affected_frames_before_insert_point =
            frame_indexes.iter().filter(|i| **i < new_index).count();
        let insert_index = new_index - num_affected_frames_before_insert_point;

        for keyframe in affected_frames {
            animation.insert_frame(keyframe, insert_index)?;
        }

        let frame_times = animation.get_frame_times().clone();

        let new_selected_indexes = (insert_index..(insert_index + frame_indexes.len())).collect();
        self.view.selection = Some(Selection::Keyframe(MultiSelection::new(
            new_selected_indexes,
        )));

        let timeline_pos = *frame_times
            .get(insert_index)
            .ok_or(DocumentError::InvalidKeyframeIndex)?;
        self.view.timeline_clock = Duration::from_millis(u64::from(timeline_pos));

        Ok(())
    }

    pub fn begin_keyframe_duration_drag(
        &mut self,
        frame_being_dragged: usize,
        reference_clock: u32,
    ) -> Result<(), DocumentError> {
        let animation_name = match &self.view.workbench_item {
            Some(WorkbenchItem::Animation(animation_name)) => Some(animation_name.to_owned()),
            _ => None,
        }
        .ok_or(DocumentError::NotEditingAnyAnimation)?;

        let frame_indexes = match &self.view.selection {
            Some(Selection::Keyframe(i)) => Some(i.clone()),
            _ => None,
        }
        .ok_or(DocumentError::NoKeyframeSelected)?;

        let mut initial_duration = HashMap::new();
        for index in frame_indexes.items {
            let keyframe = self
                .sheet
                .get_animation(&animation_name)
                .ok_or(DocumentError::AnimationNotInDocument)?
                .get_frame(index)
                .ok_or(DocumentError::InvalidKeyframeIndex)?;
            let duration = keyframe.get_duration();
            initial_duration.insert(index, duration);
        }

        self.transient = Some(Transient::KeyframeDuration(KeyframeDuration {
            initial_duration: initial_duration,
            frame_being_dragged: frame_being_dragged,
            reference_clock: reference_clock,
        }));

        Ok(())
    }

    pub fn update_keyframe_duration_drag(
        &mut self,
        clock_at_cursor: u32,
        minimum_duration: u32,
    ) -> Result<(), DocumentError> {
        let animation_name = self.get_workbench_animation()?.get_name().to_owned();

        let frame_indexes = match &self.view.selection {
            Some(Selection::Keyframe(i)) => Some(i.clone()),
            _ => None,
        }
        .ok_or(DocumentError::NoKeyframeSelected)?;

        let keyframe_duration = match &self.transient {
            Some(Transient::KeyframeDuration(x)) => Some(x.clone()),
            _ => None,
        }
        .ok_or(DocumentError::NotAdjustingKeyframeDuration)?;

        let animation = self
            .sheet
            .get_animation_mut(&animation_name)
            .ok_or(DocumentError::AnimationNotInDocument)?;

        let reference_clock = keyframe_duration.reference_clock as i32;
        let clock_at_cursor = clock_at_cursor as i32;
        let duration_delta_up_to_dragged_frame = clock_at_cursor - reference_clock;

        let duration_delta_per_frame = duration_delta_up_to_dragged_frame
            / frame_indexes
                .items
                .iter()
                .filter(|i| **i <= keyframe_duration.frame_being_dragged)
                .count()
                .max(1) as i32;

        for index in frame_indexes.items {
            let keyframe = animation
                .get_frame_mut(index)
                .ok_or(DocumentError::InvalidKeyframeIndex)?;
            let old_duration = *keyframe_duration
                .initial_duration
                .get(&index)
                .ok_or(DocumentError::MissingKeyframeDurationData)?;
            let new_duration = (old_duration as i32 + duration_delta_per_frame)
                .max(minimum_duration as i32) as u32;
            keyframe.set_duration(new_duration);
        }

        let frame_times = animation.get_frame_times();
        let timeline_pos = *frame_times
            .get(frame_indexes.last_touched_in_range)
            .ok_or(DocumentError::InvalidKeyframeIndex)?;
        self.view.timeline_clock = Duration::from_millis(u64::from(timeline_pos));

        Ok(())
    }

    pub fn begin_keyframe_drag(&mut self) {
        self.transient = Some(Transient::TimelineFrameDrag);
    }

    pub fn begin_keyframe_offset_drag(&mut self) -> Result<(), DocumentError> {
        let animation_name = self.get_workbench_animation()?.get_name().to_owned();
        let frame_indexes = match &self.view.selection {
            Some(Selection::Keyframe(i)) => Some(i.clone()),
            _ => None,
        }
        .ok_or(DocumentError::NoKeyframeSelected)?;

        let animation = self
            .sheet
            .get_animation(animation_name)
            .ok_or(DocumentError::AnimationNotInDocument)?;

        let mut initial_offset = HashMap::new();
        for keyframe_index in frame_indexes.items {
            let keyframe = animation
                .get_frame(keyframe_index)
                .ok_or(DocumentError::InvalidKeyframeIndex)?;
            initial_offset.insert(keyframe_index, keyframe.get_offset());
        }

        self.transient = Some(Transient::KeyframePosition(KeyframePosition {
            initial_offset: initial_offset,
        }));

        Ok(())
    }

    pub fn update_keyframe_offset_drag(
        &mut self,
        mut mouse_delta: Vector2D<f32>,
        both_axis: bool,
    ) -> Result<(), DocumentError> {
        let zoom = self.view.get_workbench_zoom_factor();
        let animation_name = self.get_workbench_animation()?.get_name().to_owned();
        let frame_indexes = match &self.view.selection {
            Some(Selection::Keyframe(indexes)) => Some(indexes.clone()),
            _ => None,
        }
        .ok_or(DocumentError::NoKeyframeSelected)?;

        let keyframe_position = match &self.transient {
            Some(Transient::KeyframePosition(x)) => Some(x),
            _ => None,
        }
        .ok_or(DocumentError::NotAdjustingKeyframePosition)?;

        if !both_axis {
            if mouse_delta.x.abs() > mouse_delta.y.abs() {
                mouse_delta.y = 0.0;
            } else {
                mouse_delta.x = 0.0;
            }
        }

        for index in frame_indexes.items {
            let old_offset = keyframe_position
                .initial_offset
                .get(&index)
                .ok_or(DocumentError::MissingKeyframePositionData)?;

            let new_offset = (old_offset.to_f32() + mouse_delta / zoom).floor().to_i32();

            let keyframe = self
                .sheet
                .get_animation_mut(&animation_name)
                .ok_or(DocumentError::AnimationNotInDocument)?
                .get_frame_mut(index)
                .ok_or(DocumentError::InvalidKeyframeIndex)?;
            keyframe.set_offset(new_offset);
        }

        Ok(())
    }

    pub fn create_hitbox(&mut self, mouse_position: Vector2D<f32>) -> Result<(), DocumentError> {
        let hitbox_name = {
            let frame_path = self.get_workbench_frame()?.get_source().to_owned();
            let frame = self
                .sheet
                .get_frame_mut(frame_path)
                .ok_or(DocumentError::FrameNotInDocument)?;

            let hitbox = frame.add_hitbox();
            hitbox.set_position(mouse_position.floor().to_i32());
            hitbox.get_name().to_owned()
        };
        self.select_hitboxes(&MultiSelection::new(vec![hitbox_name]))?;
        self.begin_hitbox_scale(ResizeAxis::SE)
    }

    pub fn begin_hitbox_scale(&mut self, axis: ResizeAxis) -> Result<(), DocumentError> {
        let frame_path = self.get_workbench_frame()?.get_source().to_owned();

        let hitbox_names = match &self.view.selection {
            Some(Selection::Hitbox(names)) => Some(names.items.clone()),
            _ => None,
        }
        .ok_or(DocumentError::NoHitboxSelected)?;

        let mut initial_state = HashMap::new();
        for name in &hitbox_names {
            let hitbox = self
                .sheet
                .get_frame(&frame_path)
                .ok_or(DocumentError::FrameNotInDocument)?
                .get_hitbox(name)
                .ok_or(DocumentError::InvalidHitboxName)?;
            initial_state.insert(
                name.to_owned(),
                HitboxInitialState {
                    position: hitbox.get_position(),
                    size: hitbox.get_size(),
                },
            );
        }

        self.transient = Some(Transient::HitboxSize(HitboxSize {
            axis: axis,
            initial_state: initial_state,
        }));

        Ok(())
    }

    pub fn update_hitbox_scale(
        &mut self,
        mut mouse_delta: Vector2D<f32>,
        preserve_aspect_ratio: bool,
    ) -> Result<(), DocumentError> {
        use ResizeAxis::*;

        let frame_path = self.get_workbench_frame()?.get_source().to_owned();
        let hitbox_names = match &self.view.selection {
            Some(Selection::Hitbox(n)) => Some(n.to_owned()),
            _ => None,
        }
        .ok_or(DocumentError::NoHitboxSelected)?;

        let hitbox_size = match &self.transient {
            Some(Transient::HitboxSize(x)) => Some(x),
            _ => None,
        }
        .ok_or(DocumentError::NotAdjustingHitboxSize)?;

        for hitbox_name in hitbox_names.items.iter() {
            let initial_state = hitbox_size
                .initial_state
                .get(hitbox_name)
                .ok_or(DocumentError::MissingHitboxSizeData)?;

            let initial_hitbox = Rect::new(
                initial_state.position.to_point(),
                initial_state.size.to_i32().to_size(),
            );

            let axis = hitbox_size.axis;
            if preserve_aspect_ratio && axis.is_diagonal() {
                let aspect_ratio = initial_hitbox.size.width.max(1) as f32
                    / initial_hitbox.size.height.max(1) as f32;
                let odd_axis_factor = if axis == NE || axis == SW { -1.0 } else { 1.0 };
                mouse_delta = if mouse_delta.x.abs() > mouse_delta.y.abs() {
                    vec2(
                        mouse_delta.x,
                        odd_axis_factor * (mouse_delta.x / aspect_ratio).round(),
                    )
                } else {
                    vec2(
                        odd_axis_factor * (mouse_delta.y * aspect_ratio).round(),
                        mouse_delta.y,
                    )
                };
            }

            let zoom = self.view.get_workbench_zoom_factor();
            let mouse_delta = (mouse_delta / zoom).round().to_i32();

            let bottom_left = point2(initial_hitbox.min_x(), initial_hitbox.max_y());
            let top_right = point2(initial_hitbox.max_x(), initial_hitbox.min_y());

            let new_hitbox = Rect::from_points(match axis {
                NW => vec![initial_hitbox.max(), initial_hitbox.origin + mouse_delta],
                NE => vec![bottom_left, top_right + mouse_delta],
                SW => vec![top_right, bottom_left + mouse_delta],
                SE => vec![initial_hitbox.origin, initial_hitbox.max() + mouse_delta],
                N => vec![
                    bottom_left,
                    point2(
                        initial_hitbox.max_x(),
                        initial_hitbox.min_y() + mouse_delta.y,
                    ),
                ],
                W => vec![
                    top_right,
                    point2(
                        initial_hitbox.min_x() + mouse_delta.x,
                        initial_hitbox.max_y(),
                    ),
                ],
                S => vec![
                    initial_hitbox.origin,
                    point2(
                        initial_hitbox.max_x(),
                        initial_hitbox.max_y() + mouse_delta.y,
                    ),
                ],
                E => vec![
                    initial_hitbox.origin,
                    point2(
                        initial_hitbox.max_x() + mouse_delta.x,
                        initial_hitbox.max_y(),
                    ),
                ],
            });

            let hitbox = self
                .sheet
                .get_frame_mut(&frame_path)
                .ok_or(DocumentError::FrameNotInDocument)?
                .get_hitbox_mut(&hitbox_name)
                .ok_or(DocumentError::InvalidHitboxName)?;

            hitbox.set_position(new_hitbox.origin.to_vector());
            hitbox.set_size(new_hitbox.size.to_u32().to_vector());
        }

        Ok(())
    }

    pub fn begin_hitbox_drag(&mut self) -> Result<(), DocumentError> {
        let frame_path = self.get_workbench_frame()?.get_source().to_owned();
        let hitbox_names = match &self.view.selection {
            Some(Selection::Hitbox(n)) => Some(n.to_owned()),
            _ => None,
        }
        .ok_or(DocumentError::NoHitboxSelected)?;

        let mut initial_offset = HashMap::new();
        for hitbox_name in hitbox_names.items.iter() {
            let hitbox = self
                .sheet
                .get_frame(&frame_path)
                .ok_or(DocumentError::FrameNotInDocument)?
                .get_hitbox(hitbox_name)
                .ok_or(DocumentError::InvalidHitboxName)?;
            initial_offset.insert(hitbox_name.to_owned(), hitbox.get_position());
        }

        self.transient = Some(Transient::HitboxPosition(HitboxPosition {
            initial_offset: initial_offset,
        }));

        Ok(())
    }

    pub fn update_hitbox_drag(
        &mut self,
        mut mouse_delta: Vector2D<f32>,
        both_axis: bool,
    ) -> Result<(), DocumentError> {
        let zoom = self.view.get_workbench_zoom_factor();

        let frame_path = self.get_workbench_frame()?.get_source().to_owned();
        let hitbox_names = match &self.view.selection {
            Some(Selection::Hitbox(n)) => Some(n.to_owned()),
            _ => None,
        }
        .ok_or(DocumentError::NoHitboxSelected)?;

        let hitbox_position = match &self.transient {
            Some(Transient::HitboxPosition(x)) => Some(x),
            _ => None,
        }
        .ok_or(DocumentError::NotAdjustingHitboxPosition)?;

        for hitbox_name in hitbox_names.items.iter() {
            let old_offset = hitbox_position
                .initial_offset
                .get(hitbox_name)
                .ok_or(DocumentError::MissingHitboxPositionData)?;

            if !both_axis {
                if mouse_delta.x.abs() > mouse_delta.y.abs() {
                    mouse_delta.y = 0.0;
                } else {
                    mouse_delta.x = 0.0;
                }
            }

            let new_offset = (old_offset.to_f32() + mouse_delta / zoom).floor().to_i32();

            let hitbox = self
                .sheet
                .get_frame_mut(&frame_path)
                .ok_or(DocumentError::FrameNotInDocument)?
                .get_hitbox_mut(&hitbox_name)
                .ok_or(DocumentError::InvalidHitboxName)?;
            hitbox.set_position(new_offset);
        }

        Ok(())
    }

    pub fn toggle_playback(&mut self) -> Result<(), DocumentError> {
        let mut new_timeline_clock = self.view.timeline_clock;
        {
            let animation = self.get_workbench_animation()?;

            if !self.persistent.timeline_is_playing {
                if let Some(d) = animation.get_duration() {
                    if d > 0
                        && !animation.is_looping()
                        && self.view.timeline_clock.as_millis() >= u128::from(d)
                    {
                        new_timeline_clock = Duration::new(0, 0);
                    }
                }
            }
        }

        self.persistent.timeline_is_playing = !self.persistent.timeline_is_playing;
        self.view.timeline_clock = new_timeline_clock;

        Ok(())
    }

    pub fn snap_to_previous_frame(&mut self) -> Result<(), DocumentError> {
        let clock = {
            let animation = self.get_workbench_animation()?;

            if animation.get_num_frames() == 0 {
                return Ok(());
            }

            let mut cursor = 0 as u64;
            let now = self.view.timeline_clock.as_millis() as u64;
            let frame_times: Vec<u64> = animation
                .frames_iter()
                .map(|f| {
                    let t = cursor;
                    cursor += u64::from(f.get_duration());
                    t
                })
                .collect();

            match frame_times.iter().rev().find(|t1| **t1 < now) {
                Some(t1) => *t1,
                None => match frame_times.iter().next() {
                    Some(t) => *t,
                    None => 0,
                },
            }
        };

        self.update_timeline_scrub(Duration::from_millis(clock))
    }

    pub fn snap_to_next_frame(&mut self) -> Result<(), DocumentError> {
        let clock = {
            let animation = self.get_workbench_animation()?;

            if animation.get_num_frames() == 0 {
                return Ok(());
            }

            let mut cursor = 0 as u64;
            let now = self.view.timeline_clock.as_millis() as u64;
            let frame_times: Vec<u64> = animation
                .frames_iter()
                .map(|f| {
                    let t = cursor;
                    cursor += u64::from(f.get_duration());
                    t
                })
                .collect();

            match frame_times.iter().find(|t1| **t1 > now) {
                Some(t1) => *t1,
                None => match frame_times.iter().last() {
                    Some(t) => *t,
                    None => 0,
                },
            }
        };

        self.update_timeline_scrub(Duration::from_millis(clock))
    }

    pub fn toggle_looping(&mut self) -> Result<(), DocumentError> {
        let animation = self.get_workbench_animation_mut()?;
        animation.set_is_looping(!animation.is_looping());
        Ok(())
    }

    pub fn update_timeline_scrub(&mut self, new_time: Duration) -> Result<(), DocumentError> {
        let animation = self.get_workbench_animation()?;
        let (index, _) = animation
            .get_frame_at(new_time)
            .ok_or(DocumentError::NoKeyframeForThisTime)?;
        self.select_keyframes(&MultiSelection::new(vec![index]))?;
        self.view.timeline_clock = new_time;
        Ok(())
    }

    pub fn nudge_selection(
        &mut self,
        direction: Vector2D<i32>,
        large: bool,
    ) -> Result<(), DocumentError> {
        let amplitude = if large { 10 } else { 1 };
        let offset = direction * amplitude;
        match self.view.selection.clone() {
            Some(Selection::Animation(_)) => {}
            Some(Selection::Frame(_)) => {}
            Some(Selection::Hitbox(names)) => {
                for name in names.items {
                    let hitbox = self
                        .get_workbench_frame_mut()?
                        .get_hitbox_mut(name)
                        .ok_or(DocumentError::InvalidHitboxName)?;
                    hitbox.set_position(hitbox.get_position() + offset);
                }
            }
            Some(Selection::Keyframe(indexes)) => {
                for index in indexes.items {
                    let animation_name = self.get_workbench_animation()?.get_name().to_owned();
                    let keyframe = self
                        .sheet
                        .get_animation_mut(animation_name)
                        .ok_or(DocumentError::AnimationNotInDocument)?
                        .get_frame_mut(index)
                        .ok_or(DocumentError::InvalidKeyframeIndex)?;
                    keyframe.set_offset(keyframe.get_offset() + offset);
                }
            }
            None => {}
        };
        Ok(())
    }

    pub fn delete_selection(&mut self) -> Result<(), DocumentError> {
        match &self.view.selection {
            Some(Selection::Animation(names)) => {
                for name in &names.items {
                    self.sheet.delete_animation(name);
                }
            }
            Some(Selection::Frame(paths)) => {
                for path in &paths.items {
                    self.sheet.delete_frame(&path);
                }
            }
            Some(Selection::Hitbox(names)) => {
                let frame_path = self.get_workbench_frame()?.get_source().to_owned();
                for name in &names.items {
                    self.sheet.delete_hitbox(&frame_path, name);
                }
            }
            Some(Selection::Keyframe(indexes)) => {
                let animation_name = self.get_workbench_animation()?.get_name().to_owned();
                for index in &indexes.items {
                    self.sheet.delete_keyframe(&animation_name, *index);
                }
            }
            None => {}
        };
        self.view.selection = None;
        Ok(())
    }

    pub fn begin_rename_selection(&mut self) {
        match self.view.selection.clone() {
            Some(Selection::Animation(names)) | Some(Selection::Hitbox(names)) => {
                self.begin_rename(names.last_touched_in_range.clone())
            }
            Some(Selection::Frame(_)) => (),
            Some(Selection::Keyframe(_)) => (),
            None => {}
        };
    }

    pub fn end_rename_selection(&mut self) -> Result<(), DocumentError> {
        let new_name = match &self.transient {
            Some(Transient::Rename(x)) => Some(x.new_name.clone()),
            _ => None,
        }
        .ok_or(DocumentError::NotRenaming)?;

        match self.view.selection.clone() {
            Some(Selection::Animation(names)) => {
                let old_name = names.last_touched_in_range;
                if old_name != new_name {
                    if self.sheet.has_animation(&new_name) {
                        return Err(DocumentError::AnimationAlreadyExists.into());
                    }
                    self.sheet.rename_animation(&old_name, &new_name)?;
                    self.select_animations(&MultiSelection::new(vec![new_name.clone()]))?;
                    if Some(WorkbenchItem::Animation(old_name.clone())) == self.view.workbench_item
                    {
                        self.view.workbench_item = Some(WorkbenchItem::Animation(new_name.clone()));
                    }
                }
            }
            Some(Selection::Hitbox(names)) => {
                let old_name = names.last_touched_in_range;
                if old_name != new_name {
                    let frame_path = self.get_workbench_frame()?.get_source().to_owned();
                    if self
                        .sheet
                        .get_frame(&frame_path)
                        .ok_or(DocumentError::FrameNotInDocument)?
                        .has_hitbox(&new_name)
                    {
                        return Err(DocumentError::HitboxAlreadyExists.into());
                    }
                    self.sheet
                        .get_frame_mut(&frame_path)
                        .ok_or(DocumentError::FrameNotInDocument)?
                        .rename_hitbox(&old_name, &new_name)?;
                    self.select_hitboxes(&MultiSelection::new(vec![new_name.clone()]))?;
                }
            }
            _ => (),
        }
        Ok(())
    }

    fn get_export_settings_edit_mut(&mut self) -> Result<&mut ExportSettings, DocumentError> {
        self.persistent
            .export_settings_edit
            .as_mut()
            .ok_or(DocumentError::NotExporting.into())
    }

    fn begin_export_as(&mut self) {
        self.persistent.export_settings_edit = self
            .sheet
            .get_export_settings()
            .as_ref()
            .cloned()
            .or_else(|| Some(ExportSettings::new()));
    }

    fn cancel_export_as(&mut self) {
        self.persistent.export_settings_edit = None;
    }

    fn end_set_export_texture_destination<T: AsRef<Path>>(
        &mut self,
        texture_destination: T,
    ) -> Result<(), DocumentError> {
        self.get_export_settings_edit_mut()?.texture_destination =
            texture_destination.as_ref().to_owned();
        Ok(())
    }

    fn end_set_export_metadata_destination<T: AsRef<Path>>(
        &mut self,
        metadata_destination: T,
    ) -> Result<(), DocumentError> {
        self.get_export_settings_edit_mut()?.metadata_destination =
            metadata_destination.as_ref().to_owned();
        Ok(())
    }

    fn end_set_export_metadata_paths_root<T: AsRef<Path>>(
        &mut self,
        metadata_paths_root: T,
    ) -> Result<(), DocumentError> {
        self.get_export_settings_edit_mut()?.metadata_paths_root =
            metadata_paths_root.as_ref().to_owned();
        Ok(())
    }

    fn end_set_export_format(&mut self, format: ExportFormat) -> Result<(), DocumentError> {
        self.get_export_settings_edit_mut()?.format = format;
        Ok(())
    }

    fn end_export_as(&mut self) -> Result<(), DocumentError> {
        let export_settings = self.get_export_settings_edit_mut()?.clone();
        self.sheet.set_export_settings(export_settings);
        self.persistent.export_settings_edit = None;
        Ok(())
    }

    pub fn begin_close(&mut self) {
        if self.persistent.close_state == None {
            self.persistent.close_state = Some(if self.is_saved() {
                CloseState::Allowed
            } else {
                CloseState::Requested
            });
        }
    }

    pub fn cancel_close(&mut self) {
        self.persistent.close_state = None;
    }

    pub fn process_command(&mut self, command: DocumentCommand) -> Result<(), DocumentError> {
        use DocumentCommand::*;

        match &command {
            MarkAsSaved(_, v) => self.persistent.disk_version = *v,
            EndImport(_, f) => self.sheet.add_frame(f),
            BeginExportAs => self.begin_export_as(),
            CancelExportAs => self.cancel_export_as(),
            EndSetExportTextureDestination(_, d) => self.end_set_export_texture_destination(d)?,
            EndSetExportMetadataDestination(_, d) => self.end_set_export_metadata_destination(d)?,
            EndSetExportMetadataPathsRoot(_, d) => self.end_set_export_metadata_paths_root(d)?,
            EndSetExportFormat(_, f) => self.end_set_export_format(f.clone())?,
            EndExportAs => self.end_export_as()?,
            SwitchToContentTab(t) => self.view.content_tab = *t,
            ClearSelection => self.clear_selection(),
            SelectFrames(s) => self.select_frames(&s)?,
            SelectAnimations(s) => self.select_animations(&s)?,
            SelectHitboxes(s) => self.select_hitboxes(&s)?,
            SelectKeyframes(s) => self.select_keyframes(&s)?,
            EditFrame(p) => self.edit_frame(&p)?,
            EditAnimation(a) => self.edit_animation(&a)?,
            CreateAnimation => self.create_animation()?,
            BeginFramesDrag => self.transient = Some(Transient::ContentFramesDrag),
            InsertKeyframesBefore(frames, n) => self.insert_keyframes_before(frames.clone(), *n)?,
            ReorderKeyframes(i) => self.reorder_keyframes(*i)?,
            BeginKeyframeDurationDrag(c, i) => self.begin_keyframe_duration_drag(*i, *c)?,
            UpdateKeyframeDurationDrag(d, m) => self.update_keyframe_duration_drag(*d, *m)?,
            BeginKeyframeDrag => self.begin_keyframe_drag(),
            BeginKeyframeOffsetDrag => self.begin_keyframe_offset_drag()?,
            UpdateKeyframeOffsetDrag(o, b) => self.update_keyframe_offset_drag(*o, *b)?,
            WorkbenchZoomIn => self.view.workbench_zoom_in(),
            WorkbenchZoomOut => self.view.workbench_zoom_out(),
            WorkbenchResetZoom => self.view.workbench_reset_zoom(),
            WorkbenchCenter => self.view.workbench_center(),
            Pan(delta) => self.view.pan(*delta),
            CreateHitbox(p) => self.create_hitbox(*p)?,
            BeginHitboxScale(axis) => self.begin_hitbox_scale(*axis)?,
            UpdateHitboxScale(delta, ar) => self.update_hitbox_scale(*delta, *ar)?,
            BeginHitboxDrag => self.begin_hitbox_drag()?,
            UpdateHitboxDrag(delta, b) => self.update_hitbox_drag(*delta, *b)?,
            TogglePlayback => self.toggle_playback()?,
            SnapToPreviousFrame => self.snap_to_previous_frame()?,
            SnapToNextFrame => self.snap_to_next_frame()?,
            ToggleLooping => self.toggle_looping()?,
            TimelineZoomIn => self.view.timeline_zoom_in(),
            TimelineZoomOut => self.view.timeline_zoom_out(),
            TimelineResetZoom => self.view.timeline_reset_zoom(),
            BeginScrub => self.transient = Some(Transient::TimelineScrub),
            UpdateScrub(t) => self.update_timeline_scrub(*t)?,
            NudgeSelection(d, l) => self.nudge_selection(*d, *l)?,
            DeleteSelection => self.delete_selection()?,
            BeginRenameSelection => self.begin_rename_selection(),
            UpdateRenameSelection(n) => {
                self.transient = Some(Transient::Rename(Rename {
                    new_name: n.to_owned(),
                }))
            }
            EndRenameSelection => self.end_rename_selection()?,
            Close => self.begin_close(),
            CloseAfterSaving => self.persistent.close_state = Some(CloseState::Saving),
            CloseWithoutSaving => self.persistent.close_state = Some(CloseState::Allowed),
            CancelClose => self.persistent.close_state = None,
            EndFramesDrag
            | EndKeyframeDurationDrag
            | EndKeyframeDrag
            | EndKeyframeOffsetDrag
            | EndHitboxScale
            | EndHitboxDrag
            | EndScrub => (),
        };

        if !Transient::is_transient_command(&command) {
            self.transient = None;
        }

        self.record_command(command);

        Ok(())
    }
}
