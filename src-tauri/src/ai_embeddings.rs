//! Persistent local semantic image embeddings.
//! Embeddings are stored in application data and never written to original photos.
use rusqlite::{params, Connection};
use serde::Serialize;
use std::{path::Path, time::UNIX_EPOCH};

#[derive(Serialize)]
pub struct EmbeddingStatus {
    indexed: usize,
    stale: usize,
    model_sha256: String,
}

pub(crate) fn database() -> Result<Connection, String> {
    let directory = dirs::data_local_dir()
        .ok_or("Windows local data directory unavailable")?
        .join("Open-Photo");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let connection =
        Connection::open(directory.join("ai-embeddings.sqlite")).map_err(|e| e.to_string())?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS semantic_embeddings (
                root TEXT NOT NULL,
                path TEXT NOT NULL,
                modified_unix INTEGER NOT NULL,
                model_sha256 TEXT NOT NULL,
                dimensions INTEGER NOT NULL,
                vector BLOB NOT NULL,
                PRIMARY KEY(root, path)
            );
            CREATE INDEX IF NOT EXISTS idx_semantic_model ON semantic_embeddings(model_sha256);",
        )
        .map_err(|e| e.to_string())?;
    crate::video_semantic::initialize(&connection)?;
    crate::source_version::initialize(&connection)?;
    Ok(connection)
}

fn encode_f32(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn modified_unix(source: &Path) -> Result<i64, String> {
    source
        .metadata()
        .map_err(|e| e.to_string())?
        .modified()
        .map_err(|e| e.to_string())?
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())
        .map(|duration| duration.as_secs() as i64)
}

fn embedding_is_current(
    db: &Connection,
    root: &str,
    path: &str,
    modified_unix: i64,
    model_key: &str,
    version: &str,
) -> Result<bool, String> {
    if !crate::source_version::matches(db, root, path, version)? {
        return Ok(false);
    }
    db.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM semantic_embeddings
            WHERE root=?1 AND path=?2 AND modified_unix=?3
              AND model_sha256=?4 AND dimensions=512
        )",
        params![root, path, modified_unix, model_key],
        |row| row.get::<_, i64>(0),
    )
    .map(|exists| exists != 0)
    .map_err(|e| e.to_string())
}

fn store_embedding(
    db: &Connection,
    root: &str,
    path: &str,
    modified_unix: i64,
    model_key: &str,
    values: &[f32],
    version: &str,
) -> Result<usize, String> {
    let transaction = db.unchecked_transaction().map_err(|e| e.to_string())?;
    let db = &transaction;
    let dimensions = values.len();
    let vector = encode_f32(values);
    db.execute(
        "INSERT INTO semantic_embeddings(root,path,modified_unix,model_sha256,dimensions,vector)
         VALUES(?1,?2,?3,?4,?5,?6)
         ON CONFLICT(root,path) DO UPDATE SET
           modified_unix=excluded.modified_unix,
           model_sha256=excluded.model_sha256,
           dimensions=excluded.dimensions,
           vector=excluded.vector",
        params![
            root,
            path,
            modified_unix,
            model_key,
            dimensions as i64,
            vector
        ],
    )
    .map_err(|e| e.to_string())?;
    crate::source_version::publish(db, root, path, version)?;
    transaction.commit().map_err(|e| e.to_string())?;
    Ok(dimensions)
}

#[tauri::command]
pub fn ai_index_semantic_image(
    root: String,
    path: String,
    device: Option<String>,
) -> Result<usize, String> {
    crate::ai_index::require_allowed(&root, std::slice::from_ref(&path))?;
    let source = crate::safe_media_path(&root, &path)?;
    let version = crate::source_version::capture(&root, &path)?;
    let modified_unix = modified_unix(&source)?;
    let model_key = crate::ai_models::validate_supported_clip_vision()?;
    let result = crate::ai_inference::run_semantic_image(root.clone(), path.clone(), device)?;
    let db = database()?;
    store_embedding(
        &db,
        &root,
        &path,
        modified_unix,
        &model_key,
        &result.values,
        &version,
    )
}

#[derive(Serialize)]
pub struct BatchEmbeddingResult {
    requested: usize,
    indexed: usize,
    skipped: usize,
    failed: usize,
    errors: Vec<String>,
}

fn index_semantic_batch_blocking(
    root: String,
    paths: Vec<String>,
    device: Option<String>,
) -> Result<BatchEmbeddingResult, String> {
    let model_key = crate::ai_models::validate_supported_clip_vision()?;
    let db = database()?;
    let mut session = crate::ai_inference::semantic_image_session(device.as_deref())?;
    let mut result = BatchEmbeddingResult {
        requested: paths.len(),
        indexed: 0,
        skipped: 0,
        failed: 0,
        errors: Vec::new(),
    };

    for path in paths {
        let source = match crate::safe_media_path(&root, &path) {
            Ok(source) => source,
            Err(error) => {
                result.failed += 1;
                result.errors.push(format!("{path}: {error}"));
                continue;
            }
        };
        let modified_unix = match modified_unix(&source) {
            Ok(modified) => modified,
            Err(error) => {
                result.failed += 1;
                result.errors.push(format!("{path}: {error}"));
                continue;
            }
        };
        let version = match crate::source_version::capture(&root, &path) {
            Ok(version) => version,
            Err(error) => {
                result.failed += 1;
                result.errors.push(format!("{path}: {error}"));
                continue;
            }
        };
        match embedding_is_current(&db, &root, &path, modified_unix, &model_key, &version) {
            Ok(true) => {
                result.skipped += 1;
                continue;
            }
            Ok(false) => {}
            Err(error) => {
                result.failed += 1;
                result.errors.push(format!("{path}: {error}"));
                continue;
            }
        }

        match crate::ai_inference::run_semantic_image_with_session(&mut session, &root, &path) {
            Ok(inference) => {
                if let Err(error) = store_embedding(
                    &db,
                    &root,
                    &path,
                    modified_unix,
                    &model_key,
                    &inference.values,
                    &version,
                ) {
                    result.failed += 1;
                    result.errors.push(format!("{path}: {error}"));
                } else {
                    result.indexed += 1;
                }
            }
            Err(error) => {
                result.failed += 1;
                result.errors.push(format!("{path}: {error}"));
            }
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn ai_index_semantic_batch(
    root: String,
    paths: Vec<String>,
    device: Option<String>,
) -> Result<BatchEmbeddingResult, String> {
    crate::ai_index::require_allowed(&root, &paths)?;
    if paths.is_empty() || paths.len() > 5000 {
        return Err("Select between 1 and 5,000 photos per AI indexing batch".into());
    }
    tauri::async_runtime::spawn_blocking(move || index_semantic_batch_blocking(root, paths, device))
        .await
        .map_err(|e| format!("AI indexing worker failed: {e}"))?
}

pub(crate) fn decode_f32(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if !bytes.len().is_multiple_of(4) {
        return Err("Stored embedding is corrupt".into());
    }
    Ok(bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect())
}

pub(crate) fn cosine_similarity(left: &[f32], right: &[f32]) -> Result<f32, String> {
    if left.len() != right.len() || left.is_empty() {
        return Err("Embedding dimensions do not match".into());
    }
    let mut dot = 0.0_f32;
    let mut left_norm = 0.0_f32;
    let mut right_norm = 0.0_f32;
    for (a, b) in left.iter().zip(right.iter()) {
        dot += a * b;
        left_norm += a * a;
        right_norm += b * b;
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        return Err("Cannot compare zero-length embedding vectors".into());
    }
    Ok(dot / (left_norm.sqrt() * right_norm.sqrt()))
}

#[derive(Serialize)]
pub struct SemanticMatch {
    path: String,
    score: f32,
}

#[tauri::command]
pub fn ai_find_similar_by_photo(
    root: String,
    path: String,
    limit: usize,
) -> Result<Vec<SemanticMatch>, String> {
    crate::ai_index::require_allowed(&root, std::slice::from_ref(&path))?;
    let excluded = crate::ai_index::active_exclusions()?;
    let limit = limit.clamp(1, 100);
    let current_sha = crate::ai_models::validate_supported_clip_vision()?;
    let db = database()?;
    crate::search_providers::attach_current(&db, &crate::index::current_database_path()?)?;
    crate::search_providers::prepare_scope(&db, &excluded)?;
    let query_blob: Vec<u8> = db
        .query_row(
            "SELECT a.vector FROM semantic_embeddings a JOIN visible_media m ON m.root=a.root AND m.path=a.path AND m.modified_unix=a.modified_unix JOIN ai_source_versions sv ON sv.root=a.root AND sv.path=a.path AND sv.version=m.source_version
             WHERE a.root=?1 AND a.path=?2 AND model_sha256=?3",
            params![root, path, current_sha],
            |row| row.get(0),
        )
        .map_err(|_| {
            "Index this photo with the current semantic model before searching".to_string()
        })?;
    let query = decode_f32(&query_blob)?;
    let mut statement = db
        .prepare(
            "SELECT a.path, a.vector FROM semantic_embeddings a JOIN visible_media m ON m.root=a.root AND m.path=a.path AND m.modified_unix=a.modified_unix JOIN ai_source_versions sv ON sv.root=a.root AND sv.path=a.path AND sv.version=m.source_version
             WHERE a.root=?1 AND model_sha256=?2 AND a.path<>?3",
        )
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map(params![root, current_sha, path], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        })
        .map_err(|e| e.to_string())?;
    let mut matches = Vec::new();
    for row in rows {
        let (candidate_path, blob) = row.map_err(|e| e.to_string())?;
        if !crate::ai_index::allowed(&candidate_path, &excluded) {
            continue;
        }
        let candidate = decode_f32(&blob)?;
        if candidate.len() != query.len() {
            continue;
        }
        matches.push(SemanticMatch {
            path: candidate_path,
            score: cosine_similarity(&query, &candidate)?,
        });
    }
    matches.sort_by(|a, b| b.score.total_cmp(&a.score));
    matches.truncate(limit);
    Ok(matches)
}

#[tauri::command]
pub fn ai_search_semantic_text(
    root: String,
    query: String,
    limit: usize,
    device: Option<String>,
) -> Result<Vec<SemanticMatch>, String> {
    let excluded = crate::ai_index::active_exclusions()?;
    let current_key = crate::ai_models::validate_supported_clip_bundle()?;
    let query_vector = crate::ai_text::run_semantic_text(&query, device.as_deref())?;
    let db = database()?;
    crate::search_providers::attach_current(&db, &crate::index::current_database_path()?)?;
    crate::search_providers::prepare_scope(&db, &excluded)?;
    let mut statement = db
        .prepare(
            "SELECT a.path, a.vector FROM semantic_embeddings a JOIN visible_media m ON m.root=a.root AND m.path=a.path AND m.modified_unix=a.modified_unix JOIN ai_source_versions sv ON sv.root=a.root AND sv.path=a.path AND sv.version=m.source_version
             WHERE a.root=?1 AND model_sha256=?2",
        )
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map(params![root, current_key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        })
        .map_err(|e| e.to_string())?;
    let mut matches = Vec::new();
    for row in rows {
        let (candidate_path, blob) = row.map_err(|e| e.to_string())?;
        if !crate::ai_index::allowed(&candidate_path, &excluded) {
            continue;
        }
        let candidate = decode_f32(&blob)?;
        if candidate.len() != query_vector.len() {
            continue;
        }
        matches.push(SemanticMatch {
            path: candidate_path,
            score: cosine_similarity(&query_vector, &candidate)?,
        });
    }
    matches.sort_by(|a, b| b.score.total_cmp(&a.score));
    matches.truncate(limit.clamp(1, 100));
    Ok(matches)
}

#[tauri::command]
pub fn ai_embedding_status(root: String) -> Result<EmbeddingStatus, String> {
    let current_key = crate::ai_models::validate_supported_clip_vision()?;
    let current_sha = crate::ai_models::installed_model_sha256("semantic")?;
    let db = database()?;
    crate::search_providers::attach_current(&db, &crate::index::current_database_path()?)?;
    crate::search_providers::prepare_scope(&db, &crate::ai_index::active_exclusions()?)?;
    let indexed: usize = db
        .query_row(
            "SELECT COUNT(*) FROM semantic_embeddings WHERE root=?1",
            [&root],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())? as usize;
    let stale: usize = db
        .query_row(
            "SELECT COUNT(*) FROM semantic_embeddings a WHERE a.root=?1 AND (a.model_sha256<>?2 OR NOT EXISTS(SELECT 1 FROM visible_media m JOIN ai_source_versions sv ON sv.root=m.root AND sv.path=m.path AND sv.version=m.source_version WHERE m.root=a.root AND m.path=a.path AND m.modified_unix=a.modified_unix))",
            params![root, current_key],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())? as usize;
    Ok(EmbeddingStatus {
        indexed,
        stale,
        model_sha256: current_sha,
    })
}

/// One session for a bounded background service pass, not one model load per image.
pub(crate) struct IndexWorker {
    db: Connection,
    key: String,
    session: ort::session::Session,
}
impl IndexWorker {
    pub(crate) fn new(device: &str) -> Result<Self, String> {
        Ok(Self {
            db: database()?,
            key: crate::ai_models::validate_supported_clip_vision()?,
            session: crate::ai_inference::semantic_image_session(Some(device))?,
        })
    }
    pub(crate) fn process(
        &mut self,
        root: &str,
        path: &str,
        modified: i64,
        bytes: u64,
    ) -> Result<(), String> {
        let version = crate::source_version::capture(root, path)?;
        crate::ai_index::verify_source(root, path, modified, bytes)?;
        if embedding_is_current(&self.db, root, path, modified, &self.key, &version)? {
            return Ok(());
        }
        let result =
            crate::ai_inference::run_semantic_image_with_session(&mut self.session, root, path)?;
        crate::ai_index::verify_source(root, path, modified, bytes)?;
        store_embedding(
            &self.db,
            root,
            path,
            modified,
            &self.key,
            &result.values,
            &version,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_with_preserved_mtime_replaces_vector_and_rejects_old_publication() {
        let folder = std::env::temp_dir().join(format!(
            "openphoto-embedding-version-{}",
            crate::video_project::id()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("photo.jpg");
        std::fs::write(&file, b"first").unwrap();
        let time = file.metadata().unwrap().modified().unwrap();
        let root = folder.to_str().unwrap();
        let path = file.to_str().unwrap();
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE semantic_embeddings(root TEXT,path TEXT,modified_unix INTEGER,model_sha256 TEXT,dimensions INTEGER,vector BLOB,PRIMARY KEY(root,path));").unwrap();
        crate::source_version::initialize(&db).unwrap();
        let before = crate::source_version::capture(root, path).unwrap();
        store_embedding(&db, root, path, 10, "model", &[1.; 512], &before).unwrap();
        std::fs::write(&file, b"replacement has different size").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(time))
            .unwrap();
        let after = crate::source_version::capture(root, path).unwrap();
        assert!(!embedding_is_current(&db, root, path, 10, "model", &after).unwrap());
        assert!(store_embedding(&db, root, path, 10, "model", &[3.; 512], &before).is_err());
        let blob: Vec<u8> = db
            .query_row("SELECT vector FROM semantic_embeddings", [], |r| r.get(0))
            .unwrap();
        assert_eq!(decode_f32(&blob).unwrap(), vec![1.; 512]);
        store_embedding(&db, root, path, 10, "model", &[2.; 512], &after).unwrap();
        let blob: Vec<u8> = db
            .query_row("SELECT vector FROM semantic_embeddings", [], |r| r.get(0))
            .unwrap();
        assert_eq!(decode_f32(&blob).unwrap(), vec![2.; 512]);
        assert!(embedding_is_current(&db, root, path, 10, "model", &after).unwrap());
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn cosine_similarity_ranks_identical_vectors_highest() {
        let identical = cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]).unwrap();
        let orthogonal = cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]).unwrap();
        assert!(identical > orthogonal);
        assert!((identical - 1.0).abs() < 0.0001);
    }

    #[test]
    fn round_trips_embedding_bytes() {
        let source = [1.0, 0.5, -0.25];
        assert_eq!(decode_f32(&encode_f32(&source)).unwrap(), source);
    }

    #[test]
    fn encodes_embedding_values_as_little_endian_f32() {
        let bytes = encode_f32(&[1.0, 0.5]);
        assert_eq!(bytes.len(), 8);
        assert_eq!(&bytes[0..4], &1.0_f32.to_le_bytes());
        assert_eq!(&bytes[4..8], &0.5_f32.to_le_bytes());
    }
}
