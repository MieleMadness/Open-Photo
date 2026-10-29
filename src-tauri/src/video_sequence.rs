//! Multi-asset documents share durable project storage, but never overwrite v1 drafts.
use crate::{
    video_engine, video_probe,
    video_project::{self, Asset, Clip, Output, Project, VideoTime},
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_DOCUMENT: usize = 1024 * 1024;
const MAX_ASSETS: usize = 50;
const MAX_CLIPS: usize = 200;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Sequence {
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub assets: Vec<Asset>,
    pub clips: Vec<Clip>,
    pub output: Output,
    pub frame_rate: u32,
}
#[derive(Serialize)]
pub struct SourceStatus {
    pub asset_id: String,
    pub path: Option<String>,
    pub status: String,
}
#[derive(Serialize)]
pub struct Loaded {
    pub project: Sequence,
    pub revision: u64,
    pub sources: Vec<SourceStatus>,
}

fn default_clip(asset: &Asset) -> Result<Clip, String> {
    let base = asset.info.time_base.as_deref().unwrap_or("1/1000000");
    Ok(Clip {
        id: video_project::id(),
        asset_id: asset.id.clone(),
        source_in: VideoTime::from_seconds(0.0, base)?,
        source_out: VideoTime::from_seconds(asset.info.duration_seconds, base)?,
        edits: Default::default(),
    })
}
fn single(sequence: &Sequence, asset: &Asset, clip: Clip) -> Project {
    Project {
        schema_version: 1,
        id: sequence.id.clone(),
        title: sequence.title.clone(),
        asset: asset.clone(),
        clips: vec![clip],
        output: sequence.output.clone(),
    }
}
pub fn validate(p: &Sequence) -> Result<(), String> {
    if p.schema_version != 2 {
        return Err("Unsupported multi-video project version".into());
    }
    if p.assets.is_empty()
        || p.assets.len() > MAX_ASSETS
        || p.clips.len() > MAX_CLIPS
        || !matches!(p.frame_rate, 30 | 60)
    {
        return Err(
            "A project needs 1–50 videos, at most 200 clips, and a 30 or 60 fps canvas".into(),
        );
    }
    let mut asset_ids = HashSet::new();
    let mut identities = HashSet::new();
    for asset in &p.assets {
        if !asset_ids.insert(&asset.id)
            || !identities.insert((asset.bytes, asset.sha256.to_ascii_lowercase()))
        {
            return Err("Duplicate project video identity".into());
        }
        video_project::validate(&single(p, asset, default_clip(asset)?))?;
    }
    let mut clip_ids = HashSet::new();
    let mut duration = 0.0;
    for clip in &p.clips {
        let asset = p
            .assets
            .iter()
            .find(|a| a.id == clip.asset_id)
            .ok_or("Clip refers to a missing video")?;
        if !clip_ids.insert(&clip.id) {
            return Err("Duplicate timeline clip identity".into());
        }
        video_project::validate(&single(p, asset, clip.clone()))?;
        duration += clip.source_out.seconds()? - clip.source_in.seconds()?;
    }
    if duration > 7200.0 {
        return Err("Project duration exceeds two hours".into());
    }
    if serde_json::to_vec(p).map_err(|e| e.to_string())?.len() > MAX_DOCUMENT {
        return Err("Project file too large".into());
    }
    Ok(())
}
pub(crate) fn migrate(p: Project) -> Result<Sequence, String> {
    video_project::validate(&p)?;
    let next = Sequence {
        schema_version: 2,
        id: p.id,
        title: p.title,
        assets: vec![p.asset],
        clips: p.clips,
        output: p.output,
        frame_rate: 30,
    };
    validate(&next)?;
    Ok(next)
}
fn parse(bytes: &[u8]) -> Result<Sequence, String> {
    if bytes.len() > MAX_DOCUMENT {
        return Err("Project file too large".into());
    }
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let p = match value.get("schema_version").and_then(|v| v.as_u64()) {
        Some(1) => migrate(serde_json::from_value(value).map_err(|e| e.to_string())?)?,
        Some(2) => serde_json::from_value(value).map_err(|e| e.to_string())?,
        _ => return Err("Unsupported video project version".into()),
    };
    validate(&p)?;
    Ok(p)
}
fn root_key(root: &str) -> Result<String, String> {
    Ok(fs::canonicalize(root)
        .map_err(|_| "Project folder is unavailable")?
        .to_string_lossy()
        .into_owned())
}
fn initialize(db: &Connection) -> Result<(), String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS video_sequences(id TEXT PRIMARY KEY,root TEXT NOT NULL,title TEXT NOT NULL,revision INTEGER NOT NULL,data TEXT NOT NULL,updated INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS video_sequence_history(id INTEGER PRIMARY KEY,project_id TEXT NOT NULL,revision INTEGER NOT NULL,data TEXT NOT NULL); CREATE TRIGGER IF NOT EXISTS sequence_backup BEFORE UPDATE ON video_sequences BEGIN INSERT INTO video_sequence_history(project_id,revision,data) VALUES(old.id,old.revision,old.data); DELETE FROM video_sequence_history WHERE project_id=old.id AND id NOT IN (SELECT id FROM video_sequence_history WHERE project_id=old.id ORDER BY id DESC LIMIT 20); END;").map_err(|e|e.to_string())
}
fn database() -> Result<Connection, String> {
    let db = video_project::database()?;
    initialize(&db)?;
    Ok(db)
}
fn store(db: &Connection, root: &str, p: &Sequence, expected: Option<u64>) -> Result<u64, String> {
    validate(p)?;
    let root = root_key(root)?;
    let data = serde_json::to_string(p).map_err(|e| e.to_string())?;
    match expected {
        None => {
            db.execute(
                "INSERT INTO video_sequences VALUES(?1,?2,?3,1,?4,unixepoch())",
                params![p.id, root, p.title, data],
            )
            .map_err(|e| e.to_string())?;
            Ok(1)
        }
        Some(revision) => {
            if db.execute("UPDATE video_sequences SET title=?1,revision=revision+1,data=?2,updated=unixepoch() WHERE id=?3 AND root=?4 AND revision=?5",params![p.title,data,p.id,root,revision]).map_err(|e|e.to_string())?!=1 {return Err("Project changed elsewhere. Reopen it before editing further.".into());}
            Ok(revision + 1)
        }
    }
}
fn read(db: &Connection, root: &str, id: &str) -> Result<(Sequence, u64), String> {
    let row: Option<(String, u64)> = db
        .query_row(
            "SELECT data,revision FROM video_sequences WHERE id=?1 AND root=?2",
            params![id, root_key(root)?],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let (data, revision) = row.ok_or("Multi-video project not found in this folder")?;
    Ok((parse(data.as_bytes())?, revision))
}
fn source(root: &str, p: &Sequence, asset: &Asset) -> Result<PathBuf, String> {
    video_project::source(root, &single(p, asset, default_clip(asset)?), || false)
}
fn loaded(root: &str, p: Sequence, revision: u64) -> Loaded {
    let sources = p
        .assets
        .iter()
        .map(|asset| match source(root, &p, asset) {
            Ok(path) => SourceStatus {
                asset_id: asset.id.clone(),
                path: Some(path.to_string_lossy().into_owned()),
                status: "ready".into(),
            },
            Err(status) => SourceStatus {
                asset_id: asset.id.clone(),
                path: None,
                status,
            },
        })
        .collect();
    Loaded {
        project: p,
        revision,
        sources,
    }
}
fn add_files(engine: &Path, root: &str, p: &mut Sequence, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() || paths.len() > MAX_ASSETS {
        return Err("Choose 1–50 video files".into());
    }
    // Build a candidate first: a failed batch must not partially modify a saved project.
    let mut candidate = p.clone();
    for path in paths {
        let file = crate::media_stream::validated_video(root, path)?;
        let info = video_probe::probe(engine, &file, || false)?;
        let legacy = video_project::new_project(root, &file, info)?;
        if candidate
            .assets
            .iter()
            .any(|a| a.bytes == legacy.asset.bytes && a.sha256 == legacy.asset.sha256)
        {
            continue;
        }
        if candidate.assets.len() == MAX_ASSETS {
            return Err("A project can contain up to 50 videos".into());
        }
        candidate.clips.push(legacy.clips[0].clone());
        candidate.assets.push(legacy.asset);
    }
    validate(&candidate)?;
    *p = candidate;
    Ok(())
}
#[tauri::command]
pub async fn create_video_sequence(
    app: tauri::AppHandle,
    root: String,
    paths: Vec<String>,
) -> Result<Loaded, String> {
    let engine = video_engine::directory(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut p = Sequence {
            schema_version: 2,
            id: video_project::id(),
            title: "New video project".into(),
            assets: vec![],
            clips: vec![],
            output: Output {
                width: 1920,
                height: 1080,
                fit: "fit".into(),
            },
            frame_rate: 30,
        };
        add_files(&engine, &root, &mut p, &paths)?;
        let revision = store(&database()?, &root, &p, None)?;
        Ok(loaded(&root, p, revision))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn add_sequence_videos(
    app: tauri::AppHandle,
    root: String,
    id: String,
    paths: Vec<String>,
    expected_revision: u64,
) -> Result<Loaded, String> {
    let engine = video_engine::directory(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let db = database()?;
        let (mut p, revision) = read(&db, &root, &id)?;
        if revision != expected_revision {
            return Err("Project changed elsewhere. Reopen it before adding videos.".into());
        }
        add_files(&engine, &root, &mut p, &paths)?;
        let revision = store(&db, &root, &p, Some(revision))?;
        Ok(loaded(&root, p, revision))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn save_video_sequence(
    root: String,
    project: Sequence,
    expected_revision: u64,
) -> Result<u64, String> {
    tauri::async_runtime::spawn_blocking(move || {
        validate(&project)?;
        let db = database()?;
        let (before, _) = read(&db, &root, &project.id)?;
        if before.assets != project.assets {
            return Err(
                "Change project sources through Add videos, Remove video, or Relink".into(),
            );
        }
        store(&db, &root, &project, Some(expected_revision))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn load_video_sequence(root: String, id: String) -> Result<Loaded, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (p, r) = read(&database()?, &root, &id)?;
        Ok(loaded(&root, p, r))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn recent_video_sequences(
    root: String,
) -> Result<Vec<video_project::RecentProject>, String> {
    tauri::async_runtime::spawn_blocking(move||{
        let db=database()?;let mut q=db.prepare("SELECT id,title FROM video_sequences WHERE root=?1 ORDER BY updated DESC,rowid DESC LIMIT 30").map_err(|e|e.to_string())?;
        let rows=q.query_map([root_key(&root)?],|r|Ok(video_project::RecentProject{id:r.get(0)?,title:r.get(1)?})).map_err(|e|e.to_string())?;
        rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
    }).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub async fn import_video_sequence(root: String, path: String) -> Result<Loaded, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut bytes = vec![];
        fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take((MAX_DOCUMENT + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let mut p = parse(&bytes)?;
        p.id = video_project::id();
        let r = store(&database()?, &root, &p, None)?;
        Ok(loaded(&root, p, r))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn write_video_sequence_copy(
    project: Sequence,
    destination: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || write_copy(&project, Path::new(&destination)))
        .await
        .map_err(|e| e.to_string())?
}
fn write_copy(project: &Sequence, destination: &Path) -> Result<(), String> {
    validate(project)?;
    let bytes = serde_json::to_vec_pretty(project).map_err(|e| e.to_string())?;
    let mut staged = crate::safe_output::StagedOutput::new(destination)?;
    staged.file().write_all(&bytes).map_err(|e| e.to_string())?;
    staged.commit()?;
    Ok(())
}

#[tauri::command]
pub async fn remove_sequence_video(
    root: String,
    id: String,
    asset_id: String,
    expected_revision: u64,
) -> Result<Loaded, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = database()?;
        let (mut p, revision) = read(&db, &root, &id)?;
        if revision != expected_revision {
            return Err("Project changed elsewhere. Reopen it before removing videos.".into());
        }
        if !p.assets.iter().any(|a| a.id == asset_id) {
            return Err("Video not found in this project".into());
        }
        p.assets.retain(|a| a.id != asset_id);
        p.clips.retain(|c| c.asset_id != asset_id);
        let r = store(&db, &root, &p, Some(revision))?;
        Ok(loaded(&root, p, r))
    })
    .await
    .map_err(|e| e.to_string())?
}
fn relink(p: &mut Sequence, root: &str, asset_id: &str, path: &str) -> Result<(), String> {
    let file = crate::media_stream::validated_video(root, path)?;
    let asset = p
        .assets
        .iter_mut()
        .find(|a| a.id == asset_id)
        .ok_or("Video not found in this project")?;
    let (bytes, sha) = video_project::fingerprint(&file, || false)?;
    if bytes != asset.bytes || sha != asset.sha256 {
        return Err("Choose the original video with identical contents".into());
    }
    asset.relative_path = file
        .strip_prefix(Path::new(&root_key(root)?))
        .map_err(|e| e.to_string())?
        .to_str()
        .ok_or("Invalid video path")?
        .replace('\\', "/");
    Ok(())
}
#[tauri::command]
pub async fn relink_sequence_video(
    root: String,
    id: String,
    asset_id: String,
    path: String,
    expected_revision: u64,
) -> Result<Loaded, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = database()?;
        let (mut p, revision) = read(&db, &root, &id)?;
        if revision != expected_revision {
            return Err("Project changed elsewhere. Reopen it before relinking.".into());
        }
        relink(&mut p, &root, &asset_id, &path)?;
        let r = store(&db, &root, &p, Some(revision))?;
        Ok(loaded(&root, p, r))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, Sequence) {
        let root = std::env::temp_dir().join(format!("sequence-{}", video_project::id()));
        fs::create_dir(&root).unwrap();
        let file = root.join("source.mp4");
        fs::write(&file, b"unchanged original").unwrap();
        let info=video_probe::parse_probe(br#"{"streams":[{"index":0,"codec_type":"video","codec_name":"h264","width":96,"height":64,"time_base":"1/90000"}],"format":{"duration":"2"}}"#).unwrap();
        let legacy = video_project::new_project(root.to_str().unwrap(), &file, info).unwrap();
        (root.clone(), migrate(legacy).unwrap())
    }
    #[cfg(windows)]
    #[test]
    fn short_media_batch_import_deduplicates_bytes_and_rolls_back_failed_batches() {
        use std::{ffi::OsString, time::Duration};
        let (root, mut p) = fixture();
        p.assets.clear();
        p.clips.clear();
        let engine = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/video-engine");
        let executable = video_engine::tool(&engine, "ffmpeg").unwrap();
        let mut paths = vec![];
        for (name, color) in [
            ("one space.mp4", "red"),
            ("two.mp4", "blue"),
            ("three.mp4", "green"),
        ] {
            let path = root.join(name);
            let mut args: Vec<OsString> = [
                "-nostdin",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                &format!("color={color}:size=96x64:rate=30:duration=0.3"),
                "-an",
                "-c:v",
                "libopenh264",
                "-b:v",
                "100000",
            ]
            .into_iter()
            .map(Into::into)
            .collect();
            args.push(path.clone().into());
            video_engine::run(
                &executable,
                &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                Duration::from_secs(30),
            )
            .unwrap();
            paths.push(path);
        }
        let duplicate = root.join("same content.mp4");
        fs::copy(&paths[0], &duplicate).unwrap();
        let identity = video_project::fingerprint(&paths[0], || false).unwrap();
        add_files(
            &engine,
            root.to_str().unwrap(),
            &mut p,
            &[
                paths[0].to_string_lossy().into(),
                duplicate.to_string_lossy().into(),
                paths[1].to_string_lossy().into(),
            ],
        )
        .unwrap();
        assert_eq!(p.assets.len(), 2);
        assert_eq!(p.clips.len(), 2);
        let before = p.clone();
        assert!(add_files(
            &engine,
            root.to_str().unwrap(),
            &mut p,
            &[
                paths[2].to_string_lossy().into(),
                root.join("source.mp4").to_string_lossy().into()
            ]
        )
        .is_err());
        assert_eq!(p, before);
        let mut repeated = p.clips[0].clone();
        repeated.id = video_project::id();
        p.clips.push(repeated);
        validate(&p).unwrap();
        assert_eq!(
            video_project::fingerprint(&paths[0], || false).unwrap(),
            identity
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn portable_copy_relink_and_existing_destinations_preserve_originals() {
        let (root, mut p) = fixture();
        let key = root.to_str().unwrap();
        let identity = p.assets[0].clone();
        let path = root.join("project.json");
        write_copy(&p, &path).unwrap();
        assert_eq!(parse(&fs::read(&path).unwrap()).unwrap(), p);
        let original_json = fs::read(&path).unwrap();
        assert!(write_copy(&p, &path).is_err());
        assert_eq!(fs::read(&path).unwrap(), original_json);
        fs::write(root.join("different.mp4"), b"different original").unwrap();
        let asset_id = p.assets[0].id.clone();
        assert!(relink(
            &mut p,
            key,
            &asset_id,
            root.join("different.mp4").to_str().unwrap()
        )
        .is_err());
        assert_eq!(p.assets[0], identity);
        fs::rename(root.join("source.mp4"), root.join("moved.mp4")).unwrap();
        relink(
            &mut p,
            key,
            &asset_id,
            root.join("moved.mp4").to_str().unwrap(),
        )
        .unwrap();
        assert_eq!(p.assets[0].sha256, identity.sha256);
        assert_eq!(p.assets[0].id, identity.id);
        assert!(source(key, &p, &p.assets[0]).is_ok());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn migration_preserves_legacy_edits_ticks_and_source() {
        let (root, mut p) = fixture();
        p.clips[0].source_in = VideoTime::from_seconds(0.37, "1/90000").unwrap();
        p.clips[0].edits.brightness = 19;
        let legacy = single(&p, &p.assets[0], p.clips[0].clone());
        let bytes = serde_json::to_vec(&legacy).unwrap();
        let restored = parse(&bytes).unwrap();
        assert_eq!(restored, p);
        assert_eq!(serde_json::to_vec(&legacy).unwrap(), bytes);
        assert!(source(root.to_str().unwrap(), &restored, &restored.assets[0]).is_ok());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn empty_timeline_roundtrips_without_discarding_or_bypassing_source_validation() {
        let (root, mut p) = fixture();
        let original = p.clone();
        let db = Connection::open_in_memory().unwrap();
        initialize(&db).unwrap();
        let key = root.to_str().unwrap();
        store(&db, key, &p, None).unwrap();
        p.clips.clear();
        store(&db, key, &p, Some(1)).unwrap();
        assert_eq!(read(&db, key, &p.id).unwrap().0, p);
        assert_eq!(parse(&serde_json::to_vec(&p).unwrap()).unwrap(), p);
        assert_eq!(p.assets, original.assets);
        assert_eq!(
            fs::read(root.join("source.mp4")).unwrap(),
            b"unchanged original"
        );
        p.assets[0].relative_path = "../escape.mp4".into();
        assert!(validate(&p).is_err());
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_broken_references_duplicate_ids_and_untrusted_versions() {
        let (root, p) = fixture();
        let mut bad = p.clone();
        bad.clips[0].asset_id = "missing".into();
        assert!(validate(&bad).is_err());
        let mut bad = p.clone();
        bad.assets.push(bad.assets[0].clone());
        assert!(validate(&bad).is_err());
        let mut bad = p.clone();
        bad.clips.push(bad.clips[0].clone());
        assert!(validate(&bad).is_err());
        let mut bad = p.clone();
        bad.assets[0].relative_path = "../escape.mp4".into();
        assert!(validate(&bad).is_err());
        let mut bad = p;
        bad.schema_version = 3;
        assert!(parse(&serde_json::to_vec(&bad).unwrap()).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn durable_multi_asset_roundtrip_scopes_roots_and_rejects_stale_writers() {
        let (root, mut p) = fixture();
        let second = root.join("second.mp4");
        fs::write(&second, b"second unchanged original").unwrap();
        let second =
            video_project::new_project(root.to_str().unwrap(), &second, p.assets[0].info.clone())
                .unwrap();
        p.assets.push(second.asset);
        p.clips.push(second.clips[0].clone());
        let db = Connection::open(root.join("test.sqlite")).unwrap();
        initialize(&db).unwrap();
        let key = root.to_str().unwrap();
        assert_eq!(store(&db, key, &p, None).unwrap(), 1);
        assert_eq!(read(&db, key, &p.id).unwrap().0, p);
        p.title = "Saved multi-video project".into();
        assert_eq!(store(&db, key, &p, Some(1)).unwrap(), 2);
        assert!(store(&db, key, &p, Some(1)).is_err());
        let other = root.join("other");
        fs::create_dir(&other).unwrap();
        assert!(read(&db, other.to_str().unwrap(), &p.id).is_err());
        let result = loaded(key, p.clone(), 2);
        assert!(result.sources.iter().all(|s| s.path.is_some()));
        fs::rename(root.join("second.mp4"), root.join("moved.mp4")).unwrap();
        let result = loaded(key, p, 2);
        assert!(result.sources[0].path.is_some());
        assert!(result.sources[1].path.is_none());
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }
}
