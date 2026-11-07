// Included only by the cfg(test) saved-project adapter. Immutable snapshots,
// not an active editor head, production migration or rendering authorization.
const LAYER_MANIFEST_LIMIT: usize = 4 * 1024 * 1024;
const LAYER_STORE_SQL: &str = "CREATE TABLE phase_j_layer_manifests_test(root TEXT NOT NULL,project_id TEXT NOT NULL,id TEXT NOT NULL,version INTEGER NOT NULL CHECK(version>=1),data TEXT NOT NULL,PRIMARY KEY(root,project_id,id))";
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestEdge {
    from_clip_id: String,
    to_clip_id: String,
    frames: u64,
}
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LayerManifest {
    version: u32,
    timing_policy: u32,
    root: String,
    project_id: String,
    project_revision: u64,
    document_hash: String,
    project_json: String,
    boundaries: Vec<ManifestEdge>,
    timing: crate::crossfade_layer_clock::Plan,
}
fn require_layer_store(db: &Connection) -> Result<(), String> {
    let sql:Option<String>=db.query_row("SELECT sql FROM sqlite_master WHERE type='table' AND name='phase_j_layer_manifests_test'",[],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
    // This is a private disposable schema, created only from the literal above.
    // Refuse any schema variation instead of attempting a speculative repair.
    if sql.as_deref() != Some(LAYER_STORE_SQL) {
        return Err(
            "Unknown or incomplete layer manifest storage; preserve data for review".into(),
        );
    }
    Ok(())
}
fn initialize_layer_store(db: &Connection) -> Result<(), String> {
    let tx = rusqlite::Transaction::new_unchecked(db, TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    if !table_exists(&tx, "phase_j_layer_manifests_test")? {
        tx.execute_batch(LAYER_STORE_SQL)
            .map_err(|e| e.to_string())?;
    }
    require_layer_store(&tx)?;
    tx.commit().map_err(|e| e.to_string())
}
fn layer_manifest_in(
    db: &Connection,
    root: &str,
    id: &str,
    edges: &[crate::crossfade_clock::Boundary<'_>],
) -> Result<LayerManifest, String> {
    let root = root_key(root)?;
    let (project, revision) = read(db, &root, id)?;
    if project.id != id || revision == 0 || revision > JS_SAFE {
        return Err("Invalid authoritative manifest project identity/revision".into());
    }
    let raw: String = db
        .query_row(
            "SELECT data FROM video_sequences WHERE root=?1 AND id=?2",
            params![root, id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    // Recompute from the independently read native project, never from a
    // deserialized manifest's own claims about source identity or timing.
    let timing = crate::crossfade_layer_clock::plan(&project, edges)?;
    let boundaries = project
        .clips
        .windows(2)
        .filter_map(|pair| {
            edges
                .iter()
                .find(|edge| edge.from_clip_id == pair[0].id && edge.to_clip_id == pair[1].id)
        })
        .map(|edge| ManifestEdge {
            from_clip_id: edge.from_clip_id.into(),
            to_clip_id: edge.to_clip_id.into(),
            frames: edge.frames,
        })
        .collect();
    Ok(LayerManifest {
        version: 1,
        timing_policy: 1,
        root,
        project_id: project.id,
        project_revision: revision,
        document_hash: payload_hash(&raw),
        project_json: raw,
        boundaries,
        timing,
    })
}
fn encode_layer_manifest(manifest: &LayerManifest) -> Result<String, String> {
    let data = serde_json::to_string(manifest).map_err(|e| e.to_string())?;
    if data.len() > LAYER_MANIFEST_LIMIT {
        return Err("Layer manifest is too large".into());
    }
    Ok(data)
}
fn prepare_layer_manifest(
    db: &mut Connection,
    root: &str,
    id: &str,
    edges: &[crate::crossfade_clock::Boundary<'_>],
) -> Result<String, String> {
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let data = encode_layer_manifest(&layer_manifest_in(&tx, root, id, edges)?)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(data)
}
fn validate_layer_manifest(
    db: &Connection,
    root: &str,
    id: &str,
    data: &str,
) -> Result<LayerManifest, String> {
    if data.len() > LAYER_MANIFEST_LIMIT {
        return Err("Layer manifest is too large".into());
    }
    let manifest: LayerManifest = serde_json::from_str(data).map_err(|e| e.to_string())?;
    if manifest.version != 1 || manifest.timing_policy != 1 || manifest.boundaries.len() > 199 {
        return Err("Unsupported layer manifest version/policy or edge count".into());
    }
    let edges = manifest
        .boundaries
        .iter()
        .map(|e| crate::crossfade_clock::Boundary {
            from_clip_id: &e.from_clip_id,
            to_clip_id: &e.to_clip_id,
            frames: e.frames,
        })
        .collect::<Vec<_>>();
    let expected = layer_manifest_in(db, root, id, &edges)?;
    if manifest != expected {
        return Err("Layer manifest is stale or differs from authoritative project/timing; preserve it for review".into());
    }
    Ok(manifest)
}
fn layer_row(
    db: &Connection,
    root: &str,
    project: &str,
    id: &str,
) -> Result<Option<(u32, String)>, String> {
    db.query_row("SELECT version,data FROM phase_j_layer_manifests_test WHERE root=?1 AND project_id=?2 AND id=?3",params![root,project,id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())
}
fn save_layer_manifest(
    db: &mut Connection,
    root: &str,
    project: &str,
    data: &str,
    mut checkpoint: impl FnMut(&str) -> Result<(), String>,
) -> Result<String, String> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    require_layer_store(&tx)?;
    let manifest = validate_layer_manifest(&tx, root, project, data)?;
    let id = persist_layer_manifest_in(&tx, &manifest)?;
    if !id.1 {
        tx.commit().map_err(|e| e.to_string())?;
        return Ok(id.0);
    }
    checkpoint("after_manifest_insert")?;
    checkpoint("before_manifest_commit")?;
    tx.commit().map_err(|e| e.to_string())?;
    checkpoint("after_manifest_commit")?;
    Ok(id.0)
}
// Caller owns a transaction and supplies an independently validated manifest.
// Return whether this transaction inserted a new immutable record.
fn persist_layer_manifest_in(
    db: &Connection,
    manifest: &LayerManifest,
) -> Result<(String, bool), String> {
    let canonical = encode_layer_manifest(manifest)?;
    let id = payload_hash(&canonical);
    if let Some((version, old)) = layer_row(db, &manifest.root, &manifest.project_id, &id)? {
        if version != 1 || old != canonical {
            return Err(
                "Existing layer manifest identity is corrupt or unsupported; preserve it".into(),
            );
        }
        return Ok((id, false));
    }
    db.execute("INSERT INTO phase_j_layer_manifests_test(root,project_id,id,version,data) VALUES(?1,?2,?3,1,?4)",params![manifest.root,manifest.project_id,id,canonical]).map_err(|e|e.to_string())?;
    Ok((id, true))
}
fn load_layer_manifest(
    db: &mut Connection,
    root: &str,
    project: &str,
    id: &str,
) -> Result<LayerManifest, String> {
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let manifest = load_layer_manifest_in(&tx, root, project, id)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(manifest)
}
fn load_layer_manifest_in(
    db: &Connection,
    root: &str,
    project: &str,
    id: &str,
) -> Result<LayerManifest, String> {
    if id.len() != 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("Invalid layer manifest ID".into());
    }
    require_layer_store(db)?;
    let key = root_key(root)?;
    let (version, data) = layer_row(db, &key, project, id)?.ok_or("Layer manifest not found")?;
    if version != 1 || payload_hash(&data) != id {
        return Err("Stored manifest checksum/version failed; preserve data".into());
    }
    validate_layer_manifest(db, &key, project, &data)
}
