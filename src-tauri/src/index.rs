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
    let scan = scan_library(root.clone())?;
    // A truncated scan is not a complete snapshot: do not evict previously
    // indexed media based on missing entries.
    if scan.truncated {
        return Ok(scan);
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
            seen.execute([&photo.path]).map_err(|e| e.to_string())?;
            upsert
                .execute(params![root, photo.path, photo.bytes as i64, photo.modified_unix as i64])
                .map_err(|e| e.to_string())?;
        }
    }
    transaction
        .execute(
            "DELETE FROM media WHERE root = ?1
             AND NOT EXISTS (SELECT 1 FROM seen_paths WHERE seen_paths.path = media.path)",
            [&root],
        )
        .map_err(|e| e.to_string())?;
    transaction.commit().map_err(|e| e.to_string())?;
    Ok(scan)
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
        let extension = file.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
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
        let roots = vec![first.to_string_lossy().into_owned(), second.to_string_lossy().into_owned()];
        let paths = vec![a.to_string_lossy().into_owned(), b.to_string_lossy().into_owned(), a.to_string_lossy().into_owned()];
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
