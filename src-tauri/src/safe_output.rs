//! Stage outputs beside their destination, publishing only complete files without overwrite.
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) struct StagedOutput {
    temporary: PathBuf,
    destination: PathBuf,
    file: Option<File>,
}

impl StagedOutput {
    pub fn new(destination: &Path) -> Result<Self, String> {
        let parent = fs::canonicalize(
            destination
                .parent()
                .ok_or("Choose an existing output folder")?,
        )
        .map_err(|e| e.to_string())?;
        let destination = parent.join(destination.file_name().ok_or("Choose an output filename")?);
        if destination.exists() {
            return Err("That output already exists; choose a new filename".into());
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(
            ".openphoto-{}-{stamp}-{sequence}.tmp",
            std::process::id()
        ));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            temporary,
            destination,
            file: Some(file),
        })
    }

    pub fn file(&mut self) -> &mut File {
        self.file.as_mut().expect("output is open until commit")
    }

    pub fn commit(mut self) -> Result<PathBuf, String> {
        self.file().sync_all().map_err(|e| e.to_string())?;
        self.file.take();
        publish(&self.temporary, &self.destination).map_err(|e| e.to_string())?;
        Ok(self.destination.clone())
    }
}

impl Drop for StagedOutput {
    fn drop(&mut self) {
        self.file.take();
        let _ = fs::remove_file(&self.temporary);
    }
}

#[cfg(windows)]
fn publish(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // SAFETY: both buffers are NUL-terminated and live through the call. No replace flag is used.
    let result = unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn publish(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::hard_link(source, destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn cancelled_or_failed_outputs_never_publish_and_racing_destinations_survive() {
        let directory = std::env::temp_dir().join(format!(
            "openphoto-output-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        let destination = directory.join("export.png");
        {
            let mut output = StagedOutput::new(&destination).unwrap();
            output.file().write_all(b"partial").unwrap();
        }
        assert!(!destination.exists());
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
        let mut output = StagedOutput::new(&destination).unwrap();
        output.file().write_all(b"complete").unwrap();
        fs::write(&destination, b"other app").unwrap();
        assert!(output.commit().is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"other app");
        fs::remove_file(&destination).unwrap();
        let mut output = StagedOutput::new(&destination).unwrap();
        output.file().write_all(b"complete").unwrap();
        output.commit().unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"complete");
        fs::remove_dir_all(directory).unwrap();
    }
}
