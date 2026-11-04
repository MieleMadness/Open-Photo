//! Test-only Windows file-handle protection for normalized media during FFmpeg.
//! Read sharing explicitly excludes concurrent writers and delete/rename
//! requests until the guard is dropped.
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    },
    path::{Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::{
    GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
};

/// Compare stable on-disk Win32 file identities through owned handles instead
/// of relying on path-based metadata after the verified file has been opened.
fn file_identity(file: &File) -> Result<(u32, u64, u32, u32), String> {
    let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: file is an owned, live Windows handle; a successful call writes
    // every field of BY_HANDLE_FILE_INFORMATION.
    let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) };
    if ok == 0 {
        return Err(format!(
            "Crossfade cannot inspect Windows file identity: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: the native API returned success and initialized the struct.
    let info = unsafe { info.assume_init() };
    let index = (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow);
    if index == 0 || info.dwVolumeSerialNumber == 0 {
        return Err("Crossfade file identity is unavailable on this filesystem.".into());
    }
    Ok((
        info.dwVolumeSerialNumber,
        index,
        info.nNumberOfLinks,
        info.dwFileAttributes,
    ))
}

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
        let mut identities = HashSet::<(u32, u64)>::new();
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
            if handle
                .metadata()
                .map_err(|e| e.to_string())?
                .file_attributes()
                & FILE_ATTRIBUTE_REPARSE_POINT
                != 0
            {
                return Err("Crossfade pinned input resolves to a reparse point.".into());
            }
            let (volume, index, links, attrs) = file_identity(&handle)?;
            if links != 1 || attrs & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                return Err("Crossfade input is hard-linked or is a reparse point.".into());
            }
            // Reopening with read-only sharing while the first handle is held
            // verifies the path still names the exact same underlying file.
            let verify_path = || -> Result<(), String> {
                let second = OpenOptions::new()
                    .read(true)
                    .share_mode(FILE_SHARE_READ)
                    .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
                    .open(path)
                    .map_err(|e| e.to_string())?;
                let (v, id, nlinks, flags) = file_identity(&second)?;
                if (v, id) != (volume, index)
                    || nlinks != 1
                    || flags & FILE_ATTRIBUTE_REPARSE_POINT != 0
                {
                    return Err("Crossfade pathname no longer references the verified file.".into());
                }
                Ok(())
            };
            verify_path()?;
            if !identities.insert((volume, index)) {
                return Err("Crossfade source paths alias one physical file.".into());
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
            verify_path()?;
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
    fn rejects_reparse_point_file_even_when_target_matches_expected_hash() {
        let dir = std::env::temp_dir().join(format!(
            "openphoto-crossfade-windows-reparse-{}",
            crate::video_project::id()
        ));
        fs::create_dir(&dir).unwrap();
        let target = dir.join("trusted.mkv");
        let alias = dir.join("alias.mkv");
        fs::write(&target, b"trusted normalized footage").unwrap();
        let (bytes, hash) = crate::video_project::fingerprint(&target, || false).unwrap();
        if std::os::windows::fs::symlink_file(&target, &alias).is_ok() {
            assert!(PinnedMedia::acquire(&[(&alias, bytes, &hash)], || false).is_err());
        }
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
