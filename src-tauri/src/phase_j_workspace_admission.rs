// Read-only phase 3 admission bridge. No production command or activation.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PhaseJWorkspaceReceipt {
    workspace: String,
    project: String,
    revision: u64,
    roots: Vec<String>,
    document: String,
    bindings: String,
}
pub(crate) fn phase_j_workspace_admission(
    settings: &Connection,
    projects: &Connection,
    workspace: &str,
    id: &str,
    revision: u64,
) -> Result<
    (
        PhaseJWorkspaceReceipt,
        crate::video_sequence_render::Snapshot,
    ),
    String,
> {
    use sha2::{Digest, Sha256};
    if revision == 0 || revision > 9_007_199_254_740_991 {
        return Err("Invalid preview project revision".into());
    }
    let roots = user_data::workspace_roots_from(settings, workspace)?;
    let tx = Transaction::new_unchecked(projects, TransactionBehavior::Deferred)
        .map_err(|e| e.to_string())?;
    let snapshot = render_snapshot(&tx, workspace, &roots, id, revision)?;
    // Structural snapshot validation is separate from physical source checks.
    snapshot.revalidate(|| false)?;
    for asset in &snapshot.project.assets {
        if snapshot
            .project
            .audio_tracks
            .iter()
            .any(|track| track.asset_id == asset.id)
            && !snapshot
                .project
                .clips
                .iter()
                .any(|clip| clip.asset_id == asset.id)
        {
            let root = snapshot
                .roots
                .get(&asset.id)
                .ok_or("Missing added-audio source binding")?;
            crate::sequence_media::source(root, asset, || false)?;
        }
    }
    let (document, bindings): (String, String) = tx
        .query_row(
            "SELECT data,bindings FROM workspace_video_projects WHERE workspace=?1 AND id=?2",
            params![workspace, id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;
    let receipt = PhaseJWorkspaceReceipt {
        workspace: workspace.into(),
        project: id.into(),
        revision,
        roots: roots.clone(),
        document: format!("{:x}", Sha256::digest(document.as_bytes())),
        bindings: format!("{:x}", Sha256::digest(bindings.as_bytes())),
    };
    tx.commit().map_err(|e| e.to_string())?;
    if user_data::workspace_roots_from(settings, workspace)? != roots {
        return Err("Workspace changed during preview admission".into());
    }
    Ok((receipt, snapshot))
}
pub(crate) fn phase_j_workspace_current(
    settings: &Connection,
    projects: &Connection,
    receipt: &PhaseJWorkspaceReceipt,
) -> Result<bool, String> {
    let (fresh, _) = phase_j_workspace_admission(
        settings,
        projects,
        &receipt.workspace,
        &receipt.project,
        receipt.revision,
    )?;
    Ok(&fresh == receipt)
}

// Opaque native authorization: callers may inspect roots only after matching the
// entire saved edit document, never construct an access grant from portable JSON.
pub(crate) struct PhaseJPreviewSources {
    receipt: PhaseJWorkspaceReceipt,
    snapshot: crate::video_sequence_render::Snapshot,
}
impl PhaseJPreviewSources {
    pub(crate) fn admit(
        settings: &Connection,
        projects: &Connection,
        workspace: &str,
        id: &str,
        revision: u64,
    ) -> Result<Self, String> {
        let (receipt, snapshot) =
            phase_j_workspace_admission(settings, projects, workspace, id, revision)?;
        Ok(Self { receipt, snapshot })
    }
    pub(crate) fn roots_for(&self, project: &Sequence) -> Result<&Bindings, String> {
        if serde_json::to_value(project).map_err(|e| e.to_string())?
            != serde_json::to_value(&self.snapshot.project).map_err(|e| e.to_string())?
        {
            return Err("Preview document differs from authorized workspace project".into());
        }
        Ok(&self.snapshot.roots)
    }
    pub(crate) fn current(
        &self,
        settings: &Connection,
        projects: &Connection,
    ) -> Result<bool, String> {
        phase_j_workspace_current(settings, projects, &self.receipt)
    }
}
