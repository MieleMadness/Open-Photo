//! Shared identity for saved media, queued work and AI publication.
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path, time::UNIX_EPOCH};

/// Stream content with bounded scratch memory; timestamps alone are not identity.
pub(crate) fn read(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let before = file.metadata().map_err(|e| e.to_string())?;
    let modified = before.modified().map_err(|e| e.to_string())?;
    let time = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0_u8; 128 * 1024];
    let mut bytes = 0_u64;
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        bytes += count as u64;
        if bytes > before.len() {
            return Err("Source grew while reading; Sync the library and retry".into());
        }
        hash.update(&buffer[..count]);
    }
    let after = file.metadata().map_err(|e| e.to_string())?;
    let current = path.metadata().map_err(|e| e.to_string())?;
    if bytes != before.len()
        || after.len() != before.len()
        || after.modified().map_err(|e| e.to_string())? != modified
        || current.len() != before.len()
        || current.modified().map_err(|e| e.to_string())? != modified
    {
        return Err("Source changed while reading; Sync the library and retry".into());
    }
    Ok(format!(
        "source-v1:{}:{}:{}:{:x}",
        bytes,
        time.as_secs(),
        time.subsec_nanos(),
        hash.finalize()
    ))
}

pub(crate) fn capture(root: &str, path: &str) -> Result<String, String> {
    read(&crate::safe_media_path(root, path)?)
}

pub(crate) fn verify(root: &str, path: &str, expected: &str) -> Result<(), String> {
    if capture(root, path)? != expected {
        return Err("Source changed; Sync the library before retrying AI indexing".into());
    }
    Ok(())
}

/// Missing ledger rows intentionally invalidate all legacy AI results.
pub(crate) fn initialize(db: &Connection) -> Result<(), String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS ai_source_versions(root TEXT,path TEXT,version TEXT NOT NULL,PRIMARY KEY(root,path));
        CREATE TABLE IF NOT EXISTS ai_staged_versions(root TEXT,path TEXT,version TEXT NOT NULL,PRIMARY KEY(root,path));")
        .map_err(|e| e.to_string())
}

pub(crate) fn matches(
    db: &Connection,
    root: &str,
    path: &str,
    version: &str,
) -> Result<bool, String> {
    db.query_row(
        "SELECT version FROM ai_source_versions WHERE root=?1 AND path=?2",
        params![root, path],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .map(|saved| saved.as_deref() == Some(version))
    .map_err(|e| e.to_string())
}

/// Call inside the same transaction as the result rows; failed verification rolls both back.
pub(crate) fn publish(
    db: &Connection,
    root: &str,
    path: &str,
    version: &str,
) -> Result<(), String> {
    verify(root, path, version)?;
    db.execute("INSERT INTO ai_source_versions VALUES(?1,?2,?3) ON CONFLICT(root,path) DO UPDATE SET version=excluded.version", params![root,path,version])
        .map(|_| ()).map_err(|e| e.to_string())
}

pub(crate) fn add_media_column(db: &Connection) -> Result<(), String> {
    let mut statement = db
        .prepare("PRAGMA table_info(media)")
        .map_err(|e| e.to_string())?;
    let columns = statement
        .query_map([], |r| r.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    if !columns.iter().any(|name| name == "source_version") {
        db.execute("ALTER TABLE media ADD COLUMN source_version TEXT", [])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserved_timestamps_and_same_length_replacements_change_identity() {
        let path =
            std::env::temp_dir().join(format!("openphoto-version-{}", crate::video_project::id()));
        std::fs::write(&path, b"first").unwrap();
        let time = path.metadata().unwrap().modified().unwrap();
        let initial = read(&path).unwrap();
        std::fs::write(&path, b"other").unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(time))
            .unwrap();
        let replacement = read(&path).unwrap();
        assert_ne!(initial, replacement);
        std::fs::write(&path, b"larger content").unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(time))
            .unwrap();
        assert_ne!(replacement, read(&path).unwrap());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn legacy_records_never_count_as_current() {
        let db = Connection::open_in_memory().unwrap();
        initialize(&db).unwrap();
        assert!(!matches(&db, "root", "photo", "source-v1:new").unwrap());
        db.execute(
            "INSERT INTO ai_source_versions VALUES('root','photo','source-v1:old')",
            [],
        )
        .unwrap();
        assert!(!matches(&db, "root", "photo", "source-v1:new").unwrap());
        assert!(matches(&db, "root", "photo", "source-v1:old").unwrap());
    }
}
