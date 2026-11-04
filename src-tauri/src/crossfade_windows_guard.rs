//! Test-only Windows file-handle protection for normalized media during FFmpeg.
//! Read sharing explicitly excludes concurrent writers and delete/rename
//! requests until the guard is dropped.
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
};

pub(crate) struct PinnedMedia {
    handles: Vec<File>,
}

impl PinnedMedia {
    /// Pin each expected regular file before a native render, checking bytes
    /// through the very handles retained until publication completes.
    pub(crate) fn acquire(
        expected: &[(&Path, u64, &str)],
        cancel: impl Fn() -> bool,
    ) -> Result<Self, String> {
        if expected.is_empty() || expected.len() > 200 {
            return Err("Crossfade pin requires 1–200 source files.".into());
        }
        let mut seen = HashSet::<PathBuf>::new();
        let mut handles = Vec::with_capacity(expected.len());
        for (path, bytes, hash) in expected {
            if cancel() {
                return Err("Crossfade file pinning cancelled.".into());
            }
            let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
            if !path.is_absolute()
                || !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
                || *bytes == 0
                || hash.len() != 64
                || !hash.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err("Crossfade pin requires a regular original file and SHA-256.".into());
            }
            let canonical = fs::canonicalize(path).map_err(|e| e.to_string())?;
            if !seen.insert(canonical) {
                return Err("Crossfade pin cannot accept duplicate files.".into());
            }
            let mut handle = OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ)
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
                .open(path)
                .map_err(|e| format!("Cannot pin crossfade input for read-only access: {e}"))?;
            if handle.metadata().map_err(|e| e.to_string())?.file_attributes()
                & FILE_ATTRIBUTE_REPARSE_POINT
                != 0
            {
                return Err("Crossfade pinned input resolves to a reparse point.".into());
            }
            let mut digest = Sha256::new();
            let mut buffer = [0_u8; 64 * 1024];
            let mut count = 0_u64;
            loop {
                if cancel() {
                    return Err("Crossfade file pinning cancelled.".into());
                }
                let n = handle.read(&mut buffer).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                count = count
                    .checked_add(n as u64)
                    .ok_or("Crossfade input exceeds byte limit.")?;
                if count > *bytes {
                    return Err("Crossfade pinned input size changed.".into());
                }
                digest.update(&buffer[..n]);
            }
            if count != *bytes || format!("{:x}", digest.finalize()) != *hash {
                return Err("Crossfade pinned input differs from its saved fingerprint.".into());
            }
            handle.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
            handles.push(handle);
        }
        Ok(Self { handles })
    }

    pub(crate) fn count(&self) -> usize {
        self.handles.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_read_handles_reject_writes_and_deletes_until_release() {
        let dir = std::env::temp_dir().join(format!(
            "openphoto-crossfade-windows-pin-{}",
            crate::video_project::id()
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("normalized.mkv");
        fs::write(&path, b"original normalized video").unwrap();
        let (bytes, hash) = crate::video_project::fingerprint(&path, || false).unwrap();
        let guard = PinnedMedia::acquire(&[(&path, bytes, &hash)], || false).unwrap();
        assert_eq!(guard.count(), 1);
        assert!(OpenOptions::new().write(true).open(&path).is_err());
        assert!(fs::remove_file(&path).is_err());
        drop(guard);
        fs::write(&path, b"replacement").unwrap();
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_changed_contents_and_duplicate_paths() {
        let dir = std::env::temp_dir().join(format!(
            "openphoto-crossfade-windows-pin-reject-{}",
            crate::video_project::id()
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("original.mov");
        fs::write(&path, b"saved source").unwrap();
        let (bytes, hash) = crate::video_project::fingerprint(&path, || false).unwrap();
        assert!(PinnedMedia::acquire(&[(&path, bytes, &hash)], || true).is_err());
        assert!(
            PinnedMedia::acquire(&[(&path, bytes, &hash), (&path, bytes, &hash)], || false)
                .is_err()
        );
        fs::write(&path, b"changed data").unwrap();
        assert!(PinnedMedia::acquire(&[(&path, bytes, &hash)], || false).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
