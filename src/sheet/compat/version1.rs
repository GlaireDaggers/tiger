use anyhow::bail;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use crate::sheet::compat::Version;

const THIS_VERSION: Version = Version::Tiger1;

#[derive(Serialize, Deserialize)]
pub struct VersionedSheet {
    pub sheet: Sheet,
}

pub fn read_file<T: AsRef<Path>>(version: Version, path: T) -> anyhow::Result<Sheet> {
    assert!(version == THIS_VERSION);
    match version {
        THIS_VERSION => {
            let deserialized: VersionedSheet =
                serde_json::from_reader(BufReader::new(File::open(path.as_ref())?))?;
            Ok(deserialized.sheet)
        }
        _ => bail!("Unexpected version"),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sheet {
    pub frames: Vec<Frame>,
    pub animations: Vec<Animation>,
    pub export_settings: Option<ExportSettings>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Animation {
    pub name: String,
    pub timeline: Vec<Keyframe>,
    pub is_looping: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Frame {
    pub source: PathBuf,
    pub hitboxes: Vec<Hitbox>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Keyframe {
    pub frame: PathBuf,
    pub duration: u32, // in ms
    pub offset: (i32, i32),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hitbox {
    pub name: String,
    pub geometry: Shape,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Shape {
    Rectangle(Rectangle),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rectangle {
    pub top_left: (i32, i32),
    pub size: (u32, u32),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ExportFormat {
    Template(PathBuf),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportSettings {
    pub format: ExportFormat,
    pub texture_destination: PathBuf,
    pub metadata_destination: PathBuf,
}
