//! Persistent local semantic image embeddings.
//! Embeddings are stored in application data and never written to original photos.
use rusqlite::{params, Connection};
use serde::Serialize;
use std::time::UNIX_EPOCH;

#[derive(Serialize)]
pub struct EmbeddingStatus {
    indexed: usize,
    stale: usize,
    model_sha256: String,
}

fn database() -> Result<Connection, String> {
    let directory = dirs::data_local_dir()
        .ok_or("Windows local data directory unavailable")?
        .join("Open-Photo");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let connection =
        Connection::open(directory.join("ai-embeddings.sqlite")).map_err(|e| e.to_string())?;
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
    Ok(connection)
}

fn encode_f32(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[tauri::command]
pub fn ai_index_semantic_image(root: String, path: String) -> Result<usize, String> {
    let source = crate::safe_media_path(&root, &path)?;
    let modified_unix = source
        .metadata()
        .map_err(|e| e.to_string())?
        .modified()
        .map_err(|e| e.to_string())?
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;
    let model_sha256 = crate::ai_models::installed_model_sha256("semantic")?;
    let result = crate::ai_inference::run_semantic_image(root.clone(), path.clone())?;
    let dimensions = result.values.len();
    let vector = encode_f32(&result.values);
    let db = database()?;
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
            model_sha256,
            dimensions as i64,
            vector
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(dimensions)
}

#[derive(Serialize)]
pub struct BatchEmbeddingResult {
    requested: usize,
    indexed: usize,
    skipped: usize,
    errors: Vec<String>,
}

#[tauri::command]
pub fn ai_index_semantic_batch(root: String, paths: Vec<String>) -> Result<BatchEmbeddingResult, String> {
    if paths.is_empty() || paths.len() > 500 {
        return Err("Select between 1 and 500 photos per AI indexing batch".into());
    }
    let mut result = BatchEmbeddingResult {
        requested: paths.len(),
        indexed: 0,
        skipped: 0,
        errors: Vec::new(),
    };
    for path in paths {
        match ai_index_semantic_image(root.clone(), path.clone()) {
            Ok(_) => result.indexed += 1,
            Err(error) => {
                result.skipped += 1;
                result.errors.push(format!("{path}: {error}"));
            }
        }
    }
    Ok(result)
}

#[tauri::command]
pub fn ai_embedding_status(root: String) -> Result<EmbeddingStatus, String> {
    let current_sha = crate::ai_models::installed_model_sha256("semantic")?;
    let db = database()?;
    let indexed: usize = db
        .query_row(
            "SELECT COUNT(*) FROM semantic_embeddings WHERE root=?1",
            [&root],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())? as usize;
    let stale: usize = db
        .query_row(
            "SELECT COUNT(*) FROM semantic_embeddings WHERE root=?1 AND model_sha256<>?2",
            params![root, current_sha],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())? as usize;
    Ok(EmbeddingStatus {
        indexed,
        stale,
        model_sha256: current_sha,
    })
}

#[cfg(test)]
mod tests {
    use super::encode_f32;

    #[test]
    fn encodes_embedding_values_as_little_endian_f32() {
        let bytes = encode_f32(&[1.0, 0.5]);
        assert_eq!(bytes.len(), 8);
        assert_eq!(&bytes[0..4], &1.0_f32.to_le_bytes());
        assert_eq!(&bytes[4..8], &0.5_f32.to_le_bytes());
    }
}
