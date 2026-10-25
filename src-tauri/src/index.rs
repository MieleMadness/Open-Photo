use rusqlite::{params, Connection};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use crate::{scan_library, ScanResult};

fn database() -> Result<Connection, String> {
    let directory = dirs::data_local_dir()
        .ok_or("Windows local data directory unavailable")?
        .join("Open-Photo");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let connection =
        Connection::open(directory.join("media-index.sqlite")).map_err(|e| e.to_string())?;
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS media (
                root TEXT NOT NULL,
                path TEXT NOT NULL,
                bytes INTEGER NOT NULL,
                modified_unix INTEGER NOT NULL,
                PRIMARY KEY(root, path)
            );
            CREATE INDEX IF NOT EXISTS idx_media_size ON media(bytes);",
        )
        .map_err(|e| e.to_string())?;
    Ok(connection)
}

/// Refresh the local SQLite index from a read-only filesystem scan.
/// This never removes or edits source files.
#[tauri::command]
pub fn scan_indexed(root: String) -> Result<ScanResult, String> {
    Ok(scan_indexed_with_changes(root)?.scan)
}

#[derive(Serialize)]
pub struct ChangeCounts {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub removed: usize,
    pub incomplete: bool,
}

#[derive(Serialize)]
pub struct IndexedScan {
    pub scan: ScanResult,
    pub changes: ChangeCounts,
}

/// Report index changes while preserving all source files.
#[tauri::command]
pub fn scan_indexed_with_changes(root: String) -> Result<IndexedScan, String> {
    let scan = scan_library(root.clone())?;
    let mut changes = ChangeCounts {
        added: 0,
        updated: 0,
        unchanged: 0,
        removed: 0,
        incomplete: scan.truncated,
    };
    // A truncated scan is not a complete snapshot: do not evict previously
    // indexed media based on missing entries.
    if scan.truncated {
        return Ok(IndexedScan { scan, changes });
    }
    let mut db = database()?;
    let transaction = db.transaction().map_err(|e| e.to_string())?;
    // Track the complete snapshot while preserving unchanged indexed rows.
    // This avoids tearing down and rebuilding the index on every sync.
    transaction
        .execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS seen_paths (path TEXT PRIMARY KEY);
             DELETE FROM seen_paths;",
        )
        .map_err(|e| e.to_string())?;
    // Fetch the prior snapshot once rather than executing one SELECT per media file.
    // This reduces SQLite round-trips significantly for large libraries.
    let previous_entries: HashMap<String, (i64, i64)> = {
        let mut statement = transaction
            .prepare("SELECT path, bytes, modified_unix FROM media WHERE root = ?1")
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([&root], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    (row.get::<_, i64>(1)?, row.get::<_, i64>(2)?),
                ))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };
    {
        let mut seen = transaction
            .prepare("INSERT OR IGNORE INTO seen_paths(path) VALUES (?1)")
            .map_err(|e| e.to_string())?;
        let mut upsert = transaction
            .prepare(
                "INSERT INTO media(root, path, bytes, modified_unix)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(root, path) DO UPDATE SET
                     bytes = excluded.bytes,
                     modified_unix = excluded.modified_unix
                 WHERE media.bytes != excluded.bytes
                    OR media.modified_unix != excluded.modified_unix",
            )
            .map_err(|e| e.to_string())?;
        for photo in &scan.photos {
            match previous_entries.get(&photo.path) {
                Some((bytes, modified)) => {
                    if *bytes != photo.bytes as i64 || *modified != photo.modified_unix as i64 {
                        changes.updated += 1;
                    } else {
                        changes.unchanged += 1;
                    }
                }
                None => changes.added += 1,
            }
            seen.execute([&photo.path]).map_err(|e| e.to_string())?;
            upsert
                .execute(params![
                    root,
                    photo.path,
                    photo.bytes as i64,
                    photo.modified_unix as i64
                ])
                .map_err(|e| e.to_string())?;
        }
    }
    changes.removed = transaction
        .execute(
            "DELETE FROM media WHERE root = ?1
             AND NOT EXISTS (SELECT 1 FROM seen_paths WHERE seen_paths.path = media.path)",
            [&root],
        )
        .map_err(|e| e.to_string())?;
    transaction.commit().map_err(|e| e.to_string())?;
    Ok(IndexedScan { scan, changes })
}

/// Apply a single confirmed file event without traversing the entire library.
/// Directory, ambiguous and missed events continue to use full scan fallback.
#[derive(Serialize)]
pub struct IncrementalChange {
    pub path: String,
    pub present: bool,
    pub bytes: u64,
    pub modified_unix: u64,
}

#[tauri::command]
pub fn update_indexed_path(root: String, path: String) -> Result<IncrementalChange, String> {
    let base = std::fs::canonicalize(&root).map_err(|e| e.to_string())?;
    if !base.is_dir() {
        return Err("Library root is not a folder".into());
    }
    let candidate = PathBuf::from(&path);
    if !candidate.is_absolute() || !candidate.starts_with(&base) {
        return Err("Changed file is outside the selected library".into());
    }
    // Missing files may be removed from the index, but an inaccessible file
    // must never be mistaken for deletion (e.g. disconnected network drive).
    // For deleted files, verify the containing folder too: a missing mount,
    // symlink escape, or removed directory is not proof of an individual deletion.
    // A full scan can reconcile directory changes separately.
    if !candidate.exists() {
        let parent = candidate
            .parent()
            .ok_or("Changed file has no parent folder")?;
        let resolved_parent = std::fs::canonicalize(parent)
            .map_err(|e| format!("Cannot verify parent folder; full rescan required: {e}"))?;
        if !resolved_parent.starts_with(&base) || !resolved_parent.is_dir() {
            return Err("Changed file parent is outside library or unavailable".into());
        }
    }
    let actual = match std::fs::canonicalize(&candidate) {
        Ok(resolved) => Some(resolved),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("Cannot inspect changed file: {error}")),
    };
    if actual
        .as_ref()
        .is_some_and(|resolved| !resolved.starts_with(&base))
    {
        return Err("Changed file resolves outside the library".into());
    }
    let metadata = match actual.as_ref() {
        Some(resolved) => Some(std::fs::metadata(resolved).map_err(|e| e.to_string())?),
        None => None,
    };
    if metadata.as_ref().is_some_and(|meta| meta.is_dir()) {
        return Err("Directory change requires full scan".into());
    }
    let extension = candidate
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !crate::SUPPORTED.contains(&extension.as_str()) {
        return Err("Unsupported media extension".into());
    }
    let mut change = IncrementalChange {
        path: path.clone(),
        present: false,
        bytes: 0,
        modified_unix: 0,
    };
    let db = database()?;
    if let Some(meta) = metadata.filter(|meta| meta.is_file()) {
        let modified = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |duration| duration.as_secs());
        db.execute(
            "INSERT INTO media(root,path,bytes,modified_unix) VALUES (?1,?2,?3,?4)
             ON CONFLICT(root,path) DO UPDATE SET bytes=excluded.bytes,modified_unix=excluded.modified_unix",
            params![root, path, meta.len() as i64, modified as i64],
        ).map_err(|e| e.to_string())?;
        change.present = true;
        change.bytes = meta.len();
        change.modified_unix = modified;
    } else {
        db.execute(
            "DELETE FROM media WHERE root=?1 AND path=?2",
            params![root, path],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(change)
}

/// Read-only library integrity summary; no missing files are deleted from the index.
#[derive(Serialize)]
pub struct LibraryHealth {
    pub indexed: usize,
    pub available: usize,
    pub missing: usize,
    pub inaccessible: usize,
    pub modified: usize,
}

fn classify_indexed_files<I>(entries: I) -> LibraryHealth
where
    I: IntoIterator<Item = (String, i64, i64)>,
{
    let mut health = LibraryHealth {
        indexed: 0,
        available: 0,
        missing: 0,
        inaccessible: 0,
        modified: 0,
    };
    for (path, expected_bytes, expected_modified) in entries {
        health.indexed += 1;
        match std::fs::metadata(path) {
            Ok(meta) if meta.is_file() => {
                health.available += 1;
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |duration| duration.as_secs());
                if meta.len() as i64 != expected_bytes || modified as i64 != expected_modified {
                    health.modified += 1;
                }
            }
            Ok(_) => health.inaccessible += 1,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => health.missing += 1,
            Err(_) => health.inaccessible += 1,
        }
    }
    health
}

#[tauri::command]
pub fn library_health(root: String) -> Result<LibraryHealth, String> {
    let directory = std::fs::canonicalize(&root).map_err(|e| e.to_string())?;
    if !directory.is_dir() {
        return Err("Library root is unavailable".into());
    }
    let db = database()?;
    let mut statement = db
        .prepare("SELECT path, bytes, modified_unix FROM media WHERE root = ?1")
        .map_err(|e| e.to_string())?;
    let records = statement
        .query_map([&root], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(classify_indexed_files(records))
}

#[derive(Serialize)]
pub struct DuplicateGroup {
    pub paths: Vec<String>,
    pub bytes: u64,
}

/// Only hashes same-sized candidate files. No files are written or deleted.
#[tauri::command]
pub fn find_exact_duplicates(
    root: String,
    paths: Vec<String>,
) -> Result<Vec<DuplicateGroup>, String> {
    find_workspace_duplicates(vec![root], paths)
}

/// Compare files across all selected workspace roots without touching originals.
/// Resolve paths before reading so paths outside the selected roots are excluded.
#[tauri::command]
pub fn find_workspace_duplicates(
    roots: Vec<String>,
    paths: Vec<String>,
) -> Result<Vec<DuplicateGroup>, String> {
    if roots.is_empty() || roots.len() > 64 || paths.len() > 40_000 {
        return Err("Select 1-64 roots and up to 40,000 media files".into());
    }
    let mut permitted = Vec::new();
    for root in roots {
        let canonical = std::fs::canonicalize(&root).map_err(|e| e.to_string())?;
        if !canonical.is_dir() {
            return Err("Workspace root is not a folder".into());
        }
        permitted.push(canonical);
    }
    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    let mut visited = std::collections::HashSet::new();
    for path in paths {
        let Ok(file) = std::fs::canonicalize(&path) else {
            continue;
        };
        if !permitted.iter().any(|root| file.starts_with(root)) || !file.is_file() {
            continue;
        }
        let extension = file
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !crate::SUPPORTED.contains(&extension.as_str()) || !visited.insert(file.clone()) {
            continue;
        }
        let Ok(meta) = file.metadata() else {
            continue;
        };
        by_size.entry(meta.len()).or_default().push(file);
    }
    let mut groups = Vec::new();
    for (bytes, same_size) in by_size {
        if same_size.len() < 2 {
            continue;
        }
        let mut by_hash: HashMap<Vec<u8>, Vec<String>> = HashMap::new();
        for path in same_size {
            if let Ok(digest) = sha256_file(&path) {
                by_hash
                    .entry(digest)
                    .or_default()
                    .push(path.to_string_lossy().into_owned());
            }
        }
        for paths in by_hash.into_values() {
            if paths.len() > 1 {
                groups.push(DuplicateGroup { paths, bytes });
            }
        }
    }
    groups.sort_by_key(|group| std::cmp::Reverse(group.bytes));
    Ok(groups)
}

fn sha256_file(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_parent_is_not_assumed_to_be_a_deleted_photo() {
        let directory =
            std::env::temp_dir().join(format!("openphoto-missing-parent-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let absent = directory.join("disconnected").join("photo.jpg");
        let result = update_indexed_path(
            directory.to_string_lossy().into_owned(),
            absent.to_string_lossy().into_owned(),
        );
        assert!(
            result.is_err(),
            "Missing folders require full reconciliation"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn directory_event_does_not_remove_indexed_media() {
        let directory =
            std::env::temp_dir().join(format!("openphoto-directory-event-{}", std::process::id()));
        let child = directory.join("photos");
        std::fs::create_dir_all(&child).unwrap();
        let result = update_indexed_path(
            directory.to_string_lossy().into_owned(),
            child.to_string_lossy().into_owned(),
        );
        assert!(result.is_err(), "Directory events must trigger a full scan");
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn health_inspection_counts_missing_without_mutating_sources() {
        let directory =
            std::env::temp_dir().join(format!("openphoto-health-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let existing = directory.join("existing.jpg");
        std::fs::write(&existing, b"photo").unwrap();
        let metadata = std::fs::metadata(&existing).unwrap();
        let modified = metadata
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let health = classify_indexed_files(vec![
            (existing.to_string_lossy().into_owned(), 5, modified),
            (directory.join("absent.jpg").to_string_lossy().into_owned(), 0, 0),
        ]);
        assert_eq!(health.indexed, 2);
        assert_eq!(health.available, 1);
        assert_eq!(health.missing, 1);
        assert_eq!(health.modified, 0);
        assert_eq!(std::fs::read(existing).unwrap(), b"photo");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn finds_duplicates_across_roots_without_double_counting() {
        let base = std::env::temp_dir().join(format!("openphoto-crossroot-{}", std::process::id()));
        let first = base.join("one");
        let second = base.join("two");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        let a = first.join("a.jpg");
        let b = second.join("b.jpg");
        std::fs::write(&a, b"identical").unwrap();
        std::fs::write(&b, b"identical").unwrap();
        let roots = vec![
            first.to_string_lossy().into_owned(),
            second.to_string_lossy().into_owned(),
        ];
        let paths = vec![
            a.to_string_lossy().into_owned(),
            b.to_string_lossy().into_owned(),
            a.to_string_lossy().into_owned(),
        ];
        let groups = find_workspace_duplicates(roots, paths).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].paths.len(), 2);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn hashes_detect_identical_content() {
        let directory =
            std::env::temp_dir().join(format!("openphoto-hash-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let a = directory.join("a.jpg");
        let b = directory.join("b.jpg");
        let c = directory.join("c.jpg");
        std::fs::write(&a, b"same").unwrap();
        std::fs::write(&b, b"same").unwrap();
        std::fs::write(&c, b"diff").unwrap();
        let paths = vec![
            a.to_string_lossy().into_owned(),
            b.to_string_lossy().into_owned(),
            c.to_string_lossy().into_owned(),
        ];
        let groups =
            find_exact_duplicates(directory.to_string_lossy().into_owned(), paths).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].paths.len(), 2);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
