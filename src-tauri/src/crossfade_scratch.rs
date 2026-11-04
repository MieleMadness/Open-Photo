//! Test-only filesystem ownership gate for future crossfade command execution.
//! Not connected to the production renderer or user-facing exports.
use std::{
    collections::HashSet,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

/// Require all normalized parts and the prospective output to be direct
/// children of a real, non-symlink temporary directory. This must be combined
/// with source fingerprints and stream validation before production use.
pub(crate) fn validate_scratch_paths(
    scratch: &Path,
    parts: &[PathBuf],
    output: &Path,
) -> Result<(), String> {
    if !scratch.is_absolute() || parts.len() < 2 || parts.len() > 200 {
        return Err("Crossfade scratch requires 2–200 normalized inputs.".into());
    }
    let metadata = fs::symlink_metadata(scratch).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Crossfade scratch must be a real directory.".into());
    }
    #[cfg(windows)]
    if metadata.file_attributes()
        & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
        != 0
    {
        return Err("Crossfade scratch directory cannot be a Windows reparse point.".into());
    }
    let canonical = fs::canonicalize(scratch).map_err(|e| e.to_string())?;
    if canonical != scratch {
        return Err("Crossfade scratch must use its canonical directory path.".into());
    }
    let mut unique = HashSet::new();
    for part in parts {
        if !part.is_absolute()
            || part.parent() != Some(scratch)
            || part.extension().and_then(|x| x.to_str()) != Some("mkv")
            || !unique.insert(part)
        {
            return Err("Crossfade inputs must be distinct scratch MKV files.".into());
        }
        let info = fs::symlink_metadata(part).map_err(|e| e.to_string())?;
        if !info.is_file() || info.file_type().is_symlink() {
            return Err("Crossfade input must be a regular normalized file.".into());
        }
        #[cfg(windows)]
        if info.file_attributes()
            & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
            != 0
        {
            return Err("Crossfade input cannot be a Windows reparse point.".into());
        }
        if fs::canonicalize(part).map_err(|e| e.to_string())?.parent() != Some(canonical.as_path())
        {
            return Err("Crossfade input escaped scratch ownership.".into());
        }
    }
    if !output.is_absolute()
        || output.parent() != Some(scratch)
        || output.extension().and_then(|x| x.to_str()) != Some("mp4")
    {
        return Err("Crossfade output must be a new MP4 within owned scratch.".into());
    }
    match fs::symlink_metadata(output) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Ok(_) => Err("Crossfade temporary output already exists.".into()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn only_accepts_distinct_existing_parts_in_owned_scratch() {
        let scratch = std::env::temp_dir().join(format!(
            "openphoto-crossfade-owned-{}",
            crate::video_project::id()
        ));
        fs::create_dir(&scratch).unwrap();
        let scratch = fs::canonicalize(&scratch).unwrap();
        let _fixture = Fixture(scratch.clone());
        let a = scratch.join("a.mkv");
        let b = scratch.join("b.mkv");
        fs::write(&a, b"first").unwrap();
        fs::write(&b, b"second").unwrap();
        let output = scratch.join("render.mp4");
        let parts = [a.clone(), b.clone()];
        assert!(validate_scratch_paths(&scratch, &parts, &output).is_ok());
        assert!(validate_scratch_paths(&scratch, &[a.clone(), a.clone()], &output).is_err());
        assert!(validate_scratch_paths(&scratch, &parts[..1], &output).is_err());
        assert!(validate_scratch_paths(&scratch, &parts, &scratch.join("render.mkv")).is_err());

        fs::write(&output, b"existing").unwrap();
        assert!(validate_scratch_paths(&scratch, &parts, &output).is_err());
        fs::remove_file(&output).unwrap();

        let outside = std::env::temp_dir().join(format!(
            "openphoto-crossfade-outside-{}.mkv",
            crate::video_project::id()
        ));
        fs::write(&outside, b"outside").unwrap();
        assert!(validate_scratch_paths(&scratch, &[a, outside.clone()], &output).is_err());
        fs::remove_file(outside).unwrap();
    }
}
