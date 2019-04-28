use failure::Error;
use std::path::Path;

use crate::sheet::*;

#[derive(Fail, Debug)]
pub enum DocumentError {
    #[fail(display = "Requested frame is not in document")]
    FrameNotInDocument,
    #[fail(display = "Requested animation is not in document")]
    AnimationNotInDocument,
    #[fail(display = "Requested hitbox is not in frame")]
    HitboxNotInFrame,
    #[fail(display = "A hitbox with this name already exists")]
    HitboxAlreadyExists,
    #[fail(display = "An animation with this name already exists")]
    AnimationAlreadyExists,
    #[fail(display = "Not currently editing any frame")]
    NotEditingAnyFrame,
    #[fail(display = "Not currently editing any animation")]
    NotEditingAnyAnimation,
    #[fail(display = "Currently not adjusting a hitbox")]
    NotDraggingAHitbox,
    #[fail(display = "Frame does not have a hitbox at the requested index")]
    InvalidHitboxIndex,
    #[fail(display = "Animation does not have a frame at the requested index")]
    InvalidAnimationFrameIndex,
    #[fail(display = "Currently not adjusting the duration of an animation frame")]
    NotDraggingATimelineFrame,
    #[fail(display = "No animation frame found for requested time")]
    NoAnimationFrameForThisTime,
    #[fail(display = "Not currently adjusting export settings")]
    NotExporting,
    #[fail(display = "Not currently renaming an item")]
    NotRenaming,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    sheet: Sheet,
    export_settings: Option<ExportSettings>,
}

impl Document {
    pub fn new() -> Document {
        Document {
            sheet: Sheet::new(),
            export_settings: None,
        }
    }

    pub fn open<T: AsRef<Path>>(path: T) -> Result<Document, Error> {
        let mut directory = path.as_ref().to_path_buf();
        directory.pop();
        let sheet: Sheet = compat::read_sheet(path.as_ref())?;
        let sheet = sheet.with_absolute_paths(&directory)?;
        let mut document = Document::new();
        document.sheet = sheet;
        Ok(document)
    }

    pub fn save<T: AsRef<Path>>(&self, to: T) -> Result<(), Error> {
        let mut directory = to.as_ref().to_path_buf();
        directory.pop();
        let sheet = self.get_sheet().with_relative_paths(directory)?;
        compat::write_sheet(to, &sheet)?;
        Ok(())
    }

    pub fn get_sheet(&self) -> &Sheet {
        &self.sheet
    }

    pub fn get_sheet_mut(&mut self) -> &mut Sheet {
        &mut self.sheet
    }

    pub fn import<T: AsRef<Path>>(&mut self, path: T) {
        self.sheet.add_frame(path);
    }

    pub fn get_export_settings(&self) -> &Option<ExportSettings> {
        &self.export_settings
    }

    pub fn begin_export_as(&mut self) {
        self.export_settings = self
            .get_sheet()
            .get_export_settings()
            .as_ref()
            .cloned()
            .or_else(|| Some(ExportSettings::new()));
    }

    pub fn cancel_export_as(&mut self) {
        self.export_settings = None;
    }

    pub fn end_set_export_texture_destination<T: AsRef<Path>>(
        &mut self,
        texture_destination: T,
    ) -> Result<(), Error> {
        let export_settings = &mut self
            .export_settings
            .as_mut()
            .ok_or(DocumentError::NotExporting)?;
        export_settings.texture_destination = texture_destination.as_ref().to_path_buf();
        Ok(())
    }

    pub fn end_set_export_metadata_destination<T: AsRef<Path>>(
        &mut self,
        metadata_destination: T,
    ) -> Result<(), Error> {
        let export_settings = &mut self
            .export_settings
            .as_mut()
            .ok_or(DocumentError::NotExporting)?;
        export_settings.metadata_destination = metadata_destination.as_ref().to_path_buf();
        Ok(())
    }

    pub fn end_set_export_metadata_paths_root<T: AsRef<Path>>(
        &mut self,
        metadata_paths_root: T,
    ) -> Result<(), Error> {
        let export_settings = &mut self
            .export_settings
            .as_mut()
            .ok_or(DocumentError::NotExporting)?;
        export_settings.metadata_paths_root = metadata_paths_root.as_ref().to_path_buf();
        Ok(())
    }

    pub fn end_set_export_format(&mut self, format: ExportFormat) -> Result<(), Error> {
        let export_settings = &mut self
            .export_settings
            .as_mut()
            .ok_or(DocumentError::NotExporting)?;
        export_settings.format = format;
        Ok(())
    }

    pub fn end_export_as(&mut self) -> Result<(), Error> {
        let export_settings = self
            .export_settings
            .take()
            .ok_or(DocumentError::NotExporting)?;
        self.get_sheet_mut()
            .set_export_settings(export_settings.clone());
        Ok(())
    }
}
