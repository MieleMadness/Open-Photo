//! Versioned single-source projects. SQLite drafts are independent of source files and caches.
use crate::video_probe::{self, VideoInfo};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const MAX_DOCUMENT: usize = 1024 * 1024;
pub fn id() -> String {
    format!(
        "{:x}-{:x}-{:x}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VideoTime {
    pub ticks: String,
    pub time_base: String,
}
impl VideoTime {
    pub fn seconds(&self) -> Result<f64, String> {
        let ticks = self
            .ticks
            .parse::<u64>()
            .map_err(|_| "Invalid project timestamp")?;
        let (a, b) = self
            .time_base
            .split_once('/')
            .ok_or("Invalid project time base")?;
        let a = a.parse::<u32>().map_err(|_| "Invalid project time base")?;
        let b = b.parse::<u32>().map_err(|_| "Invalid project time base")?;
        if a == 0 || b == 0 || ticks > 9_007_199_254_740_991 {
            return Err("Invalid project timestamp".into());
        }
        Ok(ticks as f64 * f64::from(a) / f64::from(b))
    }
    pub fn from_seconds(seconds: f64, base: &str) -> Result<Self, String> {
        let unit = Self {
            ticks: "1".into(),
            time_base: base.into(),
        }
        .seconds()?;
        Ok(Self {
            ticks: format!("{:.0}", (seconds / unit).round()),
            time_base: base.into(),
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Crop {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Edits {
    pub crop: Crop,
    pub quarter_turns: u8,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
    pub muted: bool,
    pub gain_percent: u16,
}
impl Default for Edits {
    fn default() -> Self {
        Self {
            crop: Crop {
                x: 0,
                y: 0,
                width: 10000,
                height: 10000,
            },
            quarter_turns: 0,
            flip_horizontal: false,
            flip_vertical: false,
            muted: false,
            gain_percent: 100,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub id: String,
    pub relative_path: String,
    pub sha256: String,
    pub bytes: u64,
    pub info: VideoInfo,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    pub id: String,
    pub asset_id: String,
    pub source_in: VideoTime,
    pub source_out: VideoTime,
    pub edits: Edits,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub width: u32,
    pub height: u32,
    pub fit: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub asset: Asset,
    pub clips: Vec<Clip>,
    pub output: Output,
}
#[derive(Serialize)]
pub struct LoadedProject {
    pub project: Project,
    pub revision: u64,
    pub source_path: Option<String>,
    pub source_status: String,
}
#[derive(Serialize)]
pub struct RecentProject {
    pub id: String,
    pub title: String,
}

fn valid_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 120 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}
fn relative_path(s: &str) -> bool {
    !s.is_empty()
        && s.len() < 4096
        && !s.contains(['\\', ':', '\0'])
        && Path::new(s)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        && s.split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}
pub fn validate(p: &Project) -> Result<(), String> {
    if p.schema_version != 1 {
        return Err("Unsupported video project version".into());
    }
    if !valid_id(&p.id)
        || !valid_id(&p.asset.id)
        || p.title.trim().is_empty()
        || p.title.chars().count() > 200
        || !relative_path(&p.asset.relative_path)
        || p.asset.sha256.len() != 64
        || !p.asset.sha256.chars().all(|c| c.is_ascii_hexdigit())
        || p.asset.bytes > 9_007_199_254_740_991
    {
        return Err("Invalid video project or asset".into());
    }
    let info = &p.asset.info;
    if !info.duration_seconds.is_finite()
        || info.duration_seconds <= 0.0
        || info.duration_seconds > 7200.0
        || info.width < 2
        || info.height < 2
        || u64::from(info.width) * u64::from(info.height) > 100_000_000
    {
        return Err("Unsupported project source dimensions or duration".into());
    }
    if p.clips.len() != 1 {
        return Err("This release supports one clip per project".into());
    }
    let c = &p.clips[0];
    let base = info.time_base.as_deref().unwrap_or("1/1000000");
    let tick = VideoTime {
        ticks: "1".into(),
        time_base: base.into(),
    }
    .seconds()?;
    let start = c.source_in.seconds()?;
    let end = c.source_out.seconds()?;
    if !valid_id(&c.id)
        || c.asset_id != p.asset.id
        || c.source_in.time_base != c.source_out.time_base
        || c.source_in.time_base != base
        || end <= start
        || end - start < 0.04
        || end > info.duration_seconds + tick / 2.0 + 0.000001
    {
        return Err("Invalid project trim range (minimum 0.04 seconds)".into());
    }
    let e = &c.edits;
    let crop = &e.crop;
    if e.quarter_turns > 3
        || e.gain_percent > 200
        || crop.width < 100
        || crop.height < 100
        || crop.x > 10000
        || crop.y > 10000
        || crop.width > 10000 - crop.x
        || crop.height > 10000 - crop.y
    {
        return Err("Invalid crop, rotation, or audio gain".into());
    }
    let o = &p.output;
    if o.width < 16
        || o.height < 16
        || o.width > 4096
        || o.height > 4096
        || !o.width.is_multiple_of(2)
        || !o.height.is_multiple_of(2)
        || u64::from(o.width) * u64::from(o.height) > 16_000_000
        || o.width > o.height * 32
        || o.height > o.width * 32
        || !matches!(o.fit.as_str(), "fit" | "fill")
    {
        return Err("Invalid output canvas".into());
    }
    if serde_json::to_vec(p).map_err(|e| e.to_string())?.len() > MAX_DOCUMENT {
        return Err("Project document too large".into());
    }
    Ok(())
}
pub fn fingerprint(path: &Path, cancel: impl Fn() -> bool) -> Result<(u64, String), String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let before = file.metadata().map_err(|e| e.to_string())?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        if cancel() {
            return Err("Video project operation cancelled".into());
        }
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    let after = fs::metadata(path).map_err(|e| e.to_string())?;
    if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
        return Err("Source changed while being verified".into());
    }
    Ok((before.len(), format!("{:x}", digest.finalize())))
}
fn root_path(root: &str) -> Result<PathBuf, String> {
    fs::canonicalize(root).map_err(|_| "Library folder unavailable".into())
}
pub fn source(root: &str, p: &Project, cancel: impl Fn() -> bool) -> Result<PathBuf, String> {
    validate(p)?;
    let root = root_path(root)?;
    let candidate = root.join(&p.asset.relative_path);
    let file = crate::media_stream::validated_video(
        root.to_str().ok_or("Invalid library path")?,
        candidate.to_str().ok_or("Invalid source path")?,
    )?;
    let (bytes, sha) = fingerprint(&file, cancel)?;
    if bytes != p.asset.bytes || sha != p.asset.sha256 {
        return Err("Source differs from this project's fingerprint. Relink the original file or create a new project.".into());
    }
    Ok(file)
}
pub(crate) fn new_project(root: &str, file: &Path, info: VideoInfo) -> Result<Project, String> {
    let file = fs::canonicalize(file).map_err(|e| e.to_string())?;
    let (bytes, sha256) = fingerprint(&file, || false)?;
    let root = root_path(root)?;
    let relative = file
        .strip_prefix(root)
        .map_err(|_| "Source is outside this library")?
        .to_str()
        .ok_or("Invalid source path")?
        .replace('\\', "/");
    let base = info.time_base.as_deref().unwrap_or("1/1000000");
    let asset_id = id();
    let portrait = (info.height > info.width)
        ^ ((info.rotation_degrees / 90.0).round() as i64)
            .rem_euclid(2)
            .eq(&1);
    let project = Project {
        schema_version: 1,
        id: id(),
        title: file
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .chars()
            .take(80)
            .collect(),
        clips: vec![Clip {
            id: id(),
            asset_id: asset_id.clone(),
            source_in: VideoTime::from_seconds(0.0, base)?,
            source_out: VideoTime::from_seconds(info.duration_seconds, base)?,
            edits: Edits::default(),
        }],
        asset: Asset {
            id: asset_id,
            relative_path: relative,
            sha256,
            bytes,
            info,
        },
        output: Output {
            width: if portrait { 1080 } else { 1920 },
            height: if portrait { 1920 } else { 1080 },
            fit: "fit".into(),
        },
    };
    validate(&project)?;
    Ok(project)
}
fn initialize(db: &Connection) -> Result<(), String> {
    db.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    db.execute_batch("PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS projects(id TEXT PRIMARY KEY,root TEXT NOT NULL,title TEXT NOT NULL,revision INTEGER NOT NULL,data TEXT NOT NULL,updated INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS project_history(id INTEGER PRIMARY KEY,project_id TEXT NOT NULL,data TEXT NOT NULL); CREATE TRIGGER IF NOT EXISTS project_backup BEFORE UPDATE ON projects BEGIN INSERT INTO project_history(project_id,data) VALUES(old.id,old.data); DELETE FROM project_history WHERE project_id=old.id AND id NOT IN (SELECT id FROM project_history WHERE project_id=old.id ORDER BY id DESC LIMIT 20); END;").map_err(|e|e.to_string())
}
fn database() -> Result<Connection, String> {
    let dir = dirs::data_local_dir()
        .ok_or("Local project storage unavailable")?
        .join("Open-Photo");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let db = Connection::open(dir.join("video-projects.sqlite")).map_err(|e| e.to_string())?;
    initialize(&db)?;
    Ok(db)
}
fn root_key(root: &str) -> Result<String, String> {
    Ok(root_path(root)?.to_string_lossy().into_owned())
}
fn store(db: &Connection, root: &str, p: &Project, expected: Option<u64>) -> Result<u64, String> {
    validate(p)?;
    let data = serde_json::to_string(p).map_err(|e| e.to_string())?;
    let root = root_key(root)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    match expected {
        None => {
            db.execute("INSERT INTO projects(id,root,title,revision,data,updated) VALUES(?1,?2,?3,1,?4,?5)",params![p.id,root,p.title,data,now]).map_err(|e|e.to_string())?;
            Ok(1)
        }
        Some(expected) => {
            let changed=db.execute("UPDATE projects SET title=?1,data=?2,revision=revision+1,updated=?3 WHERE id=?4 AND root=?5 AND revision=?6",params![p.title,data,now,p.id,root,expected]).map_err(|e|e.to_string())?;
            if changed != 1 {
                return Err("Project was changed elsewhere. Reopen the saved project before editing further.".into());
            }
            Ok(expected + 1)
        }
    }
}
fn read(db: &Connection, root: &str, id: &str) -> Result<(Project, u64), String> {
    let row: Option<(String, u64)> = db
        .query_row(
            "SELECT data,revision FROM projects WHERE id=?1 AND root=?2",
            params![id, root_key(root)?],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let (data, rev) = row.ok_or("Saved video project not found in this library")?;
    let p: Project = serde_json::from_str(&data).map_err(|e| e.to_string())?;
    validate(&p)?;
    Ok((p, rev))
}
fn loaded(root: &str, p: Project, revision: u64) -> LoadedProject {
    match source(root, &p, || false) {
        Ok(path) => LoadedProject {
            project: p,
            revision,
            source_path: Some(path.to_string_lossy().into_owned()),
            source_status: "ready".into(),
        },
        Err(error) => LoadedProject {
            project: p,
            revision,
            source_path: None,
            source_status: error,
        },
    }
}
#[tauri::command]
pub async fn create_video_project(
    app: tauri::AppHandle,
    root: String,
    path: String,
) -> Result<LoadedProject, String> {
    let engine = crate::video_engine::directory(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let file = crate::media_stream::validated_video(&root, &path)?;
        let info = video_probe::probe(&engine, &file, || false)?;
        let p = new_project(&root, &file, info)?;
        let revision = store(&database()?, &root, &p, None)?;
        Ok(LoadedProject {
            project: p,
            revision,
            source_path: Some(file.to_string_lossy().into_owned()),
            source_status: "ready".into(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn save_video_project(
    root: String,
    project: Project,
    expected_revision: u64,
) -> Result<u64, String> {
    tauri::async_runtime::spawn_blocking(move || {
        validate(&project)?;
        let db = database()?;
        let (previous, _) = read(&db, &root, &project.id)?;
        if previous.asset != project.asset
            || previous.clips[0].id != project.clips[0].id
            || previous.clips[0].asset_id != project.clips[0].asset_id
        {
            return Err("Source identity must be changed through explicit relinking".into());
        }
        store(&db, &root, &project, Some(expected_revision))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn load_video_project(root: String, id: String) -> Result<LoadedProject, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (p, revision) = read(&database()?, &root, &id)?;
        Ok(loaded(&root, p, revision))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn recent_video_projects(root: String) -> Result<Vec<RecentProject>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = database()?;
        let mut q = db
        .prepare(
            "SELECT id,title FROM projects WHERE root=?1 ORDER BY updated DESC,rowid DESC LIMIT 30",
        )
        .map_err(|e| e.to_string())?;
        let rows = q
            .query_map([root_key(&root)?], |r| {
                Ok(RecentProject {
                    id: r.get(0)?,
                    title: r.get(1)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn import_video_project(root: String, path: String) -> Result<LoadedProject, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let file = fs::File::open(path).map_err(|e| e.to_string())?;
        let mut bytes = vec![];
        file.take((MAX_DOCUMENT + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > MAX_DOCUMENT {
            return Err("Project file too large".into());
        }
        let mut p: Project =
            serde_json::from_slice(&bytes).map_err(|e| format!("Invalid project file: {e}"))?;
        validate(&p)?;
        p.id = id();
        let revision = store(&database()?, &root, &p, None)?;
        Ok(loaded(&root, p, revision))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn write_video_project_copy(project: Project, destination: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || write_copy(&project, Path::new(&destination)))
        .await
        .map_err(|e| e.to_string())?
}
pub(crate) fn write_copy(project: &Project, destination: &Path) -> Result<(), String> {
    validate(project)?;
    let bytes = serde_json::to_vec_pretty(project).map_err(|e| e.to_string())?;
    let mut staged = crate::safe_output::StagedOutput::new(destination)?;
    staged.file().write_all(&bytes).map_err(|e| e.to_string())?;
    staged.commit()?;
    Ok(())
}
#[tauri::command]
pub async fn relink_video_project(
    root: String,
    id: String,
    path: String,
    expected_revision: u64,
) -> Result<LoadedProject, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = database()?;
        let (mut p, revision) = read(&db, &root, &id)?;
        if revision != expected_revision {
            return Err("Project changed elsewhere; reopen it before relinking".into());
        }
        let file = crate::media_stream::validated_video(&root, &path)?;
        let (bytes, sha) = fingerprint(&file, || false)?;
        if bytes != p.asset.bytes || sha != p.asset.sha256 {
            return Err("That file is different from the original project source".into());
        }
        p.asset.relative_path = file
            .strip_prefix(root_path(&root)?)
            .map_err(|e| e.to_string())?
            .to_str()
            .ok_or("Invalid source path")?
            .replace('\\', "/");
        let revision = store(&db, &root, &p, Some(revision))?;
        Ok(LoadedProject {
            project: p,
            revision,
            source_path: Some(file.to_string_lossy().into_owned()),
            source_status: "ready".into(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Shared, deterministic edit graph, ready for the later export pipeline.
pub fn filters(p: &Project, preview: bool) -> Result<(String, u32, u32), String> {
    validate(p)?;
    let e = &p.clips[0].edits;
    let c = &e.crop;
    let mut f=vec!["scale=w='max(2,trunc(iw*sar/2)*2)':h='max(2,trunc(ih/2)*2)':reset_sar=1".to_string(),format!("crop=w='max(2,trunc(iw*{}/10000/2)*2)':h='max(2,trunc(ih*{}/10000/2)*2)':x='trunc(iw*{}/10000/2)*2':y='trunc(ih*{}/10000/2)*2'",c.width,c.height,c.x,c.y)];
    match e.quarter_turns {
        1 => f.push("transpose=clock".into()),
        2 => f.extend(["hflip".into(), "vflip".into()]),
        3 => f.push("transpose=cclock".into()),
        _ => {}
    }
    if e.flip_horizontal {
        f.push("hflip".into());
    }
    if e.flip_vertical {
        f.push("vflip".into());
    }
    let ratio = if preview {
        (960.0 / f64::from(p.output.width))
            .min(540.0 / f64::from(p.output.height))
            .min(1.0)
    } else {
        1.0
    };
    let width = ((f64::from(p.output.width) * ratio / 2.0).floor() as u32 * 2).max(2);
    let height = ((f64::from(p.output.height) * ratio / 2.0).floor() as u32 * 2).max(2);
    if p.output.fit == "fit" {
        f.push(format!("scale={width}:{height}:force_original_aspect_ratio=decrease:force_divisible_by=2,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:black"));
    } else {
        f.push(format!("scale={width}:{height}:force_original_aspect_ratio=increase:force_divisible_by=2,crop={width}:{height}"));
    }
    f.push("setsar=1".into());
    Ok((f.join(","), width, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, Project) {
        let root = std::env::temp_dir().join(format!("openphoto-project-{}", id()));
        fs::create_dir(&root).unwrap();
        let file = root.join("original.mp4");
        fs::write(&file, b"original bytes").unwrap();
        let info=video_probe::parse_probe(br#"{"streams":[{"index":0,"codec_type":"video","codec_name":"h264","width":1920,"height":1080,"avg_frame_rate":"30/1","time_base":"1/90000"}],"format":{"duration":"10.0"}}"#).unwrap();
        let p = new_project(root.to_str().unwrap(), &file, info).unwrap();
        (root, p)
    }
    #[test]
    fn durable_round_trip_keeps_edits_and_rejects_a_stale_writer() {
        let (root, mut p) = fixture();
        let file = root.join("projects.sqlite");
        let db = Connection::open(&file).unwrap();
        initialize(&db).unwrap();
        let r = root.to_str().unwrap();
        assert_eq!(store(&db, r, &p, None).unwrap(), 1);
        p.clips[0].edits.quarter_turns = 1;
        p.clips[0].edits.crop.width = 5000;
        p.clips[0].source_in = VideoTime::from_seconds(2.0, "1/90000").unwrap();
        assert_eq!(store(&db, r, &p, Some(1)).unwrap(), 2);
        let mut stale = p.clone();
        stale.title = "stale".into();
        assert!(store(&db, r, &stale, Some(1)).is_err());
        drop(db);
        let db = Connection::open(&file).unwrap();
        let (loaded, revision) = read(&db, r, &p.id).unwrap();
        assert_eq!(loaded, p);
        assert_eq!(revision, 2);
        let previous: String = db
            .query_row(
                "SELECT data FROM project_history WHERE project_id=?1",
                [&p.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Project>(&previous).unwrap().clips[0]
                .source_in
                .ticks,
            "0"
        );
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn validates_untrusted_documents_and_requires_original_bytes_after_a_move() {
        let (root, p) = fixture();
        let r = root.to_str().unwrap();
        assert!(source(r, &p, || false).is_ok());
        let mut bad = p.clone();
        for path in [
            "../secret.mp4",
            "C:/secret.mp4",
            "/secret.mp4",
            "folder/../secret.mp4",
            "folder\\secret.mp4",
        ] {
            bad.asset.relative_path = path.into();
            assert!(validate(&bad).is_err());
        }
        bad = p.clone();
        bad.schema_version = 2;
        assert!(validate(&bad).is_err());
        bad = p.clone();
        bad.clips[0].edits.crop.width = 10001;
        assert!(validate(&bad).is_err());
        bad = p.clone();
        bad.clips[0].source_out = bad.clips[0].source_in.clone();
        assert!(validate(&bad).is_err());
        fs::rename(root.join("original.mp4"), root.join("moved.mp4")).unwrap();
        assert!(source(r, &p, || false).is_err());
        let mut moved = p.clone();
        moved.asset.relative_path = "moved.mp4".into();
        assert!(source(r, &moved, || false).is_ok());
        fs::write(root.join("moved.mp4"), b"modified bytes").unwrap();
        assert!(source(r, &moved, || false)
            .unwrap_err()
            .contains("fingerprint"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn project_file_round_trip_and_existing_files_survive_copy_saves() {
        let (root, p) = fixture();
        let copy = root.join("project.json");
        write_copy(&p, &copy).unwrap();
        let restored: Project = serde_json::from_slice(&fs::read(&copy).unwrap()).unwrap();
        validate(&restored).unwrap();
        assert_eq!(p, restored);
        assert!(write_copy(&p, &copy).is_err());
        assert!(write_copy(&p, &root.join("original.mp4")).is_err());
        assert_eq!(
            fs::read(root.join("original.mp4")).unwrap(),
            b"original bytes"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
