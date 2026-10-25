use rusqlite::{params, Connection};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use crate::{safe_media_path, scan_library, ScanResult};

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
    transaction
        .execute("DELETE FROM media WHERE root = ?1", [&root])
        .map_err(|e| e.to_string())?;
    {
        let mut statement = transaction
            .prepare(
                "INSERT INTO media (root, path, bytes, modified_unix)
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .map_err(|e| e.to_string())?;
        for photo in &scan.photos {
            statement
                .execute(params![
                    root,
                    photo.path,
                    photo.bytes as i64,
                    photo.modified_unix as i64
                ])
                .map_err(|e| e.to_string())?;
        }
    }
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
    if paths.len() > 40_000 {
        return Err("Too many files in a duplicate search".into());
    }
    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for path in paths {
        let Ok(file) = safe_media_path(&root, &path) else {
            continue;
        };
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
