//! Workspace-owned projects. Portable documents contain edit decisions, never access grants.
//! Per-asset root bindings are local metadata and can only be changed by verified import/relink.
use crate::{
    user_data, video_engine, video_probe, video_project,
    video_sequence::{self, Sequence},
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

type Bindings = BTreeMap<String, String>;
struct Record {
    project: Sequence,
    revision: u64,
    bindings: Bindings,
}
#[derive(Serialize)]
pub struct Source {
    asset_id: String,
    path: Option<String>,
    root: Option<String>,
    status: String,
}
#[derive(Serialize)]
pub struct Loaded {
    project: Sequence,
    revision: u64,
    sources: Vec<Source>,
}
#[derive(Serialize)]
pub struct PreviousSave {
    id: i64,
    revision: Option<u64>,
    saved_at: Option<i64>,
    title: String,
    clips: usize,
    duration_seconds: f64,
}
#[derive(Serialize)]
pub struct ProjectList {
    projects: Vec<video_project::RecentProject>,
    warnings: Vec<String>,
}

fn initialize(db: &Connection) -> Result<(), String> {
    video_sequence::initialize(db)?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS workspace_video_projects(workspace TEXT NOT NULL,id TEXT NOT NULL,title TEXT NOT NULL,revision INTEGER NOT NULL,data TEXT NOT NULL,bindings TEXT NOT NULL,updated INTEGER NOT NULL,PRIMARY KEY(workspace,id));
      CREATE TABLE IF NOT EXISTS workspace_video_migrations(workspace TEXT NOT NULL,kind TEXT NOT NULL,legacy_id TEXT NOT NULL,project_id TEXT NOT NULL,PRIMARY KEY(workspace,kind,legacy_id));
      CREATE TABLE IF NOT EXISTS workspace_video_history(id INTEGER PRIMARY KEY,workspace TEXT NOT NULL,project_id TEXT NOT NULL,revision INTEGER,data TEXT NOT NULL,bindings TEXT NOT NULL,saved_at INTEGER);
      CREATE TRIGGER IF NOT EXISTS workspace_video_backup BEFORE UPDATE ON workspace_video_projects BEGIN
        INSERT INTO workspace_video_history(workspace,project_id,revision,data,bindings,saved_at) VALUES(old.workspace,old.id,old.revision,old.data,old.bindings,old.updated);
        DELETE FROM workspace_video_history WHERE workspace=old.workspace AND project_id=old.id AND id NOT IN (SELECT id FROM workspace_video_history WHERE workspace=old.workspace AND project_id=old.id ORDER BY id DESC LIMIT 20);
      END;").map_err(|e| e.to_string())
}
fn database() -> Result<Connection, String> {
    let db = video_project::database()?;
    initialize(&db)?;
    Ok(db)
}
fn read(db: &Connection, workspace: &str, id: &str) -> Result<Record, String> {
    let row: Option<(String,u64,String)> = db.query_row("SELECT data,revision,bindings FROM workspace_video_projects WHERE workspace=?1 AND id=?2", params![workspace,id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(|e|e.to_string())?;
    let (data, revision, bindings) = row.ok_or("Project not found in this workspace")?;
    Ok(Record {
        project: video_sequence::parse(data.as_bytes())?,
        revision,
        bindings: serde_json::from_str(&bindings).map_err(|_| "Invalid local source references")?,
    })
}
fn store(
    db: &Connection,
    workspace: &str,
    project: &Sequence,
    bindings: &Bindings,
    expected: Option<u64>,
) -> Result<u64, String> {
    video_sequence::validate(project)?;
    if workspace.is_empty()
        || workspace.len() > 120
        || bindings
            .keys()
            .any(|id| !project.assets.iter().any(|a| a.id == *id))
    {
        return Err("Invalid project ownership or source references".into());
    }
    let data = serde_json::to_string(project).map_err(|e| e.to_string())?;
    let bindings = serde_json::to_string(bindings).map_err(|e| e.to_string())?;
    if let Some(revision) = expected {
        let count=db.execute("UPDATE workspace_video_projects SET title=?1,revision=revision+1,data=?2,bindings=?3,updated=unixepoch() WHERE workspace=?4 AND id=?5 AND revision=?6",params![project.title,data,bindings,workspace,project.id,revision]).map_err(|e|e.to_string())?;
        if count != 1 {
            return Err("Project changed elsewhere. Reopen it before editing further.".into());
        }
        Ok(revision + 1)
    } else {
        db.execute(
            "INSERT INTO workspace_video_projects VALUES(?1,?2,?3,1,?4,?5,unixepoch())",
            params![workspace, project.id, project.title, data, bindings],
        )
        .map_err(|e| e.to_string())?;
        Ok(1)
    }
}
fn registered_root(roots: &[String], bound: &str) -> Result<PathBuf, String> {
    let bound = fs::canonicalize(bound).map_err(|_| "Source folder is unavailable")?;
    if !roots
        .iter()
        .filter_map(|r| fs::canonicalize(r).ok())
        .any(|r| r == bound)
    {
        return Err(
            "Source folder is no longer in this workspace. Add it again or relink the video."
                .into(),
        );
    }
    Ok(bound)
}
pub(crate) fn source(
    roots: &[String],
    project: &Sequence,
    bindings: &Bindings,
    asset: &video_project::Asset,
) -> Result<(PathBuf, PathBuf), String> {
    let root = registered_root(
        roots,
        bindings
            .get(&asset.id)
            .ok_or("Relink this project's original video")?,
    )?;
    let single = video_sequence::single(project, asset, video_sequence::default_clip(asset)?);
    let file = video_project::source(
        root.to_str().ok_or("Unsupported folder path")?,
        &single,
        || false,
    )?;
    Ok((root, file))
}
fn loaded(roots: &[String], record: Record) -> Loaded {
    let sources = record
        .project
        .assets
        .iter()
        .map(
            |asset| match source(roots, &record.project, &record.bindings, asset) {
                Ok((root, path)) => Source {
                    asset_id: asset.id.clone(),
                    root: Some(root.to_string_lossy().into_owned()),
                    path: Some(path.to_string_lossy().into_owned()),
                    status: "ready".into(),
                },
                Err(status) => Source {
                    asset_id: asset.id.clone(),
                    root: None,
                    path: None,
                    status,
                },
            },
        )
        .collect();
    Loaded {
        project: record.project,
        revision: record.revision,
        sources,
    }
}
fn root_for_file(roots: &[String], path: &str) -> Result<(PathBuf, PathBuf), String> {
    let file = fs::canonicalize(path).map_err(|_| "Video is unavailable")?;
    let root = roots
        .iter()
        .filter_map(|r| fs::canonicalize(r).ok())
        .filter(|r| file.starts_with(r))
        .max_by_key(|r| r.components().count())
        .ok_or("Choose a video inside this workspace's folders")?;
    crate::media_stream::validated_video(root.to_str().ok_or("Unsupported folder path")?, path)?;
    Ok((root, file))
}
fn bind_import(roots: &[String], project: &Sequence) -> Bindings {
    let mut bindings = Bindings::new();
    for asset in &project.assets {
        for root in roots {
            if !Path::new(root).join(&asset.relative_path).is_file() {
                continue;
            }
            let candidate = BTreeMap::from([(asset.id.clone(), root.clone())]);
            if source(roots, project, &candidate, asset).is_ok() {
                bindings.insert(asset.id.clone(), root.clone());
                break;
            }
        }
    }
    bindings
}
fn new_record() -> Record {
    Record {
        project: Sequence {
            schema_version: 2,
            id: video_project::id(),
            title: "New video project".into(),
            assets: vec![],
            clips: vec![],
            output: video_project::Output {
                width: 1920,
                height: 1080,
                fit: "fit".into(),
            },
            frame_rate: 30,
        },
        revision: 0,
        bindings: Bindings::new(),
    }
}
fn open_source(
    db: &Connection,
    engine: &Path,
    workspace: &str,
    roots: &[String],
    path: &str,
) -> Result<Record, String> {
    let mut fresh = new_record();
    add_files(engine, roots, &mut fresh, &[path.into()])?;
    let asset = &fresh.project.assets[0];
    let mut q=db.prepare("SELECT id FROM workspace_video_projects WHERE workspace=?1 ORDER BY updated DESC,rowid DESC").map_err(|e|e.to_string())?;
    let rows = q
        .query_map([workspace], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    for row in rows {
        let mut existing = read(db, workspace, &row.map_err(|e| e.to_string())?)?;
        if existing.project.assets.len() != 1 {
            continue;
        }
        let saved = &mut existing.project.assets[0];
        if saved.sha256 != asset.sha256 || saved.bytes != asset.bytes {
            continue;
        }
        let root = fresh.bindings.get(&asset.id).unwrap();
        if saved.relative_path != asset.relative_path
            || existing.bindings.get(&saved.id) != Some(root)
        {
            saved.relative_path = asset.relative_path.clone();
            existing.bindings.insert(saved.id.clone(), root.clone());
            existing.revision = store(
                db,
                workspace,
                &existing.project,
                &existing.bindings,
                Some(existing.revision),
            )?;
        }
        return Ok(existing);
    }
    fresh.revision = store(db, workspace, &fresh.project, &fresh.bindings, None)?;
    Ok(fresh)
}
fn add_files(
    engine: &Path,
    roots: &[String],
    record: &mut Record,
    paths: &[String],
) -> Result<(), String> {
    if paths.is_empty() || paths.len() > 50 {
        return Err("Choose 1–50 videos".into());
    }
    let mut project = record.project.clone();
    let mut bindings = record.bindings.clone();
    for path in paths {
        let (root, file) = root_for_file(roots, path)?;
        let info = video_probe::probe(engine, &file, || false)?;
        let legacy = video_project::new_project(
            root.to_str().ok_or("Unsupported folder path")?,
            &file,
            info,
        )?;
        if project
            .assets
            .iter()
            .any(|a| a.bytes == legacy.asset.bytes && a.sha256 == legacy.asset.sha256)
        {
            continue;
        }
        if project.assets.is_empty() {
            project.output = legacy.output;
            project.title = legacy.title;
        }
        bindings.insert(legacy.asset.id.clone(), root.to_string_lossy().into_owned());
        project.assets.push(legacy.asset);
        project.clips.extend(legacy.clips);
    }
    video_sequence::validate(&project)?;
    record.project = project;
    record.bindings = bindings;
    Ok(())
}
/// Copies legacy drafts once, in one transaction, preserving the legacy documents and revisions.
fn migrate_legacy(
    db: &Connection,
    workspace: &str,
    roots: &[String],
) -> Result<Vec<String>, String> {
    let mut warnings = vec![];
    let tx = Transaction::new_unchecked(db, TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    for (kind, table) in [("single", "projects"), ("sequence", "video_sequences")] {
        let mut query = tx
            .prepare(&format!(
                "SELECT id,root,data FROM {table} ORDER BY updated,rowid"
            ))
            .map_err(|e| e.to_string())?;
        let rows = query
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (legacy_id, root, data) = row.map_err(|e| e.to_string())?;
            if registered_root(roots, &root).is_err() {
                continue;
            }
            let migrated:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM workspace_video_migrations WHERE workspace=?1 AND kind=?2 AND legacy_id=?3)",params![workspace,kind,legacy_id],|r|r.get(0)).map_err(|e|e.to_string())?;
            if migrated {
                continue;
            }
            let mut project = match video_sequence::parse(data.as_bytes()) {
                Ok(project) => project,
                Err(error) => {
                    warnings.push(format!("An older {kind} draft ({legacy_id}) was kept unchanged and could not be copied: {error}"));
                    continue;
                }
            };
            let legacy_document_id = project.id.clone();
            project.id = video_project::id();
            let bindings = project
                .assets
                .iter()
                .map(|a| (a.id.clone(), root.clone()))
                .collect();
            store(&tx, workspace, &project, &bindings, None)?;
            let history_sql = if kind == "single" {
                "SELECT revision,saved_at,data FROM project_history WHERE project_id=?1 ORDER BY id DESC LIMIT 20"
            } else {
                "SELECT revision,NULL,data FROM video_sequence_history WHERE project_id=?1 ORDER BY id DESC LIMIT 20"
            };
            let mut history = tx.prepare(history_sql).map_err(|e| e.to_string())?;
            let rows = history
                .query_map([&legacy_id], |r| {
                    Ok((
                        r.get::<_, Option<u64>>(0)?,
                        r.get::<_, Option<i64>>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })
                .map_err(|e| e.to_string())?;
            let snapshots = rows
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            for (revision, saved_at, data) in snapshots.into_iter().rev() {
                let mut snapshot = match video_sequence::parse(data.as_bytes()) {
                    Ok(p) if p.id == legacy_document_id => p,
                    _ => {
                        warnings.push(format!("A previous save for older {kind} draft ({legacy_id}) could not be copied; its original was kept."));
                        continue;
                    }
                };
                snapshot.id = project.id.clone();
                let bindings: Bindings = snapshot
                    .assets
                    .iter()
                    .map(|a| (a.id.clone(), root.clone()))
                    .collect();
                tx.execute("INSERT INTO workspace_video_history(workspace,project_id,revision,data,bindings,saved_at) VALUES(?1,?2,?3,?4,?5,?6)",params![workspace,project.id,revision,serde_json::to_string(&snapshot).map_err(|e|e.to_string())?,serde_json::to_string(&bindings).map_err(|e|e.to_string())?,saved_at]).map_err(|e|e.to_string())?;
            }

            tx.execute(
                "INSERT INTO workspace_video_migrations VALUES(?1,?2,?3,?4)",
                params![workspace, kind, legacy_id, project.id],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(warnings)
}
#[tauri::command]
pub async fn workspace_video_projects(workspace_id: String) -> Result<ProjectList, String> {
    tauri::async_runtime::spawn_blocking(move||{
        let roots=user_data::workspace_roots(&workspace_id)?;let db=database()?;let warnings=migrate_legacy(&db,&workspace_id,&roots)?;
        let mut q=db.prepare("SELECT id,title FROM workspace_video_projects WHERE workspace=?1 ORDER BY updated DESC,rowid DESC LIMIT 100").map_err(|e|e.to_string())?;
        let rows=q.query_map([workspace_id],|r|Ok(video_project::RecentProject{id:r.get(0)?,title:r.get(1)?})).map_err(|e|e.to_string())?;
        let projects=rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
        Ok(ProjectList{projects,warnings})
    }).await.map_err(|e|e.to_string())?
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    OpenSource {
        path: String,
    },
    Create {
        paths: Vec<String>,
    },
    Load {
        id: String,
    },
    Import {
        path: String,
    },
    Add {
        id: String,
        paths: Vec<String>,
        expected_revision: u64,
    },
    RemoveAsset {
        id: String,
        asset_id: String,
        expected_revision: u64,
    },
    Relink {
        id: String,
        asset_id: String,
        path: String,
        expected_revision: u64,
    },
    Recover {
        id: String,
        history_id: i64,
        expected_revision: u64,
    },
}
#[tauri::command]
pub async fn edit_workspace_video_project(
    app: tauri::AppHandle,
    workspace_id: String,
    action: Action,
) -> Result<Loaded, String> {
    let engine = video_engine::directory(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let roots = user_data::workspace_roots(&workspace_id)?;
        let db = database()?;
        let record = match action {
            Action::OpenSource { path } => {
                migrate_legacy(&db, &workspace_id, &roots)?;
                open_source(&db, &engine, &workspace_id, &roots, &path)?
            }
            Action::Create { paths } => {
                let mut record = new_record();
                add_files(&engine, &roots, &mut record, &paths)?;
                record.revision =
                    store(&db, &workspace_id, &record.project, &record.bindings, None)?;
                record
            }
            Action::Load { id } => read(&db, &workspace_id, &id)?,
            Action::Import { path } => {
                let mut bytes = vec![];
                fs::File::open(path)
                    .map_err(|e| e.to_string())?
                    .take(1024 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                let mut project = video_sequence::parse(&bytes)?;
                project.id = video_project::id();
                let bindings = bind_import(&roots, &project);
                let revision = store(&db, &workspace_id, &project, &bindings, None)?;
                Record {
                    project,
                    revision,
                    bindings,
                }
            }
            Action::Recover {
                id,
                history_id,
                expected_revision,
            } => recover(&db, &workspace_id, &id, history_id, expected_revision)?,
            action => {
                let (id, expected) = match &action {
                    Action::Add {
                        id,
                        expected_revision,
                        ..
                    }
                    | Action::RemoveAsset {
                        id,
                        expected_revision,
                        ..
                    }
                    | Action::Relink {
                        id,
                        expected_revision,
                        ..
                    } => (id, *expected_revision),
                    _ => unreachable!(),
                };
                let mut record = read(&db, &workspace_id, id)?;
                if record.revision != expected {
                    return Err(
                        "Project changed elsewhere. Reopen before changing its sources.".into(),
                    );
                }
                match action {
                    Action::Add { paths, .. } => add_files(&engine, &roots, &mut record, &paths)?,
                    Action::RemoveAsset { asset_id, .. } => {
                        record.project.clips.retain(|c| c.asset_id != asset_id);
                        record.project.assets.retain(|a| a.id != asset_id);
                        record.bindings.remove(&asset_id);
                    }
                    Action::Relink { asset_id, path, .. } => {
                        let (root, file) = root_for_file(&roots, &path)?;
                        let asset = record
                            .project
                            .assets
                            .iter_mut()
                            .find(|a| a.id == asset_id)
                            .ok_or("Video no longer in project")?;
                        asset.relative_path = file
                            .strip_prefix(&root)
                            .map_err(|e| e.to_string())?
                            .to_string_lossy()
                            .replace('\\', "/");
                        record
                            .bindings
                            .insert(asset_id.clone(), root.to_string_lossy().into_owned());
                        source(
                            &roots,
                            &record.project,
                            &record.bindings,
                            record
                                .project
                                .assets
                                .iter()
                                .find(|a| a.id == asset_id)
                                .unwrap(),
                        )?;
                    }
                    _ => unreachable!(),
                }
                record.revision = store(
                    &db,
                    &workspace_id,
                    &record.project,
                    &record.bindings,
                    Some(expected),
                )?;
                record
            }
        };
        Ok(loaded(&roots, record))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn save_workspace_video_project(
    workspace_id: String,
    project: Sequence,
    expected_revision: u64,
) -> Result<u64, String> {
    tauri::async_runtime::spawn_blocking(move || {
        user_data::workspace_roots(&workspace_id)?;
        let db = database()?;
        let before = read(&db, &workspace_id, &project.id)?;
        if before.project.assets != project.assets {
            return Err("Change source videos through Add, Remove or Relink".into());
        }
        store(
            &db,
            &workspace_id,
            &project,
            &before.bindings,
            Some(expected_revision),
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn workspace_video_history(
    workspace_id: String,
    id: String,
) -> Result<Vec<PreviousSave>, String> {
    tauri::async_runtime::spawn_blocking(move||{
        user_data::workspace_roots(&workspace_id)?;let db=database()?;read(&db,&workspace_id,&id)?;
        let mut q=db.prepare("SELECT id,revision,saved_at,data FROM workspace_video_history WHERE workspace=?1 AND project_id=?2 ORDER BY id DESC LIMIT 20").map_err(|e|e.to_string())?;
        let rows=q.query_map(params![workspace_id,id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,Option<u64>>(1)?,r.get::<_,Option<i64>>(2)?,r.get::<_,String>(3)?))).map_err(|e|e.to_string())?;
        rows.map(|row|{let(id,revision,saved_at,data)=row.map_err(|e|e.to_string())?;let p=video_sequence::parse(data.as_bytes())?;
            Ok(PreviousSave{id,revision,saved_at,title:p.title,clips:p.clips.len(),duration_seconds:p.clips.iter().map(|c|Ok(c.source_out.seconds()?-c.source_in.seconds()?)).collect::<Result<Vec<f64>,String>>()?.iter().sum()})
        }).collect()
    }).await.map_err(|e|e.to_string())?
}
fn single_for_render(
    db: &Connection,
    workspace: &str,
    roots: &[String],
    id: &str,
    expected: u64,
) -> Result<(String, video_project::Project), String> {
    let record = read(db, workspace, id)?;
    if record.revision != expected {
        return Err("Project changed elsewhere. Reopen before rendering.".into());
    }
    if record.project.clips.len() != 1 {
        return Err("Continuous output for multiple clips is the next rendering milestone".into());
    }
    let clip = record.project.clips[0].clone();
    let asset = record
        .project
        .assets
        .iter()
        .find(|a| a.id == clip.asset_id)
        .ok_or("Source no longer in project")?;
    let root = registered_root(
        roots,
        record
            .bindings
            .get(&asset.id)
            .ok_or("Relink the source video before rendering")?,
    )?;
    Ok((
        root.to_string_lossy().into_owned(),
        video_sequence::single(&record.project, asset, clip),
    ))
}
#[tauri::command]
pub fn start_workspace_video_preview(
    app: tauri::AppHandle,
    jobs: tauri::State<'_, crate::video_preview::PreviewJobs>,
    workspace_id: String,
    id: String,
    expected_revision: u64,
) -> Result<String, String> {
    let roots = user_data::workspace_roots(&workspace_id)?;
    let (root, project) =
        single_for_render(&database()?, &workspace_id, &roots, &id, expected_revision)?;
    crate::video_preview::start_video_project_preview(app, jobs, root, project)
}
#[tauri::command]
pub fn start_workspace_video_export(
    app: tauri::AppHandle,
    jobs: tauri::State<'_, crate::video_export::ExportJobs>,
    previews: tauri::State<'_, crate::video_preview::PreviewJobs>,
    workspace_id: String,
    id: String,
    expected_revision: u64,
    options: crate::video_export::Options,
) -> Result<String, String> {
    let roots = user_data::workspace_roots(&workspace_id)?;
    let (root, project) =
        single_for_render(&database()?, &workspace_id, &roots, &id, expected_revision)?;
    crate::video_export::start_video_export(app, jobs, previews, root, project, options)
}
fn recover(
    db: &Connection,
    workspace: &str,
    id: &str,
    history_id: i64,
    expected: u64,
) -> Result<Record, String> {
    let tx = Transaction::new_unchecked(db, TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let current = read(&tx, workspace, id)?;
    if current.revision != expected {
        return Err("Project changed elsewhere. Refresh history before recovering.".into());
    }
    let row:Option<(String,String)>=tx.query_row("SELECT data,bindings FROM workspace_video_history WHERE id=?1 AND workspace=?2 AND project_id=?3",params![history_id,workspace,id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;
    let (data, bindings) = row.ok_or("Previous save is no longer available in this project")?;
    let mut project = video_sequence::parse(data.as_bytes())?;
    if project.id != id {
        return Err("Previous save does not belong to this project".into());
    }
    project.id = video_project::id();
    project.title = project.title.chars().take(180).collect::<String>() + " (recovered copy)";
    let bindings =
        serde_json::from_str(&bindings).map_err(|_| "Invalid saved source references")?;
    let revision = store(&tx, workspace, &project, &bindings, None)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(Record {
        project,
        revision,
        bindings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, Record) {
        let root = std::env::temp_dir().join(format!("workspace-project-{}", video_project::id()));
        fs::create_dir(&root).unwrap();
        let file = root.join("source.mp4");
        fs::write(&file, b"original unchanged").unwrap();
        let info=video_probe::parse_probe(br#"{"streams":[{"index":0,"codec_type":"video","codec_name":"h264","width":96,"height":64,"time_base":"1/90000"}],"format":{"duration":"2"}}"#).unwrap();
        let project = video_sequence::migrate(
            video_project::new_project(root.to_str().unwrap(), &file, info).unwrap(),
        )
        .unwrap();
        let bindings = BTreeMap::from([(
            project.assets[0].id.clone(),
            root.to_string_lossy().into_owned(),
        )]);
        (
            root,
            Record {
                project,
                revision: 1,
                bindings,
            },
        )
    }
    fn db() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE projects(id TEXT PRIMARY KEY,root TEXT,title TEXT,revision INTEGER,data TEXT,updated INTEGER); CREATE TABLE project_history(id INTEGER PRIMARY KEY,project_id TEXT,revision INTEGER,saved_at INTEGER,data TEXT)").unwrap();
        initialize(&db).unwrap();
        db
    }
    #[test]
    fn workspace_ownership_survives_offline_sources_and_rejects_cross_workspace_and_stale_saves() {
        let (root, mut record) = fixture();
        let db = db();
        let id = record.project.id.clone();
        store(&db, "one", &record.project, &record.bindings, None).unwrap();
        assert!(read(&db, "two", &id).is_err());
        record.project.title = "Saved edit".into();
        assert_eq!(
            store(&db, "one", &record.project, &record.bindings, Some(1)).unwrap(),
            2
        );
        assert!(store(&db, "one", &record.project, &record.bindings, Some(1)).is_err());
        let restored = loaded(&[], read(&db, "one", &id).unwrap());
        assert_eq!(restored.project.title, "Saved edit");
        assert!(restored.sources[0].path.is_none());
        assert_eq!(
            fs::read(root.join("source.mp4")).unwrap(),
            b"original unchanged"
        );
        fs::remove_dir_all(root).unwrap();
        assert_eq!(read(&db, "one", &id).unwrap().project.title, "Saved edit");
    }
    #[test]
    fn render_snapshots_require_current_owner_revision_registered_source_and_one_clip() {
        let (root, mut record) = fixture();
        let db = db();
        let workspace = "one";
        let roots = vec![root.to_string_lossy().into_owned()];
        store(&db, workspace, &record.project, &record.bindings, None).unwrap();
        let id = record.project.id.clone();
        let (_, snapshot) = single_for_render(&db, workspace, &roots, &id, 1).unwrap();
        assert_eq!(snapshot.clips, record.project.clips);
        assert!(single_for_render(&db, "other", &roots, &id, 1).is_err());
        assert!(single_for_render(&db, workspace, &[], &id, 1).is_err());
        assert!(single_for_render(&db, workspace, &roots, &id, 2).is_err());
        record.project.clips.clear();
        store(&db, workspace, &record.project, &record.bindings, Some(1)).unwrap();
        assert!(single_for_render(&db, workspace, &roots, &id, 2).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn local_bindings_cannot_grant_access_outside_the_registered_workspace() {
        let (root, record) = fixture();
        let (other, _) = fixture();
        assert!(source(
            &[other.to_string_lossy().into_owned()],
            &record.project,
            &record.bindings,
            &record.project.assets[0]
        )
        .is_err());
        assert!(source(
            &[root.to_string_lossy().into_owned()],
            &record.project,
            &record.bindings,
            &record.project.assets[0]
        )
        .is_ok());
        fs::write(root.join("source.mp4"), b"different original").unwrap();
        assert!(source(
            &[root.to_string_lossy().into_owned()],
            &record.project,
            &record.bindings,
            &record.project.assets[0]
        )
        .is_err());
        assert!(root_for_file(
            &[other.to_string_lossy().into_owned()],
            root.join("source.mp4").to_str().unwrap()
        )
        .is_err());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(other).unwrap();
    }
    #[test]
    fn copies_both_legacy_formats_once_without_modifying_drafts_or_requiring_sources_to_load() {
        let (root, record) = fixture();
        let db = db();
        let roots = vec![root.to_string_lossy().into_owned()];
        let key = fs::canonicalize(&root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let legacy = video_sequence::single(
            &record.project,
            &record.project.assets[0],
            record.project.clips[0].clone(),
        );
        let single = serde_json::to_string(&legacy).unwrap();
        let multi = serde_json::to_string(&record.project).unwrap();
        db.execute(
            "INSERT INTO projects VALUES('old-single',?1,'legacy',7,?2,0)",
            params![key, single],
        )
        .unwrap();
        db.execute(
            "INSERT INTO video_sequences VALUES('old-multi',?1,'legacy',9,?2,0)",
            params![key, multi],
        )
        .unwrap();
        db.execute(
            "INSERT INTO projects VALUES('unreadable',?1,'kept',1,'invalid-json',0)",
            [key],
        )
        .unwrap();
        db.execute("INSERT INTO project_history(project_id,revision,saved_at,data) VALUES('old-single',NULL,NULL,?1)",[&single]).unwrap();
        db.execute(
            "INSERT INTO video_sequence_history(project_id,revision,data) VALUES('old-multi',8,?1)",
            [&multi],
        )
        .unwrap();
        let warnings = migrate_legacy(&db, "workspace", &roots).unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("kept unchanged"));
        let count: i64 = db
            .query_row("SELECT count(*) FROM workspace_video_history", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 2);
        let (history_id,project_id):(i64,String)=db.query_row("SELECT id,project_id FROM workspace_video_history WHERE revision IS NULL AND saved_at IS NULL",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        let branch = recover(&db, "workspace", &project_id, history_id, 1).unwrap();
        assert_eq!(branch.project.clips, record.project.clips);
        db.execute(
            "DELETE FROM workspace_video_projects WHERE id=?1",
            [branch.project.id],
        )
        .unwrap();

        migrate_legacy(&db, "workspace", &roots).unwrap();
        let count: i64 = db
            .query_row("SELECT count(*) FROM workspace_video_projects", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 2);
        let unchanged: String = db
            .query_row("SELECT data FROM projects WHERE id='old-single'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(unchanged, single);
        let unchanged: String = db
            .query_row(
                "SELECT data FROM video_sequences WHERE id='old-multi'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(unchanged, multi);
        migrate_legacy(&db, "unrelated", &[]).unwrap();
        let count: i64 = db
            .query_row(
                "SELECT count(*) FROM workspace_video_projects WHERE workspace='unrelated'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bounded_history_recovers_as_a_new_branch_and_rejects_wrong_owner_project_and_revision() {
        let (root, mut record) = fixture();
        let db = db();
        let id = record.project.id.clone();
        store(&db, "one", &record.project, &record.bindings, None).unwrap();
        for revision in 1..=25 {
            record.project.title = format!("Edit {revision}");
            store(
                &db,
                "one",
                &record.project,
                &record.bindings,
                Some(revision),
            )
            .unwrap();
        }
        let (count, history_id): (i64, i64) = db
            .query_row(
                "SELECT count(*),max(id) FROM workspace_video_history",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(count, 20);
        assert!(recover(&db, "two", &id, history_id, 26).is_err());
        assert!(recover(&db, "one", &id, history_id, 25).is_err());
        let mut other = record.project.clone();
        other.id = video_project::id();
        store(&db, "one", &other, &record.bindings, None).unwrap();
        assert!(recover(&db, "one", &other.id, history_id, 1).is_err());
        let restored = recover(&db, "one", &id, history_id, 26).unwrap();
        assert_ne!(restored.project.id, id);
        assert_eq!(restored.project.title, "Edit 24 (recovered copy)");
        assert_eq!(restored.bindings, record.bindings);
        assert_eq!(read(&db, "one", &id).unwrap().project.title, "Edit 25");
        assert_eq!(restored.revision, 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn short_cross_folder_imports_bind_each_source_deduplicate_and_rollback_a_bad_batch() {
        use std::{ffi::OsString, time::Duration};
        let (root, mut record) = fixture();
        let (second, _) = fixture();
        let engine = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/video-engine");
        let ffmpeg = video_engine::tool(&engine, "ffmpeg").unwrap();
        let mut paths = vec![];
        for (folder, color) in [(&root, "red"), (&second, "blue")] {
            let path = folder.join("short video.mp4");
            let mut args: Vec<OsString> = [
                "-nostdin",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                &format!("color={color}:size=96x64:rate=30:duration=0.2"),
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
                &ffmpeg,
                &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                Duration::from_secs(30),
            )
            .unwrap();
            paths.push(path.to_string_lossy().into_owned());
        }
        let roots = vec![
            root.to_string_lossy().into_owned(),
            second.to_string_lossy().into_owned(),
        ];
        record.project.assets.clear();
        record.project.clips.clear();
        record.bindings.clear();
        add_files(&engine, &roots, &mut record, &paths).unwrap();
        assert_eq!(record.project.assets.len(), 2);
        for asset in &record.project.assets {
            assert!(source(&roots, &record.project, &record.bindings, asset).is_ok());
        }
        let project = record.project.clone();
        let bindings = record.bindings.clone();
        add_files(&engine, &roots, &mut record, &paths).unwrap();
        assert_eq!(record.project, project);
        assert!(add_files(
            &engine,
            &roots,
            &mut record,
            &[
                paths[0].clone(),
                root.join("source.mp4").to_string_lossy().into_owned()
            ]
        )
        .is_err());
        assert_eq!(record.project, project);
        assert_eq!(record.bindings, bindings);
        let db = db();
        store(&db, "workspace", &record.project, &record.bindings, None).unwrap();
        let reopened = loaded(&roots, read(&db, "workspace", &project.id).unwrap());
        assert!(reopened.sources.iter().all(|s| s.path.is_some()));
        assert_ne!(reopened.sources[0].root, reopened.sources[1].root);
        let portable = serde_json::to_vec(&project).unwrap();
        assert!(!String::from_utf8_lossy(&portable).contains("workspace-project-"));
        let imported = video_sequence::parse(&portable).unwrap();
        let resolved = bind_import(&roots, &imported);
        assert_eq!(resolved.len(), 2);
        for asset in &imported.assets {
            assert!(source(&roots, &imported, &resolved, asset).is_ok());
        }
        assert!(bind_import(&[], &imported).is_empty());

        let mut one = open_source(&db, &engine, "workspace", &roots, &paths[0]).unwrap();
        one.project.clips[0].edits.gain_percent = 75;
        store(&db, "workspace", &one.project, &one.bindings, Some(1)).unwrap();
        let resumed = open_source(&db, &engine, "workspace", &roots, &paths[0]).unwrap();
        assert_eq!(resumed.project.id, one.project.id);
        assert_eq!(resumed.project.clips[0].edits.gain_percent, 75);
        let moved = second.join("moved short.mp4");
        fs::copy(&paths[0], &moved).unwrap();
        let relinked =
            open_source(&db, &engine, "workspace", &roots, moved.to_str().unwrap()).unwrap();
        assert_eq!(relinked.project.id, one.project.id);
        assert_eq!(relinked.revision, 3);
        let (render_root, snapshot) =
            single_for_render(&db, "workspace", &roots, &one.project.id, 3).unwrap();
        assert_eq!(
            fs::canonicalize(render_root).unwrap(),
            fs::canonicalize(&second).unwrap()
        );
        assert_eq!(snapshot.clips[0].edits.gain_percent, 75);

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(second).unwrap();
    }
}
