//! Advisory filename/metadata observations only. The caller must first retain
//! the authorized parent directory guard. Never read staged bytes, follow links,
//! establish ownership, choose a winner, or use this inventory to authorize saves.
use serde::Serialize;
use std::{fs, path::Path};

#[derive(Serialize, Debug)]
pub(crate) struct StagingEntry {
    name: String,
    kind: String,
    bytes: Option<u64>,
    error: Option<String>,
}
#[derive(Serialize, Debug, Default)]
pub(crate) struct StagingInventory {
    entries: Vec<StagingEntry>,
    truncated: bool,
    error: Option<String>,
}
fn matches_operation(name: &str, id: i64) -> bool {
    let Some(body) = name
        .strip_prefix(".openphoto-xmp-")
        .and_then(|s| s.strip_suffix(".tmp"))
    else {
        return false;
    };
    let mut fields = body.split('-');
    let (Some(pid), Some(operation), Some(stamp)) = (fields.next(), fields.next(), fields.next())
    else {
        return false;
    };
    id > 0
        && fields.next().is_none()
        && operation == id.to_string()
        && !pid.is_empty()
        && pid.bytes().all(|b| b.is_ascii_digit())
        && pid.parse::<u32>().is_ok_and(|pid| pid > 0)
        && !stamp.is_empty()
        && stamp.bytes().all(|b| b.is_ascii_digit())
        && stamp.parse::<u128>().is_ok()
}
fn kind(metadata: &fs::Metadata) -> &'static str {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return "link";
        }
    }
    if metadata.file_type().is_symlink() {
        "link"
    } else if metadata.is_file() {
        "file"
    } else if metadata.is_dir() {
        "directory"
    } else {
        "other"
    }
}
pub(crate) fn inspect(sidecar: &Path, id: i64) -> StagingInventory {
    inspect_limited(sidecar, id, 4096, 20)
}
fn inspect_limited(
    sidecar: &Path,
    id: i64,
    scan_limit: usize,
    match_limit: usize,
) -> StagingInventory {
    let mut out = StagingInventory::default();
    let result = (|| -> Result<(), String> {
        if id <= 0 {
            return Err("Invalid operation; staging filenames were not inspected".into());
        }
        let parent = sidecar.parent().ok_or("Missing media folder")?;
        let entries = fs::read_dir(parent)
            .map_err(|e| format!("Could not list possible staging files: {e}"))?;
        for (scanned, entry) in entries.enumerate() {
            if scanned >= scan_limit {
                out.truncated = true;
                break;
            }
            let entry = entry
                .map_err(|e| format!("Could not finish listing possible staging files: {e}"))?;
            let name = entry.file_name();
            let Some(name) = name.to_str().filter(|name| matches_operation(name, id)) else {
                continue;
            };
            if out.entries.len() >= match_limit {
                out.truncated = true;
                break;
            }
            // Metadata only. This neither opens the staged data nor takes file
            // sharing locks that could interfere with a publisher/editor.
            let observed = match fs::symlink_metadata(entry.path()) {
                Ok(metadata) => {
                    let kind = kind(&metadata);
                    StagingEntry {
                        name: name.into(),
                        kind: kind.into(),
                        bytes: (kind == "file").then_some(metadata.len()),
                        error: None,
                    }
                }
                Err(error) => StagingEntry {
                    name: name.into(),
                    kind: "unavailable".into(),
                    bytes: None,
                    error: Some(error.to_string()),
                },
            };
            out.entries.push(observed);
        }
        Ok(())
    })();
    if let Err(error) = result {
        out.error = Some(error);
        out.truncated = true;
    }
    out.entries.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            loop {
                let path = std::env::temp_dir().join(format!(
                    "openphoto-stage-inventory-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(e) => panic!("Cannot create disposable fixture: {e}"),
                }
            }
        }
        fn sidecar(&self) -> PathBuf {
            self.0.join("photo.jpg.xmp")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn strict_names_scope_observations_without_claiming_file_ownership() {
        let f = Fixture::new();
        let names = [
            ".openphoto-xmp-123-7-456.tmp",
            ".openphoto-xmp-124-7-457.tmp",
            ".openphoto-xmp-123-70-456.tmp",
            ".openphoto-xmp-original-7-123.bak",
            ".openphoto-xmp-x-7-456.tmp",
            ".openphoto-xmp-123-7-456-extra.tmp",
            ".openphoto-xmp-0-7-456.tmp",
            ".openphoto-xmp-123-7-x.tmp",
        ];
        for name in names {
            fs::write(f.0.join(name), b"untrusted-stage-bytes").unwrap();
        }
        let result = inspect(&f.sidecar(), 7);
        assert_eq!(result.entries.len(), 2);
        assert!(!result.truncated);
        assert!(result.error.is_none());
        assert!(result
            .entries
            .iter()
            .all(|e| e.kind == "file" && e.bytes == Some(21)));
        for name in names {
            assert_eq!(fs::read(f.0.join(name)).unwrap(), b"untrusted-stage-bytes");
        }
    }
    #[test]
    fn directory_entries_are_not_traversed_or_treated_as_files() {
        let f = Fixture::new();
        let directory = f.0.join(".openphoto-xmp-123-7-456.tmp");
        fs::create_dir(&directory).unwrap();
        fs::write(
            directory.join(".openphoto-xmp-124-7-457.tmp"),
            b"nested-sentinel",
        )
        .unwrap();
        let result = inspect(&f.sidecar(), 7);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].kind, "directory");
        assert!(result.entries[0].bytes.is_none());
        assert_eq!(
            fs::read(directory.join(".openphoto-xmp-124-7-457.tmp")).unwrap(),
            b"nested-sentinel"
        );
    }
    #[test]
    fn scan_and_match_limits_are_explicitly_incomplete_and_never_cleanup() {
        let f = Fixture::new();
        for n in 0..12 {
            fs::write(f.0.join(format!(".openphoto-xmp-123-7-{n}.tmp")), b"stage").unwrap();
        }
        let scanned = inspect_limited(&f.sidecar(), 7, 3, 20);
        assert!(scanned.truncated);
        assert_eq!(scanned.entries.len(), 3);
        let matched = inspect_limited(&f.sidecar(), 7, 100, 2);
        assert!(matched.truncated);
        assert_eq!(matched.entries.len(), 2);
        assert_eq!(fs::read_dir(&f.0).unwrap().count(), 12);
    }
    #[test]
    fn missing_folders_and_invalid_operations_are_not_reported_as_clear() {
        let f = Fixture::new();
        let missing = inspect(&f.0.join("missing/photo.jpg.xmp"), 7);
        assert!(missing.error.is_some() && missing.truncated);
        let invalid = inspect(&f.sidecar(), -1);
        assert!(invalid.error.is_some() && invalid.truncated);
    }
    #[cfg(windows)]
    #[test]
    fn junction_entries_are_observed_without_following_or_changing_the_target() {
        let f = Fixture::new();
        let target = f.0.join("outside-target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("sentinel"), b"outside-original").unwrap();
        let link = f.0.join(".openphoto-xmp-123-7-456.tmp");
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&target)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let result = inspect(&f.sidecar(), 7);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].kind, "link");
        assert!(result.entries[0].bytes.is_none());
        assert_eq!(
            fs::read(target.join("sentinel")).unwrap(),
            b"outside-original"
        );
        fs::remove_dir(link).unwrap();
    }
}
