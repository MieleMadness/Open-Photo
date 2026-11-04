//! Test-only original-media integrity gate for experimental crossfade rendering.
//! Expectations must come from a previously validated project snapshot, not
//! from fingerprinting a source after the render has already begun.
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

pub(crate) struct SourceExpectation<'a> {
    pub path: &'a Path,
    pub bytes: u64,
    pub sha256: &'a str,
}

/// Fail closed if any source has moved, changed bytes or no longer matches
/// its previously stored SHA-256. Re-run before and after all native encoding.
pub(crate) fn verify_source_fingerprints(
    sources: &[SourceExpectation<'_>],
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    if sources.is_empty() || sources.len() > 200 {
        return Err("Crossfade needs 1–200 verified source files.".into());
    }
    let mut seen = HashSet::<PathBuf>::new();
    for source in sources {
        if cancel() {
            return Err("Crossfade source verification cancelled.".into());
        }
        if !source.path.is_absolute()
            || source.bytes == 0
            || source.sha256.len() != 64
            || !source.sha256.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("Crossfade source identity is incomplete.".into());
        }
        let canonical = std::fs::canonicalize(source.path).map_err(|e| e.to_string())?;
        if !canonical.is_file() || !seen.insert(canonical) {
            return Err("Crossfade sources must be distinct regular files.".into());
        }
        let (bytes, hash) = crate::video_project::fingerprint(source.path, &cancel)?;
        if bytes != source.bytes || hash != source.sha256 {
            return Err("Original crossfade source changed; relink it before rendering.".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detects_source_mutation_even_when_size_does_not_change() {
        let path = std::env::temp_dir().join(format!(
            "openphoto-crossfade-integrity-{}.mov",
            crate::video_project::id()
        ));
        fs::write(&path, b"original bytes").unwrap();
        let path = fs::canonicalize(&path).unwrap();
        let (bytes, hash) = crate::video_project::fingerprint(&path, || false).unwrap();
        let sources = [SourceExpectation {
            path: &path,
            bytes,
            sha256: &hash,
        }];
        assert!(verify_source_fingerprints(&sources, || false).is_ok());
        fs::write(&path, b"modified bytes").unwrap();
        assert!(verify_source_fingerprints(&sources, || false).is_err());
        assert!(verify_source_fingerprints(&sources, || true).is_err());
        fs::remove_file(&path).unwrap();
        assert!(verify_source_fingerprints(&sources, || false).is_err());
    }

    #[test]
    fn rejects_missing_and_duplicate_source_expectations() {
        let path = std::env::temp_dir().join(format!(
            "openphoto-crossfade-duplicate-{}.mov",
            crate::video_project::id()
        ));
        fs::write(&path, b"valid test source").unwrap();
        let path = fs::canonicalize(&path).unwrap();
        let (bytes, hash) = crate::video_project::fingerprint(&path, || false).unwrap();
        let source = SourceExpectation {
            path: &path,
            bytes,
            sha256: &hash,
        };
        assert!(verify_source_fingerprints(&[], || false).is_err());
        assert!(verify_source_fingerprints(&[source, source], || false).is_err());
        fs::remove_file(path).unwrap();
    }
}
