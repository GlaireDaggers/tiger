use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use crate::sheet::version2 as previous_version;
use crate::sheet::Version;

const THIS_VERSION: Version = Version::Tiger3;

#[derive(Serialize, Deserialize)]
struct VersionedSheet {
    sheet: Sheet,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Sheet {
    pub(in crate::sheet) frames: Vec<Frame>,
    pub(in crate::sheet) animations: BTreeMap<String, Animation>,
    pub(in crate::sheet) export_settings: Option<ExportSettings>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub(in crate::sheet) source: PathBuf,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Animation {
    pub(in crate::sheet) timeline: Vec<Keyframe>,
    pub(in crate::sheet) is_looping: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    pub(in crate::sheet) frame: PathBuf,
    pub(in crate::sheet) hitboxes: BTreeMap<String, Hitbox>,
    pub(in crate::sheet) duration_millis: u32,
    pub(in crate::sheet) offset: (i32, i32),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Hitbox {
    pub(in crate::sheet) geometry: Shape,
    pub(in crate::sheet) linked: bool,
    pub(in crate::sheet) locked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    Rectangle(Rectangle),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ExportSettings {
    pub(in crate::sheet) format: ExportFormat,
    pub(in crate::sheet) texture_destination: PathBuf,
    pub(in crate::sheet) metadata_destination: PathBuf,
    pub(in crate::sheet) metadata_paths_root: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ExportFormat {
    Template(PathBuf),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Rectangle {
    pub(in crate::sheet) top_left: (i32, i32),
    pub(in crate::sheet) size: (u32, u32),
}

pub(super) fn read_file<T: AsRef<Path>>(version: Version, path: T) -> anyhow::Result<Sheet> {
    match version {
        THIS_VERSION => {
            let deserialized: VersionedSheet =
                serde_json::from_reader(BufReader::new(File::open(path.as_ref())?))?;
            Ok(deserialized.sheet)
        }
        _ => Ok(previous_version::read_file(version, path)?.into()),
    }
}

impl From<previous_version::Sheet> for Sheet {
    fn from(old: previous_version::Sheet) -> Sheet {
        let mut new_animations: BTreeMap<String, Animation> = old
            .animations
            .into_iter()
            .map(|o| (o.name.clone(), o.into()))
            .collect();

        // Migrate hitbox data from frames to keyframes
        for frame in &old.frames {
            for hitbox in &frame.hitboxes {
                for (_name, animation) in &mut new_animations {
                    for keyframe in &mut animation.timeline {
                        if keyframe.frame == frame.source {
                            let mut new_hitbox: Hitbox = hitbox.clone().into();
                            let Shape::Rectangle(r) = &mut new_hitbox.geometry;
                            r.top_left.0 += keyframe.offset.0;
                            r.top_left.1 += keyframe.offset.1;
                            keyframe.hitboxes.insert(hitbox.name.clone(), new_hitbox);
                        }
                    }
                }
            }
        }
        let new_frames = old.frames.into_iter().map(|o| o.into()).collect();
        Sheet {
            frames: new_frames,
            animations: new_animations,
            export_settings: old.export_settings.map(|o| o.into()),
        }
    }
}

impl From<previous_version::Animation> for Animation {
    fn from(old: previous_version::Animation) -> Animation {
        Animation {
            timeline: old.timeline.into_iter().map(|o| o.into()).collect(),
            is_looping: old.is_looping,
        }
    }
}

impl From<previous_version::Frame> for Frame {
    fn from(old: previous_version::Frame) -> Frame {
        Frame { source: old.source }
    }
}

impl From<previous_version::Keyframe> for Keyframe {
    fn from(old: previous_version::Keyframe) -> Keyframe {
        Keyframe {
            frame: old.frame,
            duration_millis: old.duration,
            offset: old.offset,
            hitboxes: BTreeMap::new(),
        }
    }
}

impl From<previous_version::Hitbox> for Hitbox {
    fn from(old: previous_version::Hitbox) -> Hitbox {
        Hitbox {
            geometry: old.geometry.into(),
            linked: true,
            locked: false,
        }
    }
}

impl From<previous_version::Shape> for Shape {
    fn from(old: previous_version::Shape) -> Shape {
        match old {
            previous_version::Shape::Rectangle(r) => Shape::Rectangle(r.into()),
        }
    }
}

impl From<previous_version::Rectangle> for Rectangle {
    fn from(old: previous_version::Rectangle) -> Rectangle {
        Rectangle {
            top_left: old.top_left,
            size: old.size,
        }
    }
}

impl From<previous_version::ExportFormat> for ExportFormat {
    fn from(old: previous_version::ExportFormat) -> ExportFormat {
        match old {
            previous_version::ExportFormat::Template(p) => ExportFormat::Template(p),
        }
    }
}

impl From<previous_version::ExportSettings> for ExportSettings {
    fn from(old: previous_version::ExportSettings) -> ExportSettings {
        ExportSettings {
            format: old.format.into(),
            texture_destination: old.texture_destination,
            metadata_destination: old.metadata_destination.clone(),
            metadata_paths_root: old.metadata_destination,
        }
    }
}
