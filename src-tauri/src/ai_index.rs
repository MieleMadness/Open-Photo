//! Workspace-owned, resumable local AI work. Only saved media snapshots feed this queue.
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, UNIX_EPOCH},
};

type Media = (String, String, i64, u64);
type States = HashMap<Media, (usize, bool)>;

#[derive(Clone, PartialEq)]
struct Config {
    workspace: String,
    enabled: bool,
    device: String,
    engine: Option<std::path::PathBuf>,
}
#[derive(Default)]
struct Control {
    config: Option<Config>,
    generation: u64,
    running: bool,
    error: Option<String>,
    revision: u64,
    snapshot: Option<(String, Arc<Vec<Media>>)>,
}
#[derive(Clone, Default)]
pub struct AiIndexState(Arc<Mutex<Control>>);
static GATE: OnceLock<Arc<tokio::sync::Mutex<()>>> = OnceLock::new();
static SEARCH_WAITING: AtomicUsize = AtomicUsize::new(0);
fn gate() -> Arc<tokio::sync::Mutex<()>> {
    GATE.get_or_init(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}
struct Waiting;
impl Drop for Waiting {
    fn drop(&mut self) {
        SEARCH_WAITING.fetch_sub(1, Ordering::SeqCst);
    }
}
pub(crate) async fn foreground_permit() -> tokio::sync::OwnedMutexGuard<()> {
    SEARCH_WAITING.fetch_add(1, Ordering::SeqCst);
    let waiting = Waiting;
    let permit = gate().lock_owned().await;
    drop(waiting);
    permit
}
fn db() -> Result<Connection, String> {
    let path = crate::index::database_path()?.with_file_name("ai-work.sqlite");
    let db = Connection::open(path).map_err(|e| e.to_string())?;
    db.busy_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    initialize(&db)?;
    Ok(db)
}
fn initialize(db: &Connection) -> Result<(), String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS jobs(workspace TEXT,root TEXT,path TEXT,modified INTEGER,bytes INTEGER,service TEXT,model TEXT,status TEXT NOT NULL DEFAULT 'pending',error TEXT,attempted INTEGER,PRIMARY KEY(workspace,root,path,service));
      CREATE TABLE IF NOT EXISTS exclusions(workspace TEXT,folder TEXT,PRIMARY KEY(workspace,folder));
      CREATE INDEX IF NOT EXISTS idx_ai_pending ON jobs(workspace,status,service);").map_err(|e|e.to_string())?;
    Ok(())
}
pub(crate) fn normalized(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("//?/")
        .trim_end_matches('/')
        .to_lowercase()
}
fn within(path: &str, parent: &str) -> bool {
    let path = normalized(path);
    let parent = normalized(parent);
    path == parent || path.starts_with(&(parent + "/"))
}
pub(crate) fn exclusions(workspace: &str) -> Result<Vec<String>, String> {
    let db = db()?;
    let mut stmt = db
        .prepare("SELECT folder FROM exclusions WHERE workspace=?1")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([workspace], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string());
    rows
}
pub(crate) fn allowed(path: &str, excluded: &[String]) -> bool {
    !excluded.iter().any(|folder| within(path, folder))
}
pub(crate) fn verify_source(
    root: &str,
    path: &str,
    modified: i64,
    bytes: u64,
) -> Result<(), String> {
    let file = crate::safe_media_path(root, path)?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    let current = meta
        .modified()
        .map_err(|e| e.to_string())?
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;
    if meta.len() != bytes || current != modified {
        return Err("The source changed; Sync the library before retrying".into());
    }
    Ok(())
}
/// Legacy manual commands cannot bypass workspace policy or the AI switch.
pub(crate) fn require_allowed(root: &str, paths: &[String]) -> Result<(), String> {
    if crate::user_data::setting("openphoto.ai.optin")?.as_deref() != Some("true") {
        return Err("Turn local AI on before indexing".into());
    }
    let workspace =
        crate::user_data::setting("openphoto.activeWorkspace")?.ok_or("Choose a workspace")?;
    let roots = crate::user_data::workspace_roots(&workspace)?;
    let excluded = exclusions(&workspace)?;
    if !roots.iter().any(|r| normalized(r) == normalized(root)) {
        return Err("This folder is not in the active workspace".into());
    }
    for path in paths {
        let canonical = crate::safe_media_path(root, path)?;
        if !allowed(path, &excluded) || !allowed(&canonical.to_string_lossy(), &excluded) {
            return Err("This folder is excluded from AI indexing".into());
        }
    }
    Ok(())
}
pub(crate) fn active_exclusions() -> Result<Vec<String>, String> {
    match crate::user_data::setting("openphoto.activeWorkspace")? {
        Some(workspace) => exclusions(&workspace),
        None => Ok(vec![]),
    }
}
fn supported(path: &str) -> bool {
    if crate::video_semantic::is_video(path) {
        return true;
    }
    matches!(
        path.rsplit('.')
            .next()
            .unwrap_or("")
            .to_lowercase()
            .as_str(),
        "jpg" | "jpeg" | "png" | "webp" | "gif" | "tif" | "tiff" | "bmp" | "avif"
    )
}
fn model(service: &str) -> String {
    use crate::ai_models::*;
    match service {
        "video" => crate::video_semantic::key(),
        "semantic" => format!("openclip-vit-b32-oriented-v2:{SUPPORTED_CLIP_VISION_SHA256}"),
        "ocr" => format!(
            "{}:{SUPPORTED_OCR_DETECTOR_SHA256}:{SUPPORTED_OCR_RECOGNIZER_SHA256}",
            crate::image_decode::POLICY
        ),
        _ => format!(
            "{}:{SUPPORTED_YUNET_SHA256}:{SUPPORTED_SFACE_SHA256}",
            crate::image_decode::POLICY
        ),
    }
}
fn reconcile(
    db: &mut Connection,
    workspace: &str,
    media: &[(String, String, i64, u64)],
    excluded: &[String],
) -> Result<(), String> {
    let tx = db.transaction().map_err(|e| e.to_string())?;
    tx.execute_batch("CREATE TEMP TABLE IF NOT EXISTS seen(root TEXT,path TEXT,PRIMARY KEY(root,path)); DELETE FROM seen;").map_err(|e|e.to_string())?;
    for (root, path, modified, bytes) in media {
        if !supported(path) || !allowed(path, excluded) {
            continue;
        }
        tx.execute(
            "INSERT OR IGNORE INTO seen VALUES(?1,?2)",
            params![root, path],
        )
        .map_err(|e| e.to_string())?;
        let services = if crate::video_semantic::is_video(path) {
            vec!["video"]
        } else {
            vec!["semantic", "ocr", "people"]
        };
        for service in services {
            tx.execute("INSERT INTO jobs(workspace,root,path,modified,bytes,service,model,status) VALUES(?1,?2,?3,?4,?5,?6,?7,'pending') ON CONFLICT(workspace,root,path,service) DO UPDATE SET modified=excluded.modified,bytes=excluded.bytes,model=excluded.model,status='pending',error=NULL WHERE jobs.modified!=excluded.modified OR jobs.bytes!=excluded.bytes OR jobs.model!=excluded.model",params![workspace,root,path,modified,bytes,service,model(service)]).map_err(|e|e.to_string())?;
        }
    }
    // Removing/excluding a folder removes queue ownership, never source media or shared indexes.
    tx.execute("DELETE FROM jobs WHERE workspace=?1 AND NOT EXISTS(SELECT 1 FROM seen WHERE seen.root=jobs.root AND seen.path=jobs.path)",[workspace]).map_err(|e|e.to_string())?;
    tx.execute(
        "UPDATE jobs SET status='pending' WHERE workspace=?1 AND status IN ('running','blocked')",
        [workspace],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}
fn media(workspace: &str) -> Result<Vec<(String, String, i64, u64)>, String> {
    let mut all = vec![];
    let mut roots = crate::user_data::workspace_roots(workspace)?;
    roots.sort_by_key(|r| std::cmp::Reverse(r.len()));
    let mut seen = HashSet::new();
    for root in roots {
        let scan = crate::index::saved_library(root.clone())?;
        if scan.truncated {
            return Err(
                "This library exceeds the current saved-index limit; indexing is paused".into(),
            );
        }
        for photo in scan.photos {
            if !seen.insert(normalized(&photo.path)) {
                continue;
            }
            all.push((
                root.clone(),
                photo.path,
                photo.modified_unix as i64,
                photo.bytes,
            ));
        }
    }
    Ok(all)
}
#[derive(Clone)]
struct Job {
    root: String,
    path: String,
    modified: i64,
    bytes: u64,
    service: String,
    model: String,
}
fn next(db: &Connection, workspace: &str) -> Result<Option<Job>, String> {
    use rusqlite::OptionalExtension;
    db.query_row("SELECT root,path,modified,bytes,service,model FROM jobs WHERE workspace=?1 AND status='pending' ORDER BY CASE service WHEN 'semantic' THEN 0 WHEN 'ocr' THEN 1 ELSE 2 END,root,path LIMIT 1",[workspace],|r|Ok(Job{root:r.get(0)?,path:r.get(1)?,modified:r.get(2)?,bytes:r.get(3)?,service:r.get(4)?,model:r.get(5)?})).optional().map_err(|e|e.to_string())
}
enum Worker {
    Semantic(crate::ai_embeddings::IndexWorker),
    Ocr(crate::ai_ocr::IndexWorker),
    People(crate::ai_faces::IndexWorker),
    Video(Box<crate::video_semantic::IndexWorker>),
}
impl Worker {
    fn new(
        service: &str,
        device: &str,
        engine: Option<std::path::PathBuf>,
    ) -> Result<Self, String> {
        match service {
            "video" => Ok(Self::Video(Box::new(
                crate::video_semantic::IndexWorker::new(
                    device,
                    engine.ok_or("Video engine unavailable")?,
                )?,
            ))),
            "semantic" => Ok(Self::Semantic(crate::ai_embeddings::IndexWorker::new(
                device,
            )?)),
            "ocr" => Ok(Self::Ocr(crate::ai_ocr::IndexWorker::new(device)?)),
            _ => Ok(Self::People(crate::ai_faces::IndexWorker::new(device)?)),
        }
    }
    fn process(&mut self, j: &Job, cancel: impl Fn() -> bool) -> Result<bool, String> {
        match self {
            Self::Semantic(w) => w
                .process(&j.root, &j.path, j.modified, j.bytes)
                .map(|_| true),
            Self::Ocr(w) => w
                .process(&j.root, &j.path, j.modified, j.bytes)
                .map(|_| true),
            Self::People(w) => w
                .process(&j.root, &j.path, j.modified, j.bytes)
                .map(|_| true),
            Self::Video(w) => w.process(&j.root, &j.path, j.modified, j.bytes, cancel),
        }
    }
}
fn finish(
    db: &Connection,
    control: &mut Control,
    generation: u64,
    workspace: &str,
    job: &Job,
    result: Result<bool, String>,
) -> Result<(), String> {
    if control.generation != generation {
        return Ok(());
    }
    let complete = matches!(result, Ok(true));
    let error = result
        .err()
        .map(|e| e.chars().take(1000).collect::<String>());
    let changed=db.execute("UPDATE jobs SET status=?1,error=?2,attempted=unixepoch() WHERE workspace=?3 AND root=?4 AND path=?5 AND service=?6 AND modified=?7 AND bytes=?8 AND model=?9",params![if error.is_some(){"failed"}else if complete{"indexed"}else{"pending"},error,workspace,job.root,job.path,job.service,job.modified,job.bytes,job.model]).map_err(|e|e.to_string())?;
    if changed > 0 && (complete || error.is_some()) {
        control.revision += 1;
    }
    Ok(())
}
fn run(state: AiIndexState) {
    let mut worker: Option<Worker> = None;
    let mut service = String::new();
    let mut seen = u64::MAX;
    loop {
        let (config, generation) = {
            let mut control = state.0.lock().unwrap();
            match control.config.clone() {
                Some(c) if c.enabled => (c, control.generation),
                _ => {
                    control.running = false;
                    break;
                }
            }
        };
        let step = (|| -> Result<bool, String> {
            let mut database = db()?;
            if seen != generation {
                worker = None;
                service.clear();
                reconcile(
                    &mut database,
                    &config.workspace,
                    &media(&config.workspace)?,
                    &exclusions(&config.workspace)?,
                )?;
                seen = generation;
            }
            let Some(job) = next(&database, &config.workspace)? else {
                return Ok(false);
            };
            if !std::fs::metadata(&job.root).is_ok_and(|m| m.is_dir()) {
                let mut control = state.0.lock().unwrap();
                if control.generation != generation {
                    return Ok(true);
                }
                database.execute("UPDATE jobs SET status='blocked',error='Folder unavailable. Reconnect it and choose Retry failed / resume.' WHERE workspace=?1 AND root=?2 AND status='pending'",params![config.workspace,job.root]).map_err(|e|e.to_string())?;
                control.revision += 1;
                return Ok(true);
            }
            // A foreground request gets the next inference slot before another background image.
            while SEARCH_WAITING.load(Ordering::SeqCst) > 0 {
                if state.0.lock().unwrap().generation != generation {
                    return Ok(true);
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            let _permit = gate().blocking_lock_owned();
            if state.0.lock().unwrap().generation != generation {
                return Ok(true);
            }
            if service != job.service {
                worker = Some(Worker::new(
                    &job.service,
                    &config.device,
                    config.engine.clone(),
                )?);
                service = job.service.clone();
            }
            database.execute("UPDATE jobs SET status='running' WHERE workspace=?1 AND root=?2 AND path=?3 AND service=?4",params![config.workspace,job.root,job.path,job.service]).map_err(|e|e.to_string())?;
            let result = worker.as_mut().unwrap().process(&job, || {
                state.0.lock().map_or(true, |c| c.generation != generation)
            });
            // A source unit can finish after pause/exclusion; only its current generation may commit progress.
            finish(
                &database,
                &mut state.0.lock().unwrap(),
                generation,
                &config.workspace,
                &job,
                result,
            )?;
            Ok(true)
        })();
        match step {
            Ok(true) => std::thread::sleep(Duration::from_millis(10)),
            result => {
                let mut control = state.0.lock().unwrap();
                if control.generation != generation {
                    continue;
                }
                control.error = result.err();
                control.running = false;
                break;
            }
        }
    }
}

#[tauri::command]
pub fn pause_ai_index(state: tauri::State<'_, AiIndexState>) -> Result<(), String> {
    let mut control = state.0.lock().map_err(|e| e.to_string())?;
    control.generation += 1;
    if let Some(config) = &mut control.config {
        config.enabled = false;
    }
    Ok(())
}
#[tauri::command]
pub fn configure_ai_index(
    app: tauri::AppHandle,
    state: tauri::State<'_, AiIndexState>,
    workspace_id: String,
    enabled: bool,
    device: String,
) -> Result<(), String> {
    crate::ai_runtime::normalized_device(Some(&device))?;
    if !workspace_id.is_empty() {
        crate::user_data::workspace_roots(&workspace_id)?;
    }
    let mut control = state.0.lock().map_err(|e| e.to_string())?;
    control.config = Some(Config {
        workspace: workspace_id.clone(),
        enabled: enabled && !workspace_id.is_empty(),
        device,
        engine: crate::video_engine::directory(&app).ok(),
    });
    control.generation += 1;
    control.error = None;
    control.snapshot = None;
    if enabled && !workspace_id.is_empty() && !control.running {
        control.running = true;
        let copy = state.inner().clone();
        tauri::async_runtime::spawn_blocking(move || run(copy));
    }
    Ok(())
}
#[derive(Serialize, Default)]
pub struct Counts {
    indexed: usize,
    pending: usize,
    failed: usize,
    partial: usize,
    unsupported: usize,
    excluded: usize,
}
#[derive(Serialize)]
pub struct Folder {
    path: String,
    excluded: bool,
    inherited: bool,
    note: Option<String>,
    counts: Counts,
}
#[derive(Serialize)]
pub struct Failure {
    root: String,
    path: String,
    service: String,
    error: String,
    attempted: Option<i64>,
}
#[derive(Serialize)]
pub struct Report {
    counts: Counts,
    folders: Vec<Folder>,
    failures: Vec<Failure>,
    failure_total: usize,
    running: bool,
    error: Option<String>,
    revision: u64,
    generation: u64,
}
fn states(db: &Connection, workspace: &str) -> Result<States, String> {
    let mut statement=db.prepare("SELECT root,path,modified,bytes,status,COUNT(*) FROM jobs WHERE workspace=?1 GROUP BY root,path,modified,bytes,status").map_err(|e|e.to_string())?;
    let mut states = States::new();
    let rows = statement
        .query_map([workspace], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, usize>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (root, path, modified, bytes, status, count) = row.map_err(|e| e.to_string())?;
        let state = states.entry((root, path, modified, bytes)).or_default();
        if status == "indexed" {
            state.0 += count;
        }
        if status == "failed" {
            state.1 = true;
        }
    }
    Ok(states)
}
fn counts(states: &States, files: &[Media], excluded: &[String]) -> Counts {
    let mut result = Counts::default();
    let mut seen = HashSet::new();
    for (root, path, modified, bytes) in files {
        if !seen.insert(normalized(path)) {
            continue;
        }
        if !allowed(path, excluded) {
            result.excluded += 1;
            continue;
        }
        if !supported(path) {
            result.unsupported += 1;
            continue;
        }
        let (done, failed) = states
            .get(&(root.clone(), path.clone(), *modified, *bytes))
            .copied()
            .unwrap_or_default();
        if failed {
            result.failed += 1;
        } else if done
            == if crate::video_semantic::is_video(path) {
                1
            } else {
                3
            }
        {
            result.indexed += 1;
        } else {
            result.pending += 1;
        }
        if done > 0
            && done
                < if crate::video_semantic::is_video(path) {
                    1
                } else {
                    3
                }
        {
            result.partial += 1;
        }
    }
    result
}
#[tauri::command]
pub async fn ai_index_report(
    state: tauri::State<'_, AiIndexState>,
    workspace_id: String,
    failure_query: Option<String>,
    failure_offset: Option<usize>,
) -> Result<Report, String> {
    let query = failure_query.unwrap_or_default();
    if query.chars().count() > 500 {
        return Err("Keep failure searches within 500 characters".into());
    }
    let offset = failure_offset.unwrap_or(0).min(1_000_000);
    let copy = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move||{
    let (cached,generation)={let c=copy.0.lock().map_err(|e|e.to_string())?;(c.snapshot.as_ref().filter(|(w,_)|w==&workspace_id).map(|(_,f)|f.clone()),c.generation)};
    let files=match cached {Some(files)=>files,None=>{let files=Arc::new(media(&workspace_id)?);let mut c=copy.0.lock().map_err(|e|e.to_string())?;if c.generation==generation{c.snapshot=Some((workspace_id.clone(),files.clone()));}files}};
    let excluded=exclusions(&workspace_id)?;let database=db()?;let states=states(&database,&workspace_id)?;let mut folders=vec![];
    let roots=crate::user_data::workspace_roots(&workspace_id)?;
    for root in &roots {let root_files=files.iter().filter(|(r,_,_,_)|r==root).cloned().collect::<Vec<_>>();let note=database.query_row("SELECT error FROM jobs WHERE workspace=?1 AND root=?2 AND status='blocked' LIMIT 1",params![workspace_id,root],|r|r.get::<_,String>(0)).optional().map_err(|e|e.to_string())?;folders.push(Folder{note,excluded:!allowed(root,&excluded),inherited:!allowed(root,&excluded)&&!excluded.iter().any(|folder|normalized(folder)==normalized(root)),counts:counts(&states,&root_files,&excluded),path:root.clone()});}
    crate::search_providers::attach_current(&database,&crate::index::database_path()?)?;
    crate::search_providers::prepare_scope(&database,&excluded)?;
    let roots_json=serde_json::to_string(&roots).map_err(|e|e.to_string())?;

    let predicate="FROM jobs j JOIN visible_media m ON m.root=j.root AND m.path=j.path AND m.modified_unix=j.modified AND m.bytes=j.bytes WHERE j.workspace=?1 AND j.status='failed' AND j.root IN (SELECT value FROM json_each(?2)) AND instr(lower(j.path||' '||j.service||' '||coalesce(j.error,'')),lower(?3))>0";
    let mut statement=database.prepare(&format!("SELECT j.root,j.path,j.service,j.error,j.attempted {predicate} ORDER BY j.attempted DESC,j.path LIMIT 100 OFFSET ?4")).map_err(|e|e.to_string())?;
    let failures=statement.query_map(params![workspace_id,roots_json,query,offset],|r|Ok(Failure{root:r.get(0)?,path:r.get(1)?,service:r.get(2)?,error:r.get(3)?,attempted:r.get(4)?})).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
    let failure_total=database.query_row(&format!("SELECT COUNT(*) {predicate}"),params![workspace_id,roots_json,query],|r|r.get(0)).map_err(|e|e.to_string())?;
    let control=copy.0.lock().map_err(|e|e.to_string())?;if control.generation!=generation{return Err("Index state changed; refresh status".into());}
    let current=control.config.as_ref().is_some_and(|c|c.workspace==workspace_id);
    Ok(Report{counts:counts(&states,&files,&excluded),folders,failures,failure_total,running:current&&control.running,error:if current{control.error.clone()}else{None},revision:control.revision,generation})
 }).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub fn ai_index_exclude(
    state: tauri::State<'_, AiIndexState>,
    workspace_id: String,
    folder: String,
    excluded: bool,
) -> Result<(), String> {
    if !crate::user_data::workspace_roots(&workspace_id)?.contains(&folder) {
        return Err("Choose a registered workspace folder".into());
    }
    if !excluded
        && exclusions(&workspace_id)?
            .iter()
            .any(|parent| within(&folder, parent) && normalized(parent) != normalized(&folder))
    {
        return Err("Enable the parent folder first".into());
    }
    // Fence before the policy changes, so in-flight completion cannot publish current progress.
    let mut control = state.0.lock().map_err(|e| e.to_string())?;
    control.generation += 1;
    let database = db()?;
    if excluded {
        database.execute(
            "INSERT OR IGNORE INTO exclusions VALUES(?1,?2)",
            params![workspace_id, folder],
        )
    } else {
        database.execute(
            "DELETE FROM exclusions WHERE workspace=?1 AND folder=?2",
            params![workspace_id, folder],
        )
    }
    .map_err(|e| e.to_string())?;
    if control
        .config
        .as_ref()
        .is_some_and(|c| c.workspace == workspace_id && c.enabled)
        && !control.running
    {
        control.running = true;
        control.error = None;
        let copy = state.inner().clone();
        tauri::async_runtime::spawn_blocking(move || run(copy));
    }
    Ok(())
}
#[tauri::command]
pub fn retry_ai_index(
    state: tauri::State<'_, AiIndexState>,
    workspace_id: String,
    path: Option<String>,
) -> Result<(), String> {
    crate::user_data::workspace_roots(&workspace_id)?;
    db()?.execute("UPDATE jobs SET status='pending',error=NULL WHERE workspace=?1 AND status IN ('failed','blocked') AND (?2 IS NULL OR path=?2)",params![workspace_id,path]).map_err(|e|e.to_string())?;
    let mut control = state.0.lock().map_err(|e| e.to_string())?;
    control.generation += 1;
    control.error = None;
    if control
        .config
        .as_ref()
        .is_some_and(|c| c.workspace == workspace_id && c.enabled)
        && !control.running
    {
        control.running = true;
        let copy = state.inner().clone();
        tauri::async_runtime::spawn_blocking(move || run(copy));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn memory() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        initialize(&db).unwrap();
        db
    }
    fn files() -> Vec<Media> {
        vec![
            ("C:/photos".into(), "C:/photos/one.jpg".into(), 10, 42),
            (
                "C:/photos".into(),
                "C:/photos/private/two.jpg".into(),
                10,
                21,
            ),
            ("C:/photos".into(), "C:/photos/clip.mp4".into(), 10, 80),
        ]
    }
    #[test]
    fn obsolete_generations_and_source_versions_cannot_publish_progress() {
        let mut db = memory();
        let mut items = files();
        reconcile(&mut db, "a", &items, &[]).unwrap();
        let old = next(&db, "a").unwrap().unwrap();
        let mut control = Control {
            generation: 2,
            ..Default::default()
        };
        finish(&db, &mut control, 1, "a", &old, Ok(true)).unwrap();
        assert_eq!(control.revision, 0);
        items[0].2 = 20;
        reconcile(&mut db, "a", &items, &[]).unwrap();
        finish(&db, &mut control, 2, "a", &old, Ok(true)).unwrap();
        assert_eq!(control.revision, 0);
        let current = next(&db, "a").unwrap().unwrap();
        finish(&db, &mut control, 2, "a", &current, Ok(false)).unwrap();
        assert_eq!(control.revision, 0);
        finish(
            &db,
            &mut control,
            2,
            "a",
            &current,
            Err("Decode failed".into()),
        )
        .unwrap();
        assert_eq!(control.revision, 1);
        assert_eq!(counts(&states(&db, "a").unwrap(), &items, &[]).failed, 1);
    }
    #[test]
    fn scope_boundaries_ignore_case_separators_and_verbatim_prefix_without_prefix_collisions() {
        let excluded = vec!["C:\\Photos\\Private".into()];
        assert!(!allowed("\\\\?\\c:\\photos\\PRIVATE\\one.jpg", &excluded));
        assert!(allowed("C:/photos/private-other/one.jpg", &excluded));
        assert!(allowed("D:/photos/private/one.jpg", &excluded));
    }
    #[test]
    fn reconciliation_keeps_current_success_and_failures_and_resets_only_changed_versions() {
        let mut db = memory();
        let mut items = files();
        let excluded = vec!["C:/photos/private".into()];
        reconcile(&mut db, "a", &items, &excluded).unwrap();
        assert_eq!(states(&db, "a").unwrap().len(), 2);
        db.execute(
            "UPDATE jobs SET status='indexed' WHERE service='semantic'",
            [],
        )
        .unwrap();
        db.execute(
            "UPDATE jobs SET status='failed',error='corrupt' WHERE service='ocr'",
            [],
        )
        .unwrap();
        reconcile(&mut db, "a", &items, &excluded).unwrap();
        let c = counts(&states(&db, "a").unwrap(), &items, &excluded);
        assert_eq!(
            (c.failed, c.partial, c.excluded, c.unsupported),
            (1, 1, 1, 0)
        );
        items[0].2 = 11;
        reconcile(&mut db, "a", &items, &excluded).unwrap();
        assert_eq!(
            counts(&states(&db, "a").unwrap(), &items, &excluded).pending,
            2
        );
        db.execute("UPDATE jobs SET status='indexed',model='old-policy'", [])
            .unwrap();
        reconcile(&mut db, "a", &items, &excluded).unwrap();
        assert_eq!(
            counts(&states(&db, "a").unwrap(), &items, &excluded).indexed,
            0
        );
    }
    #[test]
    fn interrupted_jobs_resume_and_exclusion_removes_only_queue_ownership() {
        let mut db = memory();
        let items = files();
        reconcile(&mut db, "a", &items, &[]).unwrap();
        reconcile(&mut db, "b", &items, &[]).unwrap();
        db.execute("UPDATE jobs SET status='running' WHERE workspace='a'", [])
            .unwrap();
        initialize(&db).unwrap();
        reconcile(&mut db, "a", &items, &[]).unwrap();
        assert!(next(&db, "a").unwrap().is_some());
        reconcile(&mut db, "a", &items, &["C:/photos".into()]).unwrap();
        assert!(next(&db, "a").unwrap().is_none());
        assert!(next(&db, "b").unwrap().is_some());
        reconcile(&mut db, "a", &items, &[]).unwrap();
        assert!(next(&db, "a").unwrap().is_some());
    }
    #[test]
    fn counts_are_files_not_service_rows_and_deduplicate_overlapping_sources() {
        let mut db = memory();
        let mut items = files();
        items.push(items[0].clone());
        reconcile(&mut db, "a", &items, &[]).unwrap();
        db.execute("UPDATE jobs SET status='indexed'", []).unwrap();
        let c = counts(&states(&db, "a").unwrap(), &items, &[]);
        assert_eq!((c.indexed, c.pending, c.unsupported), (3, 0, 0));
    }
}
