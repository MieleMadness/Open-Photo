// cfg(test) only. Append-only durable checkpoints, explicit latest-resume and
// reviewed recovery into a separate session. No application data is opened.
const CHECKPOINT_LIMIT: usize = 64 * 1024;
const CHECKPOINT_SQL:&str="CREATE TABLE phase_j_session_checkpoints_test(root TEXT NOT NULL,project_id TEXT NOT NULL,session_id TEXT NOT NULL,revision INTEGER NOT NULL,data TEXT NOT NULL,hash TEXT NOT NULL,PRIMARY KEY(root,project_id,session_id,revision))";
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointReceipt {
    session_id: String,
    revision: u64,
    hash: String,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionCheckpoint {
    version: u32,
    project_revision: u64,
    document_hash: String,
    session: EditorSession,
}
fn digest_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_checkpoint_receipt(r: &CheckpointReceipt) -> bool {
    video_project::valid_id(&r.session_id)
        && r.revision > 0
        && r.revision <= JS_SAFE
        && digest_id(&r.hash)
}
fn require_checkpoint_store(db: &Connection) -> Result<(), String> {
    let sql:Option<String>=db.query_row("SELECT sql FROM sqlite_master WHERE type='table' AND name='phase_j_session_checkpoints_test'",[],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
    if sql.as_deref() != Some(CHECKPOINT_SQL) {
        return Err("Unknown checkpoint schema; preserve it for review".into());
    }
    Ok(())
}
fn initialize_checkpoint_store(db: &Connection) -> Result<(), String> {
    let tx = rusqlite::Transaction::new_unchecked(db, TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    if !table_exists(&tx, "phase_j_session_checkpoints_test")? {
        tx.execute_batch(CHECKPOINT_SQL)
            .map_err(|e| e.to_string())?;
    }
    require_checkpoint_store(&tx)?;
    tx.commit().map_err(|e| e.to_string())
}
fn checkpoint_for(db: &Connection, s: &EditorSession) -> Result<SessionCheckpoint, String> {
    if !video_project::valid_id(&s.receipt.session_id)
        || s.receipt.revision == 0
        || s.receipt.revision > JS_SAFE
        || s.undo.len() + s.redo.len() > SESSION_UNDO_LIMIT
        || s.recovered_from
            .as_ref()
            .is_some_and(|r| !valid_checkpoint_receipt(r) || r.session_id == s.receipt.session_id)
        || s.undo.last() == Some(&s.receipt.current_id)
        || s.redo.last() == Some(&s.receipt.current_id)
    {
        return Err("Invalid checkpoint session identity/revision/history".into());
    }
    let current = load_layer_manifest_in(db, &s.root, &s.project_id, &s.receipt.current_id)?;
    if current.root != s.root || current.project_id != s.project_id {
        return Err("Checkpoint scope is not canonical".into());
    }
    // Validate the full bounded history, not just the selected snapshot. Do not
    // silently drop corrupt or stale entries to make a resume succeed.
    for id in s.undo.iter().chain(&s.redo) {
        load_layer_manifest_in(db, &s.root, &s.project_id, id)?;
    }
    Ok(SessionCheckpoint {
        version: 1,
        project_revision: current.project_revision,
        document_hash: current.document_hash,
        session: s.clone(),
    })
}
fn checkpoint_row(
    db: &Connection,
    root: &str,
    project: &str,
    r: &CheckpointReceipt,
) -> Result<Option<(String, String)>, String> {
    // Bound materialized payloads even when a stored row has been corrupted.
    db.query_row("SELECT CASE WHEN length(CAST(data AS BLOB))<=?5 THEN data ELSE NULL END,hash FROM phase_j_session_checkpoints_test WHERE root=?1 AND project_id=?2 AND session_id=?3 AND revision=?4",params![root,project,r.session_id,r.revision,CHECKPOINT_LIMIT as u64],|row|Ok((row.get(0)?,row.get(1)?))).optional().map_err(|e|e.to_string())
}
fn latest_checkpoint(
    db: &Connection,
    root: &str,
    project: &str,
    session: &str,
) -> Result<Option<CheckpointReceipt>, String> {
    let receipt=db.query_row("SELECT revision,hash FROM phase_j_session_checkpoints_test WHERE root=?1 AND project_id=?2 AND session_id=?3 ORDER BY revision DESC LIMIT 1",params![root,project,session],|r|Ok(CheckpointReceipt{session_id:session.into(),revision:r.get(0)?,hash:r.get(1)?})).optional().map_err(|e|e.to_string())?;
    if receipt
        .as_ref()
        .is_some_and(|r| !valid_checkpoint_receipt(r))
    {
        return Err("Invalid latest checkpoint receipt; preserve the row".into());
    }
    Ok(receipt)
}
fn load_checkpoint_in(
    db: &Connection,
    root: &str,
    project: &str,
    r: &CheckpointReceipt,
) -> Result<EditorSession, String> {
    require_checkpoint_store(db)?;
    if !valid_checkpoint_receipt(r) {
        return Err("Invalid checkpoint receipt".into());
    }
    let key = root_key(root)?;
    let (data, hash) = checkpoint_row(db, &key, project, r)?.ok_or("Checkpoint not found")?;
    if hash != r.hash || payload_hash(&data) != hash {
        return Err("Checkpoint checksum failed; preserve data".into());
    }
    let saved: SessionCheckpoint = serde_json::from_str(&data).map_err(|e| e.to_string())?;
    if saved.version != 1
        || saved.session.root != key
        || saved.session.project_id != project
        || saved.session.receipt.session_id != r.session_id
        || saved.session.receipt.revision != r.revision
    {
        return Err("Unsupported or mismatched checkpoint identity".into());
    }
    let current = checkpoint_for(db, &saved.session)?;
    if current.project_revision != saved.project_revision
        || current.document_hash != saved.document_hash
    {
        return Err("Checkpoint project binding changed; explicit recovery review required".into());
    }
    Ok(saved.session)
}
fn persist_checkpoint_in(
    db: &Connection,
    s: &EditorSession,
    expected: Option<&CheckpointReceipt>,
) -> Result<CheckpointReceipt, String> {
    require_checkpoint_store(db)?;
    let saved = checkpoint_for(db, s)?;
    let data = serde_json::to_string(&saved).map_err(|e| e.to_string())?;
    if data.len() > CHECKPOINT_LIMIT {
        return Err("Checkpoint exceeds its payload limit".into());
    }
    let next = CheckpointReceipt {
        session_id: s.receipt.session_id.clone(),
        revision: s.receipt.revision,
        hash: payload_hash(&data),
    };
    if let Some((old, hash)) = checkpoint_row(db, &s.root, &s.project_id, &next)? {
        if old != data || hash != next.hash {
            return Err("Checkpoint revision already contains different/corrupt data".into());
        }
        return Ok(next);
    }
    let latest = latest_checkpoint(db, &s.root, &s.project_id, &s.receipt.session_id)?;
    if latest.as_ref() != expected {
        return Err(
            "Saved checkpoint changed; explicitly resume or recover a separate session".into(),
        );
    }
    if let Some(latest) = &latest {
        load_checkpoint_in(db, &s.root, &s.project_id, latest)?;
        if latest.revision >= next.revision {
            return Err("Checkpoint revisions must advance".into());
        }
    }
    db.execute("INSERT INTO phase_j_session_checkpoints_test(root,project_id,session_id,revision,data,hash) VALUES(?1,?2,?3,?4,?5,?6)",params![s.root,s.project_id,next.session_id,next.revision,data,next.hash]).map_err(|e|e.to_string())?;
    Ok(next)
}
fn save_checkpoint(
    db: &mut Connection,
    s: &EditorSession,
    expected: Option<&CheckpointReceipt>,
    mut fault: impl FnMut(&str) -> Result<(), String>,
) -> Result<CheckpointReceipt, String> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let next = persist_checkpoint_in(&tx, s, expected)?;
    fault("after_checkpoint_write")?;
    fault("before_checkpoint_commit")?;
    tx.commit().map_err(|e| e.to_string())?;
    fault("after_checkpoint_commit")?;
    Ok(next)
}
fn resume_checkpoint(
    db: &mut Connection,
    root: &str,
    project: &str,
    r: &CheckpointReceipt,
) -> Result<EditorSession, String> {
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let session = load_checkpoint_in(&tx, root, project, r)?;
    if latest_checkpoint(&tx, &session.root, project, &r.session_id)?.as_ref() != Some(r) {
        return Err("A newer checkpoint exists; choose latest resume or explicit recovery".into());
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(session)
}
struct CheckpointRecovery {
    source: CheckpointReceipt,
    copy_session_id: String,
}
fn prepare_checkpoint_recovery(
    db: &mut Connection,
    root: &str,
    project: &str,
    source: &CheckpointReceipt,
) -> Result<CheckpointRecovery, String> {
    let tx = db.transaction().map_err(|e| e.to_string())?;
    load_checkpoint_in(&tx, root, project, source)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(CheckpointRecovery {
        source: source.clone(),
        copy_session_id: video_project::id(),
    })
}
fn recover_checkpoint(
    db: &mut Connection,
    root: &str,
    project: &str,
    choice: &CheckpointRecovery,
    mut fault: impl FnMut(&str) -> Result<(), String>,
) -> Result<CheckpointReceipt, String> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let mut copy = load_checkpoint_in(&tx, root, project, &choice.source)?;
    copy.recovered_from = Some(choice.source.clone());
    copy.receipt.session_id = choice.copy_session_id.clone();
    copy.receipt.revision = 1;
    let receipt = persist_checkpoint_in(&tx, &copy, None)?;
    fault("after_recovery_checkpoint")?;
    fault("before_recovery_commit")?;
    tx.commit().map_err(|e| e.to_string())?;
    fault("after_recovery_commit")?;
    Ok(receipt)
}
