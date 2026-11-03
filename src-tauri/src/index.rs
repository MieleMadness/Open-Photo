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

pub(crate) fn database_path() -> Result<PathBuf, String> {
    let directory = dirs::data_local_dir()
        .ok_or("Windows local data directory unavailable")?
        .join("Open-Photo");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    Ok(directory.join("media-index.sqlite"))
}
fn database() -> Result<Connection, String> {
    let connection = Connection::open(database_path()?).map_err(|e| e.to_string())?;
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS media (
                root TEXT NOT NULL,
                path TEXT NOT NULL,
                bytes INTEGER NOT NULL,
                modified_unix INTEGER NOT NULL,
                capture_local TEXT,
                live_asset_id TEXT,
                PRIMARY KEY(root, path)
            );
            CREATE INDEX IF NOT EXISTS idx_media_size ON media(bytes);",
        )
        .map_err(|e| e.to_string())?;
    let has_capture_local = connection
        .prepare("PRAGMA table_info(media)")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(|e| e.to_string())?
        .iter()
        .any(|column| column == "capture_local");
    if !has_capture_local {
        connection
            .execute("ALTER TABLE media ADD COLUMN capture_local TEXT", [])
            .map_err(|e| e.to_string())?;
    }
    let has_live_asset_id = connection
        .prepare("PRAGMA table_info(media)")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(|e| e.to_string())?
        .iter()
        .any(|column| column == "live_asset_id");
    if !has_live_asset_id {
        connection
            .execute("ALTER TABLE media ADD COLUMN live_asset_id TEXT", [])
            .map_err(|e| e.to_string())?;
    }
    Ok(connection)
}

/// Refresh the local SQLite index from a read-only filesystem scan.
/// This never removes or edits source files.
#[tauri::command]
pub fn scan_indexed(root: String) -> Result<ScanResult, String> {
    Ok(scan_indexed_with_changes(root)?.scan)
}

/// Restore known media without touching source folders (including offline drives).
#[tauri::command]
pub fn saved_library(root: String) -> Result<ScanResult, String> {
    saved_library_from(&database()?, &root)
}
fn saved_library_from(db: &Connection, root: &str) -> Result<ScanResult, String> {
    let mut statement = db.prepare("SELECT path,bytes,modified_unix,capture_local,live_asset_id FROM media WHERE root=?1 ORDER BY path LIMIT 100001").map_err(|e| e.to_string())?;
    let mut photos = statement
        .query_map([root], |row| {
            let path: String = row.get(0)?;
            let name = path.rsplit(['/', '\\']).next().unwrap_or(&path).to_string();
            let extension = name
                .rsplit_once('.')
                .map(|(_, ext)| ext.to_ascii_lowercase())
                .unwrap_or_default();
            Ok(crate::Photo {
                path,
                name,
                extension,
                bytes: row.get(1)?,
                modified_unix: row.get(2)?,
                capture_local: row.get(3)?,
                live_asset_id: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let truncated = photos.len() > 100_000;
    photos.truncate(100_000);
    Ok(ScanResult {
        scanned_files: photos.len(),
        photos,
        truncated,
        incomplete: truncated,
        errors: vec![],
    })
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
    reconcile_scan(&mut database()?, root, scan)
}

fn reconcile_scan(
    db: &mut Connection,
    root: String,
    scan: ScanResult,
) -> Result<IndexedScan, String> {
    let mut changes = ChangeCounts {
        added: 0,
        updated: 0,
        unchanged: 0,
        removed: 0,
        incomplete: scan.incomplete,
    };
    // A truncated scan is not a complete snapshot: do not evict previously
    // indexed media based on missing entries.
    if scan.incomplete {
        return Ok(IndexedScan { scan, changes });
    }
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
    let previous_entries: HashMap<String, (i64, i64, Option<String>, Option<String>)> = {
        let mut statement = transaction
            .prepare("SELECT path, bytes, modified_unix, capture_local, live_asset_id FROM media WHERE root = ?1")
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([&root], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    (
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ),
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
                "INSERT INTO media(root, path, bytes, modified_unix, capture_local, live_asset_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(root, path) DO UPDATE SET
                     bytes = excluded.bytes,
                     modified_unix = excluded.modified_unix,
                     capture_local = excluded.capture_local,
                     live_asset_id = excluded.live_asset_id
                 WHERE media.bytes != excluded.bytes
                    OR media.modified_unix != excluded.modified_unix
                    OR media.capture_local IS NOT excluded.capture_local
                    OR media.live_asset_id IS NOT excluded.live_asset_id",
            )
            .map_err(|e| e.to_string())?;
        for photo in &scan.photos {
            match previous_entries.get(&photo.path) {
                Some((bytes, modified, capture_local, live_asset_id)) => {
                    if *bytes != photo.bytes as i64
                        || *modified != photo.modified_unix as i64
                        || capture_local != &photo.capture_local
                        || live_asset_id != &photo.live_asset_id
                    {
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
                    photo.modified_unix as i64,
                    photo.capture_local,
                    photo.live_asset_id
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
    pub capture_local: Option<String>,
    pub live_asset_id: Option<String>,
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
        capture_local: None,
        live_asset_id: None,
    };
    let db = database()?;
    if let Some(meta) = metadata.filter(|meta| meta.is_file()) {
        let modified = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |duration| duration.as_secs());
        let capture_local = actual
            .as_ref()
            .and_then(|resolved| crate::read_capture_local(resolved, &extension));
        let live_asset_id = actual
            .as_ref()
            .and_then(|resolved| crate::read_live_asset_id(resolved, &extension));
        db.execute(
            "INSERT INTO media(root,path,bytes,modified_unix,capture_local,live_asset_id) VALUES (?1,?2,?3,?4,?5,?6)
             ON CONFLICT(root,path) DO UPDATE SET
                bytes=excluded.bytes,
                modified_unix=excluded.modified_unix,
                capture_local=excluded.capture_local,
                live_asset_id=excluded.live_asset_id",
            params![
                root,
                path,
                meta.len() as i64,
                modified as i64,
                capture_local,
                live_asset_id
            ],
        )
        .map_err(|e| e.to_string())?;
        change.present = true;
        change.bytes = meta.len();
        change.modified_unix = modified;
        change.capture_local = capture_local;
        change.live_asset_id = live_asset_id;
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
    fn restores_only_requested_root_without_accessing_missing_source_files() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE media(root TEXT,path TEXT,bytes INTEGER,modified_unix INTEGER,capture_local TEXT,live_asset_id TEXT);
            INSERT INTO media VALUES('offline','Z:/missing/世界.JPG',42,10,'2026-10-30T12:00:00','pair'),('other','Z:/other.jpg',8,20,NULL,NULL);").unwrap();
        let restored = saved_library_from(&db, "offline").unwrap();
        assert_eq!(restored.photos.len(), 1);
        assert_eq!(restored.photos[0].name, "世界.JPG");
        assert_eq!(restored.photos[0].extension, "jpg");
        assert_eq!(restored.photos[0].bytes, 42);
        assert_eq!(restored.photos[0].live_asset_id.as_deref(), Some("pair"));
        assert_eq!(
            restored.photos[0].capture_local.as_deref(),
            Some("2026-10-30T12:00:00")
        );
        assert!(saved_library_from(&db, "unknown")
            .unwrap()
            .photos
            .is_empty());
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM media", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            2
        );
    }

    #[test]
    fn incomplete_snapshot_keeps_previous_index_but_complete_snapshot_reconciles() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE media(root TEXT,path TEXT,bytes INTEGER,modified_unix INTEGER,capture_local TEXT,live_asset_id TEXT,PRIMARY KEY(root,path));
            INSERT INTO media VALUES('library','photo.jpg',10,1,NULL,NULL);").unwrap();
        let incomplete = ScanResult {
            photos: vec![],
            scanned_files: 1,
            truncated: false,
            incomplete: true,
            errors: vec!["metadata unavailable".into()],
        };
        let result = reconcile_scan(&mut db, "library".into(), incomplete).unwrap();
        assert!(result.changes.incomplete);
        assert_eq!(result.changes.removed, 0);
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM media", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        let complete = ScanResult {
            photos: vec![],
            scanned_files: 0,
            truncated: false,
            incomplete: false,
            errors: vec![],
        };
        assert_eq!(
            reconcile_scan(&mut db, "library".into(), complete)
                .unwrap()
                .changes
                .removed,
            1
        );
    }

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
            (
                directory.join("absent.jpg").to_string_lossy().into_owned(),
                0,
                0,
            ),
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
