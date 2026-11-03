//! Adapters over existing local indexes; these queries never regroup People or index source files.
use crate::search::{self, Batch, Evidence, Kind, MAX_MATCHES, MAX_SCAN};
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};

/// ATTACH uses a fixed local index path, opened read-only; source paths never become database names.
pub(crate) fn attach_current(db: &Connection, path: &std::path::Path) -> Result<(), String> {
    if !path.is_file() {
        return Err("Sync the workspace Library before searching content indexes".into());
    }
    let uri = format!(
        "file:{}?mode=ro",
        path.to_string_lossy()
            .replace('\\', "/")
            .replace('%', "%25")
            .replace('?', "%3F")
            .replace('#', "%23")
    );
    db.execute("ATTACH DATABASE ?1 AS current_media", [uri])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Exclude before ranking/scan caps. Normal filesystem metadata remains untouched.
pub(crate) fn prepare_scope(db: &Connection, excluded: &[String]) -> Result<(), String> {
    // SQLite lower() is ASCII-only. Resolve Windows/Unicode path policy in Rust,
    // then apply indexed anti-joins before provider limits.
    db.execute_batch("CREATE TEMP TABLE IF NOT EXISTS blocked_ai_roots(root TEXT PRIMARY KEY); CREATE TEMP TABLE IF NOT EXISTS blocked_ai_paths(root TEXT,path TEXT,PRIMARY KEY(root,path)); DELETE FROM blocked_ai_roots; DELETE FROM blocked_ai_paths;").map_err(|e|e.to_string())?;
    if !excluded.is_empty() {
        let mut statement = db
            .prepare("SELECT DISTINCT root FROM current_media.media")
            .map_err(|e| e.to_string())?;
        let roots = statement
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        for root in roots {
            if !crate::ai_index::allowed(&root, excluded) {
                db.execute("INSERT INTO blocked_ai_roots VALUES(?1)", [&root])
                    .map_err(|e| e.to_string())?;
                continue;
            }
            if !excluded.iter().any(|folder| {
                crate::ai_index::normalized(folder)
                    .starts_with(&(crate::ai_index::normalized(&root) + "/"))
            }) {
                continue;
            }
            let mut paths = db
                .prepare("SELECT path FROM current_media.media WHERE root=?1")
                .map_err(|e| e.to_string())?;
            for row in paths
                .query_map([&root], |r| r.get::<_, String>(0))
                .map_err(|e| e.to_string())?
            {
                let path = row.map_err(|e| e.to_string())?;
                if !crate::ai_index::allowed(&path, excluded) {
                    db.execute(
                        "INSERT OR IGNORE INTO blocked_ai_paths VALUES(?1,?2)",
                        params![root, path],
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
        }
    }
    db.execute_batch("CREATE TEMP VIEW IF NOT EXISTS visible_media AS SELECT * FROM current_media.media m WHERE NOT EXISTS(SELECT 1 FROM blocked_ai_roots b WHERE b.root=m.root) AND NOT EXISTS(SELECT 1 FROM blocked_ai_paths b WHERE b.root=m.root AND b.path=m.path);").map_err(|e|e.to_string())
}
pub(crate) fn ocr(
    roots: &[String],
    query: &[String],
    excluded: &[String],
    cancelled: impl Fn() -> bool,
) -> Result<Batch, String> {
    {
        let db = crate::ai_ocr::database()?;
        attach_current(&db, &crate::index::database_path()?)?;
        prepare_scope(&db, excluded)?;
        ocr_from(&db, roots, query, cancelled)
    }
}
fn ocr_from(
    db: &Connection,
    roots: &[String],
    query: &[String],
    cancelled: impl Fn() -> bool,
) -> Result<Batch, String> {
    let words: Vec<String> = query
        .iter()
        .map(|term| term.to_lowercase())
        .filter(|term| !term.is_empty())
        .collect();
    let key = format!(
        "ppocrv4-en-rec-mobile:{}:{}",
        crate::image_decode::POLICY,
        crate::ai_models::SUPPORTED_OCR_RECOGNIZER_SHA256
    );
    let mut statement=db.prepare("SELECT a.root,a.path,a.modified_unix,a.text FROM ocr_index a JOIN visible_media m ON m.root=a.root AND m.path=a.path AND m.modified_unix=a.modified_unix WHERE a.root IN (SELECT value FROM json_each(?1)) AND a.recognizer_key=?2 ORDER BY a.root,a.path LIMIT ?3").map_err(|e|e.to_string())?;
    let rows = statement
        .query_map(
            params![
                serde_json::to_string(roots).map_err(|e| e.to_string())?,
                key,
                (MAX_SCAN + 1) as i64
            ],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )
        .map_err(|e| e.to_string())?;
    let mut batch = Batch::default();
    for (index, row) in rows.enumerate() {
        if cancelled() {
            return Ok(Batch::default());
        }
        if index == MAX_SCAN {
            batch.truncated = true;
            break;
        }
        let (root, path, modified_unix, text) = row.map_err(|e| e.to_string())?;
        let lower = text.to_lowercase();
        let matched_terms: Vec<String> = words
            .iter()
            .filter(|w| lower.contains(w.as_str()))
            .cloned()
            .collect();
        if !matched_terms.is_empty() {
            if batch.evidence.len() == MAX_MATCHES {
                batch.truncated = true;
                break;
            }
            batch.evidence.push(Evidence {
                root,
                path,
                modified_unix,
                kind: Kind::Ocr,
                text: search::excerpt(&text, &matched_terms),
                matched_terms,
                score: 1.0,
                timestamp_seconds: None,
            });
        }
    }
    Ok(batch)
}
pub(crate) fn people(
    roots: &[String],
    query: &str,
    excluded: &[String],
    cancelled: impl Fn() -> bool,
) -> Result<Batch, String> {
    {
        let db = crate::ai_faces::database()?;
        attach_current(&db, &crate::index::database_path()?)?;
        prepare_scope(&db, excluded)?;
        people_from(&db, roots, query, cancelled)
    }
}
fn people_from(
    db: &Connection,
    roots: &[String],
    query: &str,
    cancelled: impl Fn() -> bool,
) -> Result<Batch, String> {
    let words = search::terms(query);
    let key = format!(
        "sface-2021dec:{}:{}",
        crate::image_decode::POLICY,
        crate::ai_models::SUPPORTED_SFACE_SHA256
    );
    let mut statement=db.prepare("SELECT a.root,a.path,a.modified_unix,a.vector FROM face_embeddings a JOIN visible_media m ON m.root=a.root AND m.path=a.path AND m.modified_unix=a.modified_unix WHERE a.root IN (SELECT value FROM json_each(?1)) AND a.embedder_key=?2 AND a.dimensions=128 AND length(a.vector)=512 ORDER BY a.root,a.path,a.face_index LIMIT ?3").map_err(|e|e.to_string())?;
    let rows = statement
        .query_map(
            params![
                serde_json::to_string(roots).map_err(|e| e.to_string())?,
                key,
                (MAX_SCAN + 1) as i64
            ],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Vec<u8>>(3)?,
                ))
            },
        )
        .map_err(|e| e.to_string())?;
    let mut batch = Batch::default();
    let mut candidates = Vec::new();
    let mut keys = HashSet::new();
    for (index, row) in rows.enumerate() {
        if cancelled() {
            return Ok(Batch::default());
        }
        if index == MAX_SCAN {
            batch.truncated = true;
            break;
        }
        let (root, path, modified, blob) = row.map_err(|e| e.to_string())?;
        let face_key = crate::ai_faces::face_key(&key, &blob);
        keys.insert(face_key.clone());
        candidates.push((root, path, modified, face_key));
    }
    let names: HashMap<String, String> = {
        let mut statement=db.prepare("SELECT f.face_key,p.name FROM person_faces f JOIN people p ON p.id=f.person_id WHERE trim(p.name)<>'' AND f.face_key IN (SELECT value FROM json_each(?1))").map_err(|e|e.to_string())?;
        let rows = statement
            .query_map(
                [serde_json::to_string(&keys).map_err(|e| e.to_string())?],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<HashMap<_, _>, _>>()
            .map_err(|e| e.to_string())?
    };
    let names: HashMap<_, _> = names
        .into_iter()
        .filter(|(_, name)| words.iter().any(|w| name.to_lowercase().contains(w)))
        .collect();
    let mut seen = HashSet::new();
    for (root, path, modified_unix, key) in candidates {
        if cancelled() {
            return Ok(Batch::default());
        }
        if let Some(name) = names.get(&key) {
            if seen.insert((root.clone(), path.clone(), name.clone())) {
                if batch.evidence.len() == MAX_MATCHES {
                    batch.truncated = true;
                    break;
                }
                batch.evidence.push(Evidence {
                    root,
                    path,
                    modified_unix,
                    kind: Kind::Person,
                    text: name.chars().take(200).collect(),
                    matched_terms: Vec::new(),
                    score: 1.0,
                    timestamp_seconds: None,
                });
            }
        }
    }
    Ok(batch)
}
pub(crate) fn semantic(
    roots: &[String],
    vector: &[f32],
    excluded: &[String],
    cancelled: impl Fn() -> bool,
) -> Result<Batch, String> {
    {
        let db = crate::ai_embeddings::database()?;
        attach_current(&db, &crate::index::database_path()?)?;
        prepare_scope(&db, excluded)?;
        let mut photos = semantic_from(&db, roots, vector, &cancelled)?;
        let videos = video_semantic_from(&db, roots, vector, &cancelled)?;
        photos.truncated |= videos.truncated;
        photos.evidence.extend(videos.evidence);
        photos
            .evidence
            .sort_by(|a, b| b.score.total_cmp(&a.score).then(a.path.cmp(&b.path)));
        let mut parents = HashMap::new();
        let mut kept = vec![];
        for item in photos.evidence {
            let id = (item.root.clone(), item.path.clone());
            if !parents.contains_key(&id) && parents.len() >= 200 {
                photos.truncated = true;
                continue;
            }
            let count = parents.entry(id).or_insert(0);
            if *count < 5 {
                *count += 1;
                kept.push(item);
            }
        }
        photos.evidence = kept;
        Ok(photos)
    }
}
fn semantic_from(
    db: &Connection,
    roots: &[String],
    vector: &[f32],
    cancelled: impl Fn() -> bool,
) -> Result<Batch, String> {
    let key = format!(
        "openclip-vit-b32-oriented-v2:{}",
        crate::ai_models::SUPPORTED_CLIP_VISION_SHA256
    );
    let mut statement=db.prepare("SELECT a.root,a.path,a.modified_unix,a.vector FROM semantic_embeddings a JOIN visible_media m ON m.root=a.root AND m.path=a.path AND m.modified_unix=a.modified_unix WHERE a.root IN (SELECT value FROM json_each(?1)) AND a.model_sha256=?2 AND a.dimensions=512 AND length(a.vector)=2048 ORDER BY a.root,a.path LIMIT ?3").map_err(|e|e.to_string())?;
    let rows = statement
        .query_map(
            params![
                serde_json::to_string(roots).map_err(|e| e.to_string())?,
                key,
                (MAX_SCAN + 1) as i64
            ],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Vec<u8>>(3)?,
                ))
            },
        )
        .map_err(|e| e.to_string())?;
    let mut batch = Batch::default();
    for (index, row) in rows.enumerate() {
        if cancelled() {
            return Ok(Batch::default());
        }
        if index == MAX_SCAN {
            batch.truncated = true;
            break;
        }
        let (root, path, modified_unix, blob) = row.map_err(|e| e.to_string())?;
        let candidate = crate::ai_embeddings::decode_f32(&blob)?;
        let score = crate::ai_embeddings::cosine_similarity(vector, &candidate)?;
        if score.is_finite() && score >= 0.2 {
            batch.evidence.push(Evidence {
                root,
                path,
                modified_unix,
                kind: Kind::Semantic,
                text: String::new(),
                matched_terms: Vec::new(),
                score,
                timestamp_seconds: None,
            });
        }
    }
    batch
        .evidence
        .sort_by(|a, b| b.score.total_cmp(&a.score).then(a.path.cmp(&b.path)));
    if batch.evidence.len() > 200 {
        batch.truncated = true;
        batch.evidence.truncate(200);
    }
    Ok(batch)
}

fn video_semantic_from(
    db: &Connection,
    roots: &[String],
    vector: &[f32],
    cancelled: impl Fn() -> bool,
) -> Result<Batch, String> {
    let mut stmt=db.prepare("SELECT v.root,v.path,v.modified,v.seconds,v.vector FROM video_frames v JOIN video_index_state s ON s.root=v.root AND s.path=v.path AND s.modified=v.modified AND s.bytes=v.bytes AND s.model=v.model JOIN visible_media m ON m.root=v.root AND m.path=v.path AND m.modified_unix=v.modified AND m.bytes=v.bytes WHERE v.root IN(SELECT value FROM json_each(?1)) AND v.model=?2 AND length(v.vector)=2048 ORDER BY v.root,v.path,v.seconds LIMIT ?3").map_err(|e|e.to_string())?;
    let rows = stmt
        .query_map(
            params![
                serde_json::to_string(roots).map_err(|e| e.to_string())?,
                crate::video_semantic::key(),
                (MAX_SCAN + 1) as i64
            ],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, f64>(3)?,
                    r.get::<_, Vec<u8>>(4)?,
                ))
            },
        )
        .map_err(|e| e.to_string())?;
    let mut batch = Batch::default();
    let mut parents: HashMap<(String, String), Vec<Evidence>> = HashMap::new();
    for (index, row) in rows.enumerate() {
        if cancelled() {
            return Ok(Batch::default());
        }
        if index == MAX_SCAN {
            batch.truncated = true;
            break;
        }
        let (root, path, modified, seconds, blob) = row.map_err(|e| e.to_string())?;
        if !seconds.is_finite() || seconds < 0. {
            continue;
        }
        let candidate = crate::ai_embeddings::decode_f32(&blob)?;
        let score = crate::ai_embeddings::cosine_similarity(vector, &candidate)?;
        if !score.is_finite() || score < 0.2 {
            continue;
        }
        let hits = parents.entry((root.clone(), path.clone())).or_default();
        hits.push(Evidence {
            root,
            path,
            modified_unix: modified,
            kind: Kind::VideoSemantic,
            text: String::new(),
            matched_terms: vec![],
            score,
            timestamp_seconds: Some(seconds),
        });
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        hits.truncate(5);
    }
    let mut groups = parents.into_values().collect::<Vec<_>>();
    groups.sort_by(|a, b| {
        b[0].score
            .total_cmp(&a[0].score)
            .then(a[0].path.cmp(&b[0].path))
    });
    if groups.len() > 200 {
        batch.truncated = true;
        groups.truncate(200);
    }
    batch.evidence = groups.into_iter().flatten().collect();
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn db() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("ATTACH DATABASE ':memory:' AS current_media; CREATE TABLE current_media.media(root TEXT,path TEXT,modified_unix INTEGER,bytes INTEGER DEFAULT 0,PRIMARY KEY(root,path));").unwrap();
        db.execute_batch("CREATE TABLE ocr_index(root TEXT,path TEXT,modified_unix INTEGER,text TEXT,recognizer_key TEXT);CREATE TABLE face_embeddings(root TEXT,path TEXT,modified_unix INTEGER,vector BLOB,embedder_key TEXT,dimensions INTEGER,face_index INTEGER);CREATE TABLE people(id TEXT PRIMARY KEY,name TEXT);CREATE TABLE person_faces(face_key TEXT PRIMARY KEY,person_id TEXT);CREATE TABLE semantic_embeddings(root TEXT,path TEXT,modified_unix INTEGER,vector BLOB,model_sha256 TEXT,dimensions INTEGER);").unwrap();
        crate::video_semantic::initialize(&db).unwrap();
        prepare_scope(&db, &[]).unwrap();
        db
    }
    fn vector(value: f32, size: usize) -> Vec<u8> {
        (0..size).flat_map(|_| value.to_le_bytes()).collect()
    }
    #[test]
    fn video_matches_require_committed_current_frames_and_return_parent_timestamps() {
        let db = db();
        let key = crate::video_semantic::key();
        let blob = vector(1., 512);
        db.execute_batch("INSERT INTO current_media.media(root,path,modified_unix,bytes) VALUES('C:/r','C:/r/one.mp4',10,100),('C:/r','C:/r/hidden/two.mp4',10,100);").unwrap();
        for path in ["C:/r/one.mp4", "C:/r/hidden/two.mp4"] {
            db.execute(
                "INSERT INTO video_frames VALUES('C:/r',?1,10,100,?2,2.5,?3)",
                params![path, key, blob],
            )
            .unwrap();
            db.execute(
                "INSERT INTO video_index_state VALUES('C:/r',?1,10,100,?2)",
                params![path, key],
            )
            .unwrap();
        }
        prepare_scope(&db, &["C:/r/hidden".into()]).unwrap();
        let result = video_semantic_from(&db, &["C:/r".into()], &[1.; 512], || false).unwrap();
        assert_eq!(result.evidence.len(), 1);
        assert_eq!(result.evidence[0].timestamp_seconds, Some(2.5));
        db.execute("DELETE FROM video_index_state", []).unwrap();
        assert!(
            video_semantic_from(&db, &["C:/r".into()], &[1.; 512], || false)
                .unwrap()
                .evidence
                .is_empty()
        );
    }
    #[test]
    fn exclusion_paths_use_unicode_case_folding_before_query_limits() {
        let db = db();
        db.execute_batch("INSERT INTO current_media.media(root,path,modified_unix) VALUES('C:/ФОТО','C:/ФОТО/СЕКРЕТ/one.jpg',10),('C:/ФОТО','C:/ФОТО/other.jpg',10);").unwrap();
        prepare_scope(&db, &["c:/фото/секрет".into()]).unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM visible_media", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    #[test]
    fn excluded_subfolders_are_filtered_before_content_scan_limits() {
        let db = db();
        db.execute_batch("INSERT INTO current_media.media(root,path,modified_unix) VALUES('C:/Photos','C:/Photos/Private/one.jpg',10),('C:/Photos','C:/Photos/Public/two.jpg',10),('C:/Photos','C:/Photos/Private-other/three.jpg',10);").unwrap();
        prepare_scope(&db, &["c:\\photos\\private".into()]).unwrap();
        let count: i64 = db
            .query_row("SELECT COUNT(*) FROM visible_media", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
        prepare_scope(&db, &[]).unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM visible_media", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            3
        );
    }
    #[test]
    fn ocr_scope_model_keys_cancellation_and_long_text_terms_are_preserved() {
        let db = db();
        let key = format!(
            "ppocrv4-en-rec-mobile:{}:{}",
            crate::image_decode::POLICY,
            crate::ai_models::SUPPORTED_OCR_RECOGNIZER_SHA256
        );
        for (root, path, model, text) in [
            ("other", "foreign", key.as_str(), "burger"),
            ("r", "stale", "old-model", "burger"),
            ("r", "one", key.as_str(), "BURGER at lunch"),
        ] {
            db.execute(
                "INSERT INTO ocr_index VALUES(?1,?2,10,?3,?4)",
                params![root, path, text, model],
            )
            .unwrap();
        }
        let text = "x ".repeat(1000) + " cream dessert";
        db.execute(
            "INSERT INTO ocr_index VALUES('r','two',10,?1,?2)",
            params![text, key],
        )
        .unwrap();
        db.execute_batch("INSERT INTO current_media.media(root,path,modified_unix) VALUES('r','one',10),('r','two',10);")
            .unwrap();
        let roots = vec!["r".into()];
        let batch = ocr_from(&db, &roots, &search::terms("burger cream"), || false).unwrap();
        assert_eq!(batch.evidence.len(), 2);
        assert_eq!(batch.evidence[0].path, "one");
        assert_eq!(batch.evidence[1].matched_terms, vec!["cream"]);
        assert!(batch.evidence[1].text.contains("cream"));
        assert!(ocr_from(&db, &roots, &search::terms("burger"), || true)
            .unwrap()
            .evidence
            .is_empty());
        assert!(ocr_from(&db, &[], &search::terms("burger"), || false)
            .unwrap()
            .evidence
            .is_empty());
    }
    #[test]
    fn named_people_search_uses_existing_identities_without_regrouping_and_scopes_faces_first() {
        let db = db();
        let key = format!(
            "sface-2021dec:{}:{}",
            crate::image_decode::POLICY,
            crate::ai_models::SUPPORTED_SFACE_SHA256
        );
        let blob = vector(1.0, 128);
        let face_key = crate::ai_faces::face_key(&key, &blob);
        db.execute("INSERT INTO people VALUES('person','Zoë Lee')", [])
            .unwrap();
        db.execute("INSERT INTO person_faces VALUES(?1,'person')", [face_key])
            .unwrap();
        for (root, path) in [("other", "foreign"), ("r", "local")] {
            db.execute(
                "INSERT INTO face_embeddings VALUES(?1,?2,10,?3,?4,128,0)",
                params![root, path, blob, key],
            )
            .unwrap();
        }
        db.execute_batch(
            "INSERT INTO current_media.media(root,path,modified_unix) VALUES('r','local',10);",
        )
        .unwrap();
        let batch = people_from(&db, &["r".into()], "ZOË", || false).unwrap();
        assert_eq!(batch.evidence.len(), 1);
        assert_eq!(batch.evidence[0].path, "local");
        assert_eq!(batch.evidence[0].text, "Zoë Lee");
        assert!(people_from(&db, &["r".into()], "Unknown", || false)
            .unwrap()
            .evidence
            .is_empty());
        assert_eq!(
            db.query_row("SELECT count(*) FROM people", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    #[test]
    fn semantic_candidates_are_ranked_across_allowed_roots_with_one_query_vector() {
        let db = db();
        let key = format!(
            "openclip-vit-b32-oriented-v2:{}",
            crate::ai_models::SUPPORTED_CLIP_VISION_SHA256
        );
        let same = vector(1.0, 512);
        let opposite = vector(-1.0, 512);
        for (root, path, blob, model) in [
            ("other", "foreign", &same, key.as_str()),
            ("a", "a", &opposite, key.as_str()),
            ("b", "best", &same, key.as_str()),
            ("a", "stale", &same, "old-model"),
        ] {
            db.execute(
                "INSERT INTO semantic_embeddings VALUES(?1,?2,10,?3,?4,512)",
                params![root, path, blob, model],
            )
            .unwrap();
        }
        db.execute_batch(
            "INSERT INTO current_media.media(root,path,modified_unix) VALUES('a','a',10),('b','best',10),('a','stale',10);",
        )
        .unwrap();
        let batch =
            semantic_from(&db, &["a".into(), "b".into()], &vec![1.0; 512], || false).unwrap();
        assert_eq!(batch.evidence.len(), 1);
        assert_eq!(batch.evidence[0].path, "best");
        assert!(batch.evidence[0].score > 0.99);
    }
    #[test]
    fn stale_and_deleted_content_is_excluded_before_provider_limits() {
        let db = db();
        let key = format!(
            "ppocrv4-en-rec-mobile:{}:{}",
            crate::image_decode::POLICY,
            crate::ai_models::SUPPORTED_OCR_RECOGNIZER_SHA256
        );
        db.execute(
            "INSERT INTO ocr_index VALUES('r','deleted',10,'burger',?1)",
            [&key],
        )
        .unwrap();
        db.execute(
            "INSERT INTO ocr_index VALUES('r','changed',9,'burger',?1)",
            [&key],
        )
        .unwrap();
        db.execute(
            "INSERT INTO ocr_index VALUES('r','current',10,'burger',?1)",
            [&key],
        )
        .unwrap();
        db.execute_batch(
            "INSERT INTO current_media.media(root,path,modified_unix) VALUES('r','changed',10),('r','current',10);",
        )
        .unwrap();
        let batch = ocr_from(&db, &["r".into()], &search::terms("burger"), || false).unwrap();
        assert_eq!(batch.evidence.len(), 1);
        assert_eq!(batch.evidence[0].path, "current");
    }
    #[test]
    fn current_index_attachment_handles_unicode_and_uri_characters_and_is_read_only() {
        let path = std::env::temp_dir().join(format!(
            "search index 世界 #%-{}.sqlite",
            crate::video_project::id()
        ));
        {
            let index = Connection::open(&path).unwrap();
            index.execute_batch("CREATE TABLE media(root TEXT,path TEXT,modified_unix INTEGER);INSERT INTO media VALUES('r','one',10);").unwrap();
        }
        let db = Connection::open_in_memory().unwrap();
        attach_current(&db, &path).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM current_media.media", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert!(db.execute("DELETE FROM current_media.media", []).is_err());
        drop(db);
        std::fs::remove_file(path).unwrap();
    }
}
