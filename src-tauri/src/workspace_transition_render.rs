// Native-only render authorization. Portable documents never supply these paths or grants.
#[derive(Clone)]
struct RenderContext {
    workspace: String,
    project_id: String,
    project: ProjectReceipt,
    revision: u64,
    roots: Vec<String>,
    resolved_roots: BTreeMap<String, String>,
    bindings_hash: String,
    record_hash: String,
    settings_path: PathBuf,
    projects_path: PathBuf,
}
#[derive(Clone)]
pub(crate) struct RenderBinding {
    pub(crate) edges: Vec<Edge>,
    context: Option<RenderContext>,
}
fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn binding_hashes(
    db: &Connection,
    workspace: &str,
    id: &str,
    revision: u64,
) -> Result<(String, String), String> {
    let bindings: String = db
        .query_row(
            "SELECT bindings FROM workspace_video_projects WHERE workspace=?1 AND id=?2",
            params![workspace, id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let record = if revision == 0 {
        String::new()
    } else {
        db.query_row("SELECT data FROM workspace_transition_revisions WHERE workspace=?1 AND project_id=?2 AND revision=?3",params![workspace,id,revision],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?
    };
    Ok((digest(&bindings), digest(&record)))
}
impl RenderBinding {
    #[cfg(test)]
    pub(crate) fn fixture(edges: Vec<Edge>) -> Self {
        Self {
            edges,
            context: None,
        }
    }
    pub(crate) fn current(&self) -> Result<bool, String> {
        let Some(context) = &self.context else {
            #[cfg(test)]
            {
                return Ok(true);
            }
            #[cfg(not(test))]
            {
                return Err("Missing native transition authorization".into());
            }
        };
        let flags =
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let settings = Connection::open_with_flags(&context.settings_path, flags)
            .map_err(|e| e.to_string())?;
        let projects = Connection::open_with_flags(&context.projects_path, flags)
            .map_err(|e| e.to_string())?;
        let tx = Transaction::new_unchecked(&projects, TransactionBehavior::Deferred)
            .map_err(|e| e.to_string())?;
        let roots = user_data::workspace_roots_from(&settings, &context.workspace)?;
        // Schema validation is read-only here. Never recreate objects during delivery.
        if !store_schema_present(&tx)? {
            return Err("Transition storage changed during rendering".into());
        }
        let fresh = state(&tx, &context.workspace, &context.project_id)?;
        if roots != context.roots
            || fresh.project != context.project
            || fresh.revision != context.revision
            || fresh.stale
            || fresh
                .record
                .as_ref()
                .map_or(&[] as &[Edge], |r| r.edges.as_slice())
                != self.edges.as_slice()
        {
            return Ok(false);
        }
        let hashes = binding_hashes(
            &tx,
            &context.workspace,
            &context.project_id,
            context.revision,
        )?;
        if hashes != (context.bindings_hash.clone(), context.record_hash.clone()) {
            return Ok(false);
        }
        let record = super::read(&tx, &context.workspace, &context.project_id)?;
        tx.commit().map_err(|e| e.to_string())?;
        if super::resolve_snapshot(record, &roots)?.roots != context.resolved_roots {
            return Ok(false);
        }
        Ok(user_data::workspace_roots_from(&settings, &context.workspace)? == roots)
    }
}
fn render_transition_snapshot(
    settings: &Connection,
    projects: &Connection,
    workspace: &str,
    id: &str,
    expected_project: &ProjectReceipt,
    expected_revision: u64,
    cancel: impl Fn() -> bool,
) -> Result<crate::video_sequence_render::Snapshot, String> {
    let tx = Transaction::new_unchecked(projects, TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    initialize_store_in(&tx)?;
    let roots = user_data::workspace_roots_from(settings, workspace)?;
    let current = state(&tx, workspace, id)?;
    if current.project != *expected_project
        || current.revision != expected_revision
        || current.stale
    {
        return Err("Project or transitions changed; reload before rendering".into());
    }
    let (bindings_hash, record_hash) = binding_hashes(&tx, workspace, id, current.revision)?;
    let record = super::read(&tx, workspace, id)?;
    let mut context = RenderContext {
        workspace: workspace.into(),
        project_id: id.into(),
        project: current.project,
        revision: current.revision,
        roots: roots.clone(),
        resolved_roots: BTreeMap::new(),
        bindings_hash,
        record_hash,
        settings_path: fs::canonicalize(
            settings
                .path()
                .ok_or("Transition rendering requires durable settings")?,
        )
        .map_err(|e| e.to_string())?,
        projects_path: fs::canonicalize(
            projects
                .path()
                .ok_or("Transition rendering requires durable projects")?,
        )
        .map_err(|e| e.to_string())?,
    };
    tx.commit().map_err(|e| e.to_string())?;
    let mut snapshot = super::resolve_snapshot(record, &roots)?;
    context.resolved_roots = snapshot.roots.clone();
    snapshot.transition = Some(RenderBinding {
        edges: current.record.map_or_else(Vec::new, |r| r.edges),
        context: Some(context),
    });
    snapshot.revalidate(cancel)?;
    Ok(snapshot)
}
// Production activation stays gated until installed editor/parity acceptance.
fn require_transition_preview_gate() -> Result<(), String> {
    if !transition_render_available() {
        return Err("This build can save crossfade proposals but cannot render them yet".into());
    }
    Ok(())
}
fn transition_render_available() -> bool {
    cfg!(windows) && (cfg!(debug_assertions) || cfg!(feature = "phase-j-preview"))
}
#[derive(Serialize)]
pub struct TransitionCapabilities {
    rendering: bool,
    reason: Option<&'static str>,
}
#[tauri::command]
pub fn workspace_transition_capabilities() -> TransitionCapabilities {
    let rendering = transition_render_available();
    TransitionCapabilities {
        rendering,
        reason: if rendering {
            None
        } else {
            Some("Crossfade playback and export are unavailable in this build. You can still keep and recover proposals.")
        },
    }
}
#[tauri::command]
pub async fn start_workspace_transition_preview(
    app: tauri::AppHandle,
    jobs: tauri::State<'_, crate::video_preview::PreviewJobs>,
    request: TransitionRenderRequest,
) -> Result<String, String> {
    require_transition_preview_gate()?;
    crate::video_preview::start_transition_preview(app, jobs, request)
}
#[tauri::command]
pub async fn start_workspace_transition_export(
    app: tauri::AppHandle,
    jobs: tauri::State<'_, crate::video_export::ExportJobs>,
    previews: tauri::State<'_, crate::video_preview::PreviewJobs>,
    request: TransitionRenderRequest,
    options: crate::video_export::Options,
) -> Result<String, String> {
    require_transition_preview_gate()?;
    request.validate()?;
    let (request, title) = tauri::async_runtime::spawn_blocking(move || {
        let settings = user_data::database()?;
        user_data::workspace_roots_from(&settings, &request.workspace_id)?;
        let projects = database()?;
        if project_receipt(&projects, &request.workspace_id, &request.id)?.1
            != request.expected_project
        {
            return Err("Project changed; reload before exporting".to_string());
        }
        let title = super::read(&projects, &request.workspace_id, &request.id)?
            .project
            .title;
        Ok::<_, String>((request, title))
    })
    .await
    .map_err(|e| e.to_string())??;
    crate::video_export::start_transition_export(app, jobs, previews, request, title, options)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionRenderRequest {
    workspace_id: String,
    id: String,
    expected_project: ProjectReceipt,
    expected_revision: u64,
}
impl TransitionRenderRequest {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.workspace_id.is_empty()
            || self.workspace_id.len() > 200
            || self.id.is_empty()
            || self.id.len() > 200
            || self.expected_project.revision == 0
            || self.expected_project.revision > SAFE_REVISION
            || self.expected_revision > SAFE_REVISION
            || self.expected_project.hash.len() != 64
            || !self
                .expected_project
                .hash
                .bytes()
                .all(|c| c.is_ascii_hexdigit())
        {
            return Err("Invalid transition render receipt".into());
        }
        Ok(())
    }
}
pub(crate) fn snapshot_request(
    request: TransitionRenderRequest,
    cancel: impl Fn() -> bool,
) -> Result<crate::video_sequence_render::Snapshot, String> {
    request.validate()?;
    render_transition_snapshot(
        &user_data::database()?,
        &database()?,
        &request.workspace_id,
        &request.id,
        &request.expected_project,
        request.expected_revision,
        cancel,
    )
}
