use core::cmp::Ordering;
use euclid::default::*;
use euclid::rect;
use pathdiff::diff_paths;
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

pub use self::compat::version2::*;
use self::constants::*;

pub mod compat;

pub mod constants {
    pub const MAX_ANIMATION_NAME_LENGTH: usize = 32;
    pub const MAX_HITBOX_NAME_LENGTH: usize = 32;
}

#[derive(Error, Debug)]
pub enum SheetError {
    #[error("Animation was not found")]
    AnimationNotFound,
    #[error("Hitbox was not found")]
    HitboxNotFound,
    #[error("Animation name too long")]
    AnimationNameTooLong,
    #[error("Hitbox name too long")]
    HitboxNameTooLong,
    #[error("Error converting an absolute path to a relative path")]
    AbsoluteToRelativePath,
    #[error("Invalid frame index")]
    InvalidFrameIndex,
}

impl Sheet {
    pub fn with_relative_paths<T: AsRef<Path>>(&self, relative_to: T) -> Result<Sheet, SheetError> {
        let mut sheet = self.clone();
        for frame in sheet.frames_iter_mut() {
            frame.source = diff_paths(&frame.source, relative_to.as_ref())
                .ok_or(SheetError::AbsoluteToRelativePath)?;
        }
        for animation in sheet.animations.iter_mut() {
            for keyframe in animation.frames_iter_mut() {
                keyframe.frame = diff_paths(&keyframe.frame, relative_to.as_ref())
                    .ok_or(SheetError::AbsoluteToRelativePath)?;
            }
        }
        if let Some(e) = sheet.export_settings {
            sheet.export_settings = e.with_relative_paths(relative_to).ok();
        }
        Ok(sheet)
    }

    pub fn with_absolute_paths<T: AsRef<Path>>(&self, relative_to: T) -> Sheet {
        let mut sheet = self.clone();
        for frame in sheet.frames_iter_mut() {
            frame.source = relative_to.as_ref().join(&frame.source);
        }
        for animation in sheet.animations.iter_mut() {
            for keyframe in animation.frames_iter_mut() {
                keyframe.frame = relative_to.as_ref().join(&&keyframe.frame);
            }
        }
        if let Some(e) = sheet.export_settings {
            sheet.export_settings = Some(e.with_absolute_paths(relative_to));
        }
        sheet
    }

    pub fn frames_iter(&self) -> std::slice::Iter<'_, Frame> {
        self.frames.iter()
    }

    pub fn frames_iter_mut(&mut self) -> std::slice::IterMut<'_, Frame> {
        self.frames.iter_mut()
    }

    pub fn animations_iter(&self) -> std::slice::Iter<'_, Animation> {
        self.animations.iter()
    }

    pub fn has_frame<T: AsRef<Path>>(&self, path: T) -> bool {
        self.frames.iter().any(|f| f.source == path.as_ref())
    }

    pub fn has_animation<T: AsRef<str>>(&self, name: T) -> bool {
        self.animations.iter().any(|a| a.name == name.as_ref())
    }

    pub fn add_frame<T: AsRef<Path>>(&mut self, path: T) {
        if self.has_frame(&path) {
            return;
        }
        let frame = Frame::new(path);
        self.frames.push(frame);
    }

    pub fn add_animation(&mut self) -> &mut Animation {
        let mut name = "New Animation".to_owned();
        let mut index = 2;
        while self.has_animation(&name) {
            name = format!("New Animation {}", index);
            index += 1;
        }
        let animation = Animation::new(&name);
        self.animations.push(animation);
        self.animations.last_mut().unwrap()
    }

    pub fn get_frame<T: AsRef<Path>>(&self, path: T) -> Option<&Frame> {
        self.frames.iter().find(|f| f.source == path.as_ref())
    }

    pub fn get_frame_mut<T: AsRef<Path>>(&mut self, path: T) -> Option<&mut Frame> {
        self.frames.iter_mut().find(|f| f.source == path.as_ref())
    }

    pub fn get_animation<T: AsRef<str>>(&self, name: T) -> Option<&Animation> {
        self.animations.iter().find(|a| a.name == name.as_ref())
    }

    pub fn get_animation_mut<T: AsRef<str>>(&mut self, name: T) -> Option<&mut Animation> {
        self.animations.iter_mut().find(|a| a.name == name.as_ref())
    }

    pub fn get_export_settings(&self) -> &Option<ExportSettings> {
        &self.export_settings
    }

    pub fn set_export_settings(&mut self, export_settings: ExportSettings) {
        self.export_settings = Some(export_settings);
    }

    pub fn rename_animation<T: AsRef<str>, U: AsRef<str>>(
        &mut self,
        old_name: T,
        new_name: U,
    ) -> Result<(), SheetError> {
        if new_name.as_ref().len() > MAX_ANIMATION_NAME_LENGTH {
            return Err(SheetError::AnimationNameTooLong.into());
        }
        let animation = self
            .get_animation_mut(old_name)
            .ok_or(SheetError::AnimationNotFound)?;
        animation.name = new_name.as_ref().to_owned();
        Ok(())
    }

    pub fn delete_frame<T: AsRef<Path>>(&mut self, path: T) {
        self.frames.retain(|f| f.source != path.as_ref());
        for animation in self.animations.iter_mut() {
            animation.timeline.retain(|kf| kf.frame != path.as_ref())
        }
    }

    pub fn delete_hitbox<T: AsRef<Path>, U: AsRef<str>>(&mut self, path: T, name: U) {
        if let Some(frame) = self.get_frame_mut(path.as_ref()) {
            frame.hitboxes.retain(|h| h.name != name.as_ref());
        }
    }

    pub fn delete_animation<T: AsRef<str>>(&mut self, name: T) {
        self.animations.retain(|a| a.name != name.as_ref());
    }

    pub fn delete_keyframe<T: AsRef<str>>(&mut self, animation_name: T, frame_index: usize) {
        if let Some(animation) = self.get_animation_mut(animation_name) {
            if frame_index < animation.timeline.len() {
                animation.timeline.remove(frame_index);
            }
        }
    }
}

impl Animation {
    pub fn new<T: AsRef<str>>(name: T) -> Animation {
        Animation {
            name: name.as_ref().to_owned(),
            timeline: vec![],
            is_looping: true,
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_num_frames(&self) -> usize {
        self.timeline.len()
    }

    pub fn is_looping(&self) -> bool {
        self.is_looping
    }

    pub fn set_is_looping(&mut self, new_is_looping: bool) {
        self.is_looping = new_is_looping;
    }

    pub fn get_duration(&self) -> Option<u32> {
        if self.timeline.is_empty() {
            return None;
        }
        Some(self.timeline.iter().map(|f| f.duration).sum())
    }

    pub fn get_frame(&self, index: usize) -> Option<&Keyframe> {
        if index >= self.timeline.len() {
            return None;
        }
        Some(&self.timeline[index])
    }

    pub fn get_frame_mut(&mut self, index: usize) -> Option<&mut Keyframe> {
        if index >= self.timeline.len() {
            return None;
        }
        Some(&mut self.timeline[index])
    }

    pub fn get_frame_at(&self, time: Duration) -> Option<(usize, &Keyframe)> {
        let duration = match self.get_duration() {
            None => return None,
            Some(0) => return None,
            Some(d) => d,
        };
        let time = if self.is_looping {
            Duration::from_millis(time.as_millis() as u64 % u64::from(duration))
        } else {
            time
        };
        let mut cursor = Duration::new(0, 0);
        for (index, frame) in self.timeline.iter().enumerate() {
            cursor += Duration::from_millis(u64::from(frame.duration));
            if time < cursor {
                return Some((index, frame));
            }
        }
        Some((
            self.timeline.len() - 1,
            self.timeline.iter().last().unwrap(),
        )) // TODO no unwrap
    }

    pub fn get_frame_times(&self) -> Vec<u64> {
        let mut cursor = 0;
        self.frames_iter()
            .map(|f| {
                let t = cursor;
                cursor += u64::from(f.get_duration());
                t
            })
            .collect()
    }

    pub fn create_frame<T: AsRef<Path>>(
        &mut self,
        frame: T,
        index: usize,
    ) -> Result<(), SheetError> {
        // TODO validate that frame exists in sheet!
        if index > self.timeline.len() {
            return Err(SheetError::InvalidFrameIndex.into());
        }
        let keyframe = Keyframe::new(frame);
        self.timeline.insert(index, keyframe);
        Ok(())
    }

    pub fn insert_frame(&mut self, keyframe: Keyframe, index: usize) -> Result<(), SheetError> {
        if index > self.timeline.len() {
            return Err(SheetError::InvalidFrameIndex.into());
        }
        self.timeline.insert(index, keyframe);
        Ok(())
    }

    pub fn take_frame(&mut self, index: usize) -> Result<Keyframe, SheetError> {
        if index >= self.timeline.len() {
            return Err(SheetError::InvalidFrameIndex.into());
        }
        Ok(self.timeline.remove(index))
    }

    pub fn frames_iter(&self) -> std::slice::Iter<'_, Keyframe> {
        self.timeline.iter()
    }

    pub fn frames_iter_mut(&mut self) -> std::slice::IterMut<'_, Keyframe> {
        self.timeline.iter_mut()
    }
}

impl Ord for Animation {
    fn cmp(&self, other: &Animation) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for Animation {
    fn partial_cmp(&self, other: &Animation) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Frame {
    pub fn new<T: AsRef<Path>>(path: T) -> Frame {
        Frame {
            source: path.as_ref().to_owned(),
            hitboxes: vec![],
        }
    }

    pub fn get_source(&self) -> &Path {
        &self.source
    }

    pub fn hitboxes_iter(&self) -> std::slice::Iter<'_, Hitbox> {
        self.hitboxes.iter()
    }

    pub fn get_hitbox<T: AsRef<str>>(&self, name: T) -> Option<&Hitbox> {
        self.hitboxes.iter().find(|a| a.name == name.as_ref())
    }

    pub fn get_hitbox_mut<T: AsRef<str>>(&mut self, name: T) -> Option<&mut Hitbox> {
        self.hitboxes.iter_mut().find(|a| a.name == name.as_ref())
    }

    pub fn has_hitbox<T: AsRef<str>>(&self, name: T) -> bool {
        self.hitboxes.iter().any(|a| a.name == name.as_ref())
    }

    pub fn add_hitbox(&mut self) -> &mut Hitbox {
        let mut name = "New Hitbox".to_owned();
        let mut index = 2;
        while self.has_hitbox(&name) {
            name = format!("New Hitbox {}", index);
            index += 1;
        }

        self.hitboxes.push(Hitbox {
            name,
            geometry: Shape::Rectangle(Rectangle {
                top_left: (0, 0),
                size: (0, 0),
            }),
        });
        self.hitboxes.last_mut().unwrap() // TODO no unwrap?
    }

    pub fn rename_hitbox<T: AsRef<str>, U: AsRef<str>>(
        &mut self,
        old_name: T,
        new_name: U,
    ) -> Result<(), SheetError> {
        if new_name.as_ref().len() > MAX_HITBOX_NAME_LENGTH {
            return Err(SheetError::HitboxNameTooLong.into());
        }
        let hitbox = self
            .get_hitbox_mut(old_name)
            .ok_or(SheetError::HitboxNotFound)?;
        hitbox.name = new_name.as_ref().to_owned();
        Ok(())
    }
}

impl Ord for Frame {
    fn cmp(&self, other: &Frame) -> Ordering {
        self.source
            .to_string_lossy()
            .cmp(&other.source.to_string_lossy())
    }
}

impl PartialOrd for Frame {
    fn partial_cmp(&self, other: &Frame) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Hitbox {
    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_rectangle(&self) -> Rect<i32> {
        match &self.geometry {
            Shape::Rectangle(r) => {
                rect(r.top_left.0, r.top_left.1, r.size.0 as i32, r.size.1 as i32)
            }
        }
    }

    pub fn get_position(&self) -> Vector2D<i32> {
        match &self.geometry {
            Shape::Rectangle(r) => r.top_left.into(),
        }
    }

    pub fn get_size(&self) -> Vector2D<u32> {
        match &self.geometry {
            Shape::Rectangle(r) => r.size.into(),
        }
    }

    pub fn set_position(&mut self, new_position: Vector2D<i32>) {
        match &mut self.geometry {
            Shape::Rectangle(r) => {
                r.top_left = new_position.to_tuple();
            }
        }
    }

    pub fn set_size(&mut self, new_size: Vector2D<u32>) {
        match &mut self.geometry {
            Shape::Rectangle(r) => {
                r.size = new_size.to_tuple();
            }
        }
    }
}

impl Ord for Hitbox {
    fn cmp(&self, other: &Hitbox) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for Hitbox {
    fn partial_cmp(&self, other: &Hitbox) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Keyframe {
    pub fn new<T: AsRef<Path>>(frame: T) -> Keyframe {
        Keyframe {
            frame: frame.as_ref().to_owned(),
            duration: 100, // TODO better default?
            offset: (0, 0),
        }
    }

    pub fn get_frame(&self) -> &Path {
        &self.frame
    }

    pub fn get_duration(&self) -> u32 {
        self.duration
    }

    pub fn get_offset(&self) -> Vector2D<i32> {
        self.offset.into()
    }

    pub fn set_duration(&mut self, new_duration: u32) {
        self.duration = new_duration;
    }

    pub fn set_offset(&mut self, new_offset: Vector2D<i32>) {
        self.offset = new_offset.to_tuple();
    }
}

impl ExportFormat {
    pub fn with_relative_paths<T: AsRef<Path>>(
        &self,
        relative_to: T,
    ) -> Result<ExportFormat, SheetError> {
        match self {
            ExportFormat::Template(p) => Ok(ExportFormat::Template(
                diff_paths(&p, relative_to.as_ref()).ok_or(SheetError::AbsoluteToRelativePath)?,
            )),
        }
    }

    pub fn with_absolute_paths<T: AsRef<Path>>(&self, relative_to: T) -> ExportFormat {
        match self {
            ExportFormat::Template(p) => ExportFormat::Template(relative_to.as_ref().join(&p)),
        }
    }
}

impl ExportSettings {
    pub fn new() -> ExportSettings {
        ExportSettings {
            format: ExportFormat::Template(PathBuf::new()),
            texture_destination: PathBuf::new(),
            metadata_destination: PathBuf::new(),
            metadata_paths_root: PathBuf::new(),
        }
    }

    pub fn with_relative_paths<T: AsRef<Path>>(
        &self,
        relative_to: T,
    ) -> Result<ExportSettings, SheetError> {
        Ok(ExportSettings {
            format: self.format.with_relative_paths(&relative_to)?,
            texture_destination: diff_paths(&self.texture_destination, relative_to.as_ref())
                .ok_or(SheetError::AbsoluteToRelativePath)?,
            metadata_destination: diff_paths(&self.metadata_destination, relative_to.as_ref())
                .ok_or(SheetError::AbsoluteToRelativePath)?,
            metadata_paths_root: diff_paths(&self.metadata_paths_root, relative_to.as_ref())
                .ok_or(SheetError::AbsoluteToRelativePath)?,
        })
    }

    pub fn with_absolute_paths<T: AsRef<Path>>(&self, relative_to: T) -> ExportSettings {
        ExportSettings {
            format: self.format.with_absolute_paths(&relative_to),
            texture_destination: relative_to.as_ref().join(&self.texture_destination),
            metadata_destination: relative_to.as_ref().join(&self.metadata_destination),
            metadata_paths_root: relative_to.as_ref().join(&self.metadata_paths_root),
        }
    }
}
