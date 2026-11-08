//! Workspace-owned transition proposals. These never alter v3 projects or grant source access.
//! Immutable revisions and explicit rebase/recovery keep old clients from silently losing edits.
use super::*;
use crate::crossfade_clock::Boundary;
use sha2::{Digest, Sha256};
include!("workspace_transition_render.rs");

const SAFE_REVISION: u64 = 9_007_199_254_740_991;
const MAX_RECORD: usize = 131_072;
pub(super) const MAX_PORTABLE: usize = 2 * 1024 * 1024;
const OBJECTS: &[(&str, &str)] = &[
    ("workspace_transition_bases", "CREATE TABLE workspace_transition_bases(workspace TEXT NOT NULL,project_id TEXT NOT NULL,hash TEXT NOT NULL,data TEXT NOT NULL,PRIMARY KEY(workspace,project_id,hash))"),
    ("workspace_transition_base_no_update", "CREATE TRIGGER workspace_transition_base_no_update BEFORE UPDATE ON workspace_transition_bases BEGIN SELECT RAISE(ABORT,'Transition bases are immutable'); END"),
    ("workspace_transition_base_no_delete", "CREATE TRIGGER workspace_transition_base_no_delete BEFORE DELETE ON workspace_transition_bases BEGIN SELECT RAISE(ABORT,'Transition bases are immutable'); END"),
    ("workspace_transition_heads", "CREATE TABLE workspace_transition_heads(workspace TEXT NOT NULL,project_id TEXT NOT NULL,revision INTEGER NOT NULL CHECK(revision>0),PRIMARY KEY(workspace,project_id))"),
    ("workspace_transition_revisions", "CREATE TABLE workspace_transition_revisions(workspace TEXT NOT NULL,project_id TEXT NOT NULL,revision INTEGER NOT NULL CHECK(revision>0),data TEXT NOT NULL,PRIMARY KEY(workspace,project_id,revision))"),
    ("workspace_transition_no_update", "CREATE TRIGGER workspace_transition_no_update BEFORE UPDATE ON workspace_transition_revisions BEGIN SELECT RAISE(ABORT,'Transition history is immutable'); END"),
    ("workspace_transition_no_delete", "CREATE TRIGGER workspace_transition_no_delete BEFORE DELETE ON workspace_transition_revisions BEGIN SELECT RAISE(ABORT,'Transition history is immutable'); END"),
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectReceipt {
    pub revision: u64,
    pub hash: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    pub from_clip_id: String,
    pub to_clip_id: String,
    pub frames: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionRecord {
    pub schema_version: u32,
    pub timing_policy: u32,
    pub workspace: String,
    pub project_id: String,
    pub revision: u64,
    pub project: ProjectReceipt,
    pub edges: Vec<Edge>,
    pub recovered_from: Option<u64>,
}
#[derive(Serialize)]
pub struct HistoryEntry {
    revision: u64,
    supported: bool,
}
#[derive(Serialize)]
pub struct State {
    project: ProjectReceipt,
    revision: u64,
    record: Option<TransitionRecord>,
    stale: bool,
    history: Vec<HistoryEntry>,
    timing: Option<crate::crossfade_layer_clock::Plan>,
    seek: Option<crate::sequence_frame_seek::FrameSeek>,
    portable: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableTransitions {
    schema_version: u32,
    timing_policy: u32,
    project_json: String,
    edges: Vec<Edge>,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    ExportHistory {
        id: String,
        expected_project: ProjectReceipt,
        expected_revision: u64,
        history_revision: u64,
    },
    ImportFile {
        id: String,
        expected_project: ProjectReceipt,
        expected_revision: u64,
        path: String,
        rebase: bool,
    },
    Export {
        id: String,
        expected_project: ProjectReceipt,
        expected_revision: u64,
    },
    Import {
        id: String,
        expected_project: ProjectReceipt,
        expected_revision: u64,
        document: String,
        rebase: bool,
    },
    Seek {
        id: String,
        expected_project: ProjectReceipt,
        expected_revision: u64,
        point: crate::sequence_frame_seek::SeekPoint,
    },
    Load {
        id: String,
    },
    Save {
        id: String,
        expected_project: ProjectReceipt,
        expected_revision: u64,
        edges: Vec<Edge>,
        rebase: bool,
    },
    History {
        id: String,
        before_revision: Option<u64>,
        limit: u8,
    },
    Recover {
        id: String,
        expected_project: ProjectReceipt,
        expected_revision: u64,
        history_revision: u64,
        rebase: bool,
    },
}
fn store_schema_present(tx: &Connection) -> Result<bool, String> {
    // Inspect every existing object before making any schema changes. Unknown
    // schemas and future records are preserved, never rebuilt or normalized.
    let mut existing = 0;
    for (name, expected) in OBJECTS {
        let actual: Option<String> = tx
            .query_row("SELECT sql FROM sqlite_master WHERE name=?1", [name], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|e| e.to_string())?;
        if actual.as_deref().is_some_and(|sql| sql != *expected) {
            return Err("Unsupported transition storage schema; preserve this database and use a compatible version".into());
        }
        existing += usize::from(actual.is_some());
    }
    if existing != 0 && existing != OBJECTS.len() {
        return Err("Transition storage is incomplete; preserve it for explicit recovery".into());
    }
    Ok(existing == OBJECTS.len())
}
fn initialize_store_in(tx: &Connection) -> Result<(), String> {
    if !store_schema_present(tx)? {
        for (_, sql) in OBJECTS {
            tx.execute_batch(sql).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
fn project_receipt(
    db: &Connection,
    workspace: &str,
    id: &str,
) -> Result<(Sequence, ProjectReceipt), String> {
    let (data, revision): (String, u64) = db
        .query_row(
            "SELECT data,revision FROM workspace_video_projects WHERE workspace=?1 AND id=?2",
            params![workspace, id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| "Project not found in this workspace")?;
    let project = video_sequence::parse(data.as_bytes())?;
    if project.id != id || !(1..=SAFE_REVISION).contains(&revision) {
        return Err("Invalid transition project identity or revision".into());
    }
    Ok((
        project,
        ProjectReceipt {
            revision,
            hash: format!("{:x}", Sha256::digest(data.as_bytes())),
        },
    ))
}
fn decode_record(
    raw: &str,
    workspace: &str,
    id: &str,
    revision: u64,
) -> Result<TransitionRecord, String> {
    if raw.len() > MAX_RECORD {
        return Err("Transition record exceeds its size budget".into());
    }
    let record: TransitionRecord = serde_json::from_str(raw)
        .map_err(|_| "Unsupported or corrupt transition record; preserve the stored data")?;
    if record.schema_version != 1
        || record.timing_policy != 1
        || record.workspace != workspace
        || record.project_id != id
        || record.revision != revision
        || !(1..=SAFE_REVISION).contains(&revision)
        || !(1..=SAFE_REVISION).contains(&record.project.revision)
        || record.project.hash.len() != 64
        || !record.project.hash.bytes().all(|b| b.is_ascii_hexdigit())
        || record.edges.len() > 199
        || record
            .recovered_from
            .is_some_and(|r| r == 0 || r >= revision)
        || record.edges.iter().any(|e| {
            !video_project::valid_id(&e.from_clip_id)
                || !video_project::valid_id(&e.to_clip_id)
                || e.frames == 0
                || e.frames > SAFE_REVISION
        })
    {
        return Err("Unsupported or corrupt transition record; preserve the stored data".into());
    }
    Ok(record)
}
fn record_at(
    db: &Connection,
    workspace: &str,
    id: &str,
    revision: u64,
) -> Result<TransitionRecord, String> {
    let raw: String = db.query_row("SELECT data FROM workspace_transition_revisions WHERE workspace=?1 AND project_id=?2 AND revision=?3",params![workspace,id,revision],|r|r.get(0)).map_err(|_| "Transition history entry is unavailable")?;
    let record = decode_record(&raw, workspace, id, revision)?;
    let base = base_document(db, &record)?;
    proposal_timing(&video_sequence::parse(base.as_bytes())?, &record.edges)?;
    Ok(record)
}
fn base_document(db: &Connection, record: &TransitionRecord) -> Result<String, String> {
    let raw:String=db.query_row("SELECT data FROM workspace_transition_bases WHERE workspace=?1 AND project_id=?2 AND hash=?3",params![record.workspace,record.project_id,record.project.hash],|r|r.get(0)).map_err(|_|"The immutable source timeline for this proposal is unavailable; preserve the database")?;
    if digest(&raw) != record.project.hash
        || video_sequence::parse(raw.as_bytes())?.id != record.project_id
    {
        return Err(
            "The immutable transition source timeline changed; preserve the database".into(),
        );
    }
    Ok(raw)
}
fn head(db: &Connection, workspace: &str, id: &str) -> Result<Option<TransitionRecord>, String> {
    let revision: Option<u64> = db
        .query_row(
            "SELECT revision FROM workspace_transition_heads WHERE workspace=?1 AND project_id=?2",
            params![workspace, id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    revision
        .map(|r| record_at(db, workspace, id, r))
        .transpose()
}
fn timing(
    project: &Sequence,
    edges: &[Edge],
) -> Result<crate::crossfade_layer_clock::Plan, String> {
    crate::crossfade_layer_clock::plan(
        project,
        &edges
            .iter()
            .map(|e| Boundary {
                from_clip_id: &e.from_clip_id,
                to_clip_id: &e.to_clip_id,
                frames: e.frames,
            })
            .collect::<Vec<_>>(),
    )
}
fn proposal_timing(
    project: &Sequence,
    edges: &[Edge],
) -> Result<Option<crate::crossfade_layer_clock::Plan>, String> {
    // An empty source timeline must still be able to clear its old proposal.
    if edges.is_empty() && project.clips.is_empty() {
        return Ok(None);
    }
    timing(project, edges).map(Some)
}
fn state(db: &Connection, workspace: &str, id: &str) -> Result<State, String> {
    let (project, receipt) = project_receipt(db, workspace, id)?;
    let record = head(db, workspace, id)?;
    let stale = record.as_ref().is_some_and(|r| r.project != receipt);
    let plan = if stale {
        None
    } else {
        proposal_timing(
            &project,
            record.as_ref().map_or(&[], |r| r.edges.as_slice()),
        )?
    };
    Ok(State {
        project: receipt,
        revision: record.as_ref().map_or(0, |r| r.revision),
        record,
        stale,
        history: vec![],
        timing: plan,
        seek: None,
        portable: None,
    })
}
fn edit_in(db: &Connection, workspace: &str, action: Action) -> Result<State, String> {
    let id = match &action {
        Action::Export { id, .. }
        | Action::ExportHistory { id, .. }
        | Action::ImportFile { id, .. }
        | Action::Import { id, .. }
        | Action::Seek { id, .. }
        | Action::Load { id }
        | Action::Save { id, .. }
        | Action::History { id, .. }
        | Action::Recover { id, .. } => id.clone(),
    };
    if workspace.is_empty() || workspace.len() > 120 || !video_project::valid_id(&id) {
        return Err("Invalid transition ownership".into());
    }
    let mut current = state(db, workspace, &id)?;
    let (expected_project, expected_revision, mut edges, rebase, recovered_from) = match action {
        Action::ExportHistory {
            expected_project,
            expected_revision,
            history_revision,
            ..
        } => {
            if expected_project != current.project || expected_revision != current.revision {
                return Err(
                    "Project or proposal changed; refresh history before recovering".into(),
                );
            }
            let old = record_at(db, workspace, &id, history_revision)?;
            current.portable = Some(
                serde_json::to_string(&PortableTransitions {
                    schema_version: 1,
                    timing_policy: 1,
                    project_json: base_document(db, &old)?,
                    edges: old.edges,
                })
                .map_err(|e| e.to_string())?,
            );
            return Ok(current);
        }
        Action::ImportFile { .. } => {
            return Err("Read the proposal file before opening the edit transaction".into())
        }
        Action::Export {
            expected_project,
            expected_revision,
            ..
        } => {
            if expected_project != current.project
                || expected_revision != current.revision
                || current.stale
            {
                return Err("Project or transition draft changed; reload or explicitly rebase before exporting".into());
            }
            let raw: String = db
                .query_row(
                    "SELECT data FROM workspace_video_projects WHERE workspace=?1 AND id=?2",
                    params![workspace, id],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            let portable = PortableTransitions {
                schema_version: 1,
                timing_policy: 1,
                project_json: raw,
                edges: current
                    .record
                    .as_ref()
                    .map_or_else(Vec::new, |r| r.edges.clone()),
            };
            let encoded = serde_json::to_string(&portable).map_err(|e| e.to_string())?;
            if encoded.len() > MAX_PORTABLE {
                return Err("Portable transition document exceeds its size budget".into());
            }
            current.portable = Some(encoded);
            return Ok(current);
        }
        Action::Import {
            expected_project,
            expected_revision,
            document,
            rebase,
            ..
        } => {
            let portable = parse_portable(&document)?;
            let hash = format!("{:x}", Sha256::digest(portable.project_json.as_bytes()));
            if hash != current.project.hash && !rebase {
                return Err("Imported transitions belong to a different project snapshot; explicit rebase is required".into());
            }
            (
                expected_project,
                expected_revision,
                portable.edges,
                rebase,
                None,
            )
        }
        Action::Seek {
            expected_project,
            expected_revision,
            point,
            ..
        } => {
            if expected_project != current.project
                || expected_revision != current.revision
                || current.stale
            {
                return Err("Project or transition draft changed; reload before seeking".into());
            }
            current.seek = Some(crate::sequence_frame_seek::seek(
                current
                    .timing
                    .as_ref()
                    .ok_or("Transition clock is unavailable")?,
                point,
            )?);
            return Ok(current);
        }
        Action::Load { .. } => return Ok(current),
        Action::History {
            before_revision,
            limit,
            ..
        } => {
            if !(1..=100).contains(&limit)
                || before_revision.is_some_and(|r| r == 0 || r > SAFE_REVISION)
            {
                return Err("Invalid transition history page".into());
            }
            let mut query=db.prepare("SELECT revision,data FROM workspace_transition_revisions WHERE workspace=?1 AND project_id=?2 AND revision<?3 ORDER BY revision DESC LIMIT ?4").map_err(|e|e.to_string())?;
            let rows = query
                .query_map(
                    params![
                        workspace,
                        id,
                        before_revision.unwrap_or(SAFE_REVISION + 1),
                        limit
                    ],
                    |r| Ok((r.get::<_, u64>(0)?, r.get::<_, String>(1)?)),
                )
                .map_err(|e| e.to_string())?;
            for row in rows {
                let (revision, raw) = row.map_err(|e| e.to_string())?;
                current.history.push(HistoryEntry {
                    revision,
                    supported: decode_record(&raw, workspace, &id, revision)
                        .and_then(|record| {
                            base_document(db, &record).and_then(|base| {
                                proposal_timing(
                                    &video_sequence::parse(base.as_bytes())?,
                                    &record.edges,
                                )
                                .map(|_| ())
                            })
                        })
                        .is_ok(),
                });
            }
            return Ok(current);
        }
        Action::Save {
            expected_project,
            expected_revision,
            edges,
            rebase,
            ..
        } => (expected_project, expected_revision, edges, rebase, None),
        Action::Recover {
            expected_project,
            expected_revision,
            history_revision,
            rebase,
            ..
        } => {
            if history_revision == 0 || history_revision > expected_revision {
                return Err("Choose an existing transition history revision".into());
            }
            let old = record_at(db, workspace, &id, history_revision)?;
            if old.project != current.project && !rebase {
                return Err(
                    "Project changed since this history entry; explicit rebase is required".into(),
                );
            }
            (
                expected_project,
                expected_revision,
                old.edges,
                rebase,
                Some(history_revision),
            )
        }
    };
    if expected_revision != current.revision || expected_project != current.project {
        return Err("Project or transition draft changed; reload before saving".into());
    }
    if current.stale && !rebase {
        return Err(
            "Project changed since this transition draft; explicit rebase is required".into(),
        );
    }
    let (project, _) = project_receipt(db, workspace, &id)?;
    let plan = proposal_timing(&project, &edges)?;
    // Canonical timeline order makes retries/no-op comparisons deterministic.
    edges.sort_by_key(|e| {
        project
            .clips
            .iter()
            .position(|c| c.id == e.from_clip_id)
            .unwrap_or(usize::MAX)
    });
    if recovered_from.is_none()
        && current
            .record
            .as_ref()
            .is_some_and(|r| r.project == current.project && r.edges == edges)
    {
        return Ok(current);
    }
    let revision = current
        .revision
        .checked_add(1)
        .filter(|r| *r <= SAFE_REVISION)
        .ok_or("Transition revision limit reached")?;
    let record = TransitionRecord {
        schema_version: 1,
        timing_policy: 1,
        workspace: workspace.into(),
        project_id: id.clone(),
        revision,
        project: current.project.clone(),
        edges,
        recovered_from,
    };
    let raw = serde_json::to_string(&record).map_err(|e| e.to_string())?;
    decode_record(&raw, workspace, &id, revision)?;
    let base: String = db
        .query_row(
            "SELECT data FROM workspace_video_projects WHERE workspace=?1 AND id=?2",
            params![workspace, id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    db.execute(
        "INSERT INTO workspace_transition_bases VALUES(?1,?2,?3,?4) ON CONFLICT DO NOTHING",
        params![workspace, id, current.project.hash, base],
    )
    .map_err(|e| e.to_string())?;
    if base_document(db, &record)? != base {
        return Err("Transition source snapshot conflicts with its stored receipt".into());
    }
    db.execute(
        "INSERT INTO workspace_transition_revisions VALUES(?1,?2,?3,?4)",
        params![workspace, id, revision, raw],
    )
    .map_err(|e| e.to_string())?;
    if current.revision==0 {
        db.execute("INSERT INTO workspace_transition_heads VALUES(?1,?2,?3)",params![workspace,id,revision]).map_err(|e|e.to_string())?;
    } else if db.execute("UPDATE workspace_transition_heads SET revision=?1 WHERE workspace=?2 AND project_id=?3 AND revision=?4",params![revision,workspace,id,current.revision]).map_err(|e|e.to_string())?!=1 {
        return Err("Transition draft changed; reload before saving".into());
    }
    if head(db, workspace, &id)?.as_ref() != Some(&record)
        || project_receipt(db, workspace, &id)?.1 != current.project
    {
        return Err("Transition publication changed unexpectedly; the edit was rolled back".into());
    }
    current.revision = revision;
    current.record = Some(record);
    current.stale = false;
    current.timing = plan;
    Ok(current)
}
fn edit(
    settings: &Connection,
    db: &Connection,
    workspace: &str,
    action: Action,
) -> Result<State, String> {
    let roots = user_data::workspace_roots_from(settings, workspace)?;
    let action = match action {
        Action::ImportFile {
            id,
            expected_project,
            expected_revision,
            path,
            rebase,
        } => {
            let path = Path::new(&path);
            if !path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
            {
                return Err("Choose a JSON crossfade proposal".into());
            }
            let mut bytes = vec![];
            fs::File::open(path)
                .map_err(|e| e.to_string())?
                .take((MAX_PORTABLE + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > MAX_PORTABLE {
                return Err("Portable transition document exceeds its size budget".into());
            }
            Action::Import {
                id,
                expected_project,
                expected_revision,
                document: String::from_utf8(bytes)
                    .map_err(|_| "Crossfade proposal is not valid UTF-8")?,
                rebase,
            }
        }
        action => action,
    };
    let tx = Transaction::new_unchecked(db, TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    initialize_store_in(&tx)?;
    let result = edit_in(&tx, workspace, action)?;
    if user_data::workspace_roots_from(settings, workspace)? != roots {
        return Err("Workspace changed during transition edit".into());
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(result)
}
fn parse_portable(document: &str) -> Result<PortableTransitions, String> {
    if document.len() > MAX_PORTABLE {
        return Err("Portable transition document exceeds its size budget".into());
    }
    let portable: PortableTransitions = serde_json::from_str(document)
        .map_err(|_| "Unsupported portable transition document; preserve the original file")?;
    if portable.schema_version != 1 || portable.timing_policy != 1 {
        return Err("Unsupported portable transition version; preserve the original file".into());
    }
    let project = video_sequence::parse(portable.project_json.as_bytes())?;
    proposal_timing(&project, &portable.edges)?;
    Ok(portable)
}
pub(super) fn is_portable_project(bytes: &[u8]) -> Result<bool, String> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| "Project file is not valid JSON")?;
    Ok(value.get("project_json").is_some() || value.get("timing_policy").is_some())
}
pub(super) fn has_saved_proposal(
    db: &Connection,
    workspace: &str,
    id: &str,
) -> Result<bool, String> {
    if !store_schema_present(db)? {
        return Ok(false);
    }
    Ok(head(db, workspace, id)?.is_some())
}
pub(super) fn check_namespace(db: &Connection) -> Result<(), String> {
    store_schema_present(db).map(|_| ())
}
pub(super) fn import_project_copy(
    settings: &Connection,
    db: &Connection,
    workspace: &str,
    roots: &[String],
    bytes: &[u8],
) -> Result<Record, String> {
    let document =
        std::str::from_utf8(bytes).map_err(|_| "Crossfade project is not valid UTF-8")?;
    let portable = parse_portable(document)?;
    let mut project = video_sequence::parse(portable.project_json.as_bytes())?;
    project.id = video_project::id();
    let bindings = super::bind_import(roots, &project);
    let tx = Transaction::new_unchecked(db, TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    initialize_store_in(&tx)?;
    let revision = super::store(&tx, workspace, &project, &bindings, None)?;
    let (_, receipt) = project_receipt(&tx, workspace, &project.id)?;
    edit_in(
        &tx,
        workspace,
        Action::Save {
            id: project.id.clone(),
            expected_project: receipt,
            expected_revision: 0,
            edges: portable.edges,
            rebase: false,
        },
    )?;
    if user_data::workspace_roots_from(settings, workspace)? != roots {
        return Err("Workspace changed during project import".into());
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(Record {
        project,
        revision,
        bindings,
    })
}
fn recover_project_copy(
    settings: &Connection,
    db: &Connection,
    request: TransitionRenderRequest,
    history_revision: u64,
) -> Result<Loaded, String> {
    let state = edit(
        settings,
        db,
        &request.workspace_id,
        Action::ExportHistory {
            id: request.id,
            expected_project: request.expected_project,
            expected_revision: request.expected_revision,
            history_revision,
        },
    )?;
    let mut portable = parse_portable(
        state
            .portable
            .as_deref()
            .ok_or("Historical project copy is unavailable")?,
    )?;
    let mut project = video_sequence::parse(portable.project_json.as_bytes())?;
    project.title = project.title.chars().take(80).collect::<String>() + " (crossfade recovery)";
    portable.project_json = serde_json::to_string(&project).map_err(|e| e.to_string())?;
    let roots = user_data::workspace_roots_from(settings, &request.workspace_id)?;
    let record = import_project_copy(
        settings,
        db,
        &request.workspace_id,
        &roots,
        serde_json::to_string(&portable)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )?;
    let mut loaded = super::loaded(&roots, record);
    loaded.transition_proposal = true;
    Ok(loaded)
}
#[tauri::command]
pub async fn recover_workspace_transition_project(
    request: TransitionRenderRequest,
    history_revision: u64,
) -> Result<Loaded, String> {
    tauri::async_runtime::spawn_blocking(move || {
        recover_project_copy(
            &user_data::database()?,
            &database()?,
            request,
            history_revision,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
fn write_portable_copy(document: &str, destination: &Path) -> Result<(), String> {
    parse_portable(document)?;
    if !destination
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
    {
        return Err("Choose a new .json filename for the crossfade proposal".into());
    }
    let mut output = crate::safe_output::StagedOutput::new(destination)?;
    use std::io::Write;
    output
        .file()
        .write_all(document.as_bytes())
        .map_err(|e| e.to_string())?;
    output.commit()?;
    Ok(())
}
#[tauri::command]
pub async fn write_transition_proposal_copy(
    document: String,
    destination: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        write_portable_copy(&document, Path::new(&destination))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn edit_workspace_video_transitions(
    workspace_id: String,
    action: Action,
) -> Result<State, String> {
    tauri::async_runtime::spawn_blocking(move || {
        edit(&user_data::database()?, &database()?, &workspace_id, action)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
#[path = "workspace_transitions_tests.rs"]
mod tests;
