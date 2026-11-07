//! Content identity shared by image, HEIC and video preview caches.
use std::path::{Path, PathBuf};

pub(crate) struct PreviewSource {
    pub(crate) path: PathBuf,
    pub(crate) version: String,
}
impl PreviewSource {
    pub(crate) fn capture(path: &Path, expected: Option<&str>) -> Result<Self, String> {
        let version = crate::source_version::read(path)?;
        if expected.is_some_and(|expected| expected != version) {
            return Err("Source changed; Sync the library before previewing this file".into());
        }
        Ok(Self {
            path: path.to_owned(),
            version,
        })
    }
    pub(crate) fn verify(&self) -> Result<(), String> {
        if crate::source_version::read(&self.path)? != self.version {
            return Err("Source changed while its preview was generated; Sync and retry".into());
        }
        Ok(())
    }
}
