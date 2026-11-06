// Gated component v1 contract. Included only by the test-only saved adapter.
// No production migration, automatic pruning, rebase or recovery activation.
const CONTRACT_TABLES: &[(&str, &[&str])] = &[
    ("phase_j_sequence_schema_test", &["singleton", "version"]),
    (
        "phase_j_sequence_drafts_test",
        &[
            "root",
            "project_id",
            "project_revision",
            "project_hash",
            "revision",
            "data",
            "data_hash",
        ],
    ),
    (
        "phase_j_sequence_history_test",
        &[
            "root",
            "project_id",
            "project_revision",
            "project_hash",
            "revision",
            "data",
            "data_hash",
        ],
    ),
    (
        "phase_j_sequence_recovery_test",
        &[
            "root",
            "project_id",
            "copy_id",
            "project_revision",
            "project_hash",
            "source_revision",
            "data",
            "data_hash",
            "operation_hash",
        ],
    ),
];
fn payload_hash(data: &str) -> String {
    format!("{:x}", Sha256::digest(data.as_bytes()))
}
fn table_exists(db: &Connection, name: &str) -> Result<bool, String> {
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name=?1)",
        [name],
        |r| r.get(0),
    )
    .map_err(|e| e.to_string())
}
fn require_contract(db: &Connection) -> Result<(), String> {
    let rows: Vec<(i64, i64)> = db
        .prepare("SELECT singleton,version FROM phase_j_sequence_schema_test")
        .map_err(|e| e.to_string())?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<_>>()
        .map_err(|e| e.to_string())?;
    if rows != [(1, 1)] {
        return Err("Unsupported draft storage version; preserve data for review".into());
    }
    for (table, columns) in CONTRACT_TABLES {
        // Identifiers come exclusively from the constants above.
        let shape: Vec<(String, String, i64, i64)> = db
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(|e| e.to_string())?
            .query_map([], |r| Ok((r.get(1)?, r.get(2)?, r.get(3)?, r.get(5)?)))
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<_>>()
            .map_err(|e| e.to_string())?;
        if shape.iter().map(|c| c.0.as_str()).collect::<Vec<_>>() != *columns {
            return Err("Incomplete draft storage schema; preserve data for review".into());
        }
        for (name, kind, required, pk) in shape {
            let integer = matches!(
                name.as_str(),
                "singleton" | "version" | "project_revision" | "revision" | "source_revision"
            );
            let expected_pk = match name.as_str() {
                "singleton" | "root" => 1,
                "project_id" => 2,
                "copy_id" => 3,
                "revision" if *table == "phase_j_sequence_history_test" => 3,
                _ => 0,
            };
            if kind != if integer { "INTEGER" } else { "TEXT" }
                || required != i64::from(name != "singleton")
                || pk != expected_pk
            {
                return Err("Invalid draft storage keys/types; preserve data for review".into());
            }
        }
    }
    Ok(())
}
fn initialize_contract(db: &Connection) -> Result<(), String> {
    let tx = rusqlite::Transaction::new_unchecked(db, TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    if table_exists(&tx, CONTRACT_TABLES[0].0)? {
        require_contract(&tx)?;
    } else {
        for (name, _) in &CONTRACT_TABLES[1..] {
            if table_exists(&tx, name)? {
                return Err(
                    "Unversioned draft data requires explicit import review; preserving all rows"
                        .into(),
                );
            }
        }
        tx.execute_batch("CREATE TABLE phase_j_sequence_schema_test(singleton INTEGER PRIMARY KEY CHECK(singleton=1),version INTEGER NOT NULL CHECK(version>0));
            INSERT INTO phase_j_sequence_schema_test VALUES(1,1);
            CREATE TABLE phase_j_sequence_drafts_test(root TEXT NOT NULL,project_id TEXT NOT NULL,project_revision INTEGER NOT NULL,project_hash TEXT NOT NULL,revision INTEGER NOT NULL,data TEXT NOT NULL,data_hash TEXT NOT NULL,PRIMARY KEY(root,project_id));
            CREATE TABLE phase_j_sequence_history_test(root TEXT NOT NULL,project_id TEXT NOT NULL,project_revision INTEGER NOT NULL,project_hash TEXT NOT NULL,revision INTEGER NOT NULL,data TEXT NOT NULL,data_hash TEXT NOT NULL,PRIMARY KEY(root,project_id,revision));
            CREATE TABLE phase_j_sequence_recovery_test(root TEXT NOT NULL,project_id TEXT NOT NULL,copy_id TEXT NOT NULL,project_revision INTEGER NOT NULL,project_hash TEXT NOT NULL,source_revision INTEGER NOT NULL,data TEXT NOT NULL,data_hash TEXT NOT NULL,operation_hash TEXT NOT NULL,PRIMARY KEY(root,project_id,copy_id));").map_err(|e|e.to_string())?;
        require_contract(&tx)?;
    }
    tx.commit().map_err(|e| e.to_string())
}
fn archive_draft(db: &Connection, context: &Context, draft: &Draft) -> Result<(), String> {
    db.execute("INSERT INTO phase_j_sequence_history_test(root,project_id,project_revision,project_hash,revision,data,data_hash) VALUES(?1,?2,?3,?4,?5,?6,?7)", params![context.root,context.project.id,draft.project_revision,draft.project_hash,draft.revision,draft.data,draft.data_hash]).map_err(|e|e.to_string())?;
    Ok(())
}
fn history_draft(db: &Connection, c: &Context, revision: u64) -> Result<Draft, String> {
    db.query_row("SELECT project_revision,project_hash,revision,data,data_hash FROM phase_j_sequence_history_test WHERE root=?1 AND project_id=?2 AND revision=?3",params![c.root,c.project.id,revision],|r|Ok(Draft{project_revision:r.get(0)?,project_hash:r.get(1)?,revision:r.get(2)?,data:r.get(3)?,data_hash:r.get(4)?})).map_err(|e|e.to_string())
}
#[derive(Debug)]
struct HistoryEntry {
    revision: u64,
    error: Option<String>,
}
#[derive(Debug)]
struct HistoryPage {
    entries: Vec<HistoryEntry>,
    next: Option<u64>,
}
fn history_page(
    db: &mut Connection,
    root: &str,
    id: &str,
    before: Option<u64>,
) -> Result<HistoryPage, String> {
    if before.is_some_and(|r| r == 0 || r > JS_SAFE) {
        return Err("Invalid history cursor".into());
    }
    let tx = db.transaction().map_err(|e| e.to_string())?;
    require_contract(&tx)?;
    let c = context_in(&tx, root, id)?;
    let revisions:Vec<u64>=tx.prepare("SELECT revision FROM phase_j_sequence_history_test WHERE root=?1 AND project_id=?2 AND revision<?3 ORDER BY revision DESC LIMIT 26").map_err(|e|e.to_string())?
        .query_map(params![c.root,c.project.id,before.unwrap_or(JS_SAFE+1)],|r|r.get(0)).map_err(|e|e.to_string())?.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())?;
    let next = if revisions.len() > 25 {
        Some(revisions[24])
    } else {
        None
    };
    let entries = revisions
        .into_iter()
        .take(25)
        .map(|revision| {
            let error = history_draft(&tx, &c, revision)
                .and_then(|d| require_current(&c, &d))
                .err();
            HistoryEntry { revision, error }
        })
        .collect();
    tx.commit().map_err(|e| e.to_string())?;
    Ok(HistoryPage { entries, next })
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryChoice {
    root: String,
    project_id: String,
    project_revision: u64,
    project_hash: String,
    history_revision: u64,
    history_hash: String,
    head_hash: String,
    copy_id: String,
}
fn head_hash(db: &Connection, c: &Context) -> Result<String, String> {
    Ok(payload_hash(
        &serde_json::to_string(&stored(db, c)?).map_err(|e| e.to_string())?,
    ))
}
fn prepare_recovery_copy(
    db: &mut Connection,
    root: &str,
    id: &str,
    revision: u64,
) -> Result<RecoveryChoice, String> {
    let tx = db.transaction().map_err(|e| e.to_string())?;
    require_contract(&tx)?;
    let c = context_in(&tx, root, id)?;
    let draft = history_draft(&tx, &c, revision)?;
    require_current(&c, &draft)?;
    let choice = RecoveryChoice {
        root: c.root.clone(),
        project_id: c.project.id.clone(),
        project_revision: c.revision,
        project_hash: c.document_hash.clone(),
        history_revision: revision,
        history_hash: draft.data_hash,
        head_hash: head_hash(&tx, &c)?,
        copy_id: video_project::id(),
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(choice)
}
fn recovery_in(db: &Connection, c: &Context, id: &str) -> Result<Option<(Draft, String)>, String> {
    db.query_row("SELECT project_revision,project_hash,source_revision,data,data_hash,operation_hash FROM phase_j_sequence_recovery_test WHERE root=?1 AND project_id=?2 AND copy_id=?3",params![c.root,c.project.id,id],|r|Ok((Draft{project_revision:r.get(0)?,project_hash:r.get(1)?,revision:r.get(2)?,data:r.get(3)?,data_hash:r.get(4)?},r.get(5)?))).optional().map_err(|e|e.to_string())
}
fn read_recovery_copy(
    db: &mut Connection,
    root: &str,
    id: &str,
    copy_id: &str,
) -> Result<Draft, String> {
    let tx = db.transaction().map_err(|e| e.to_string())?;
    require_contract(&tx)?;
    let c = context_in(&tx, root, id)?;
    let (draft, _) = recovery_in(&tx, &c, copy_id)?.ok_or("Recovery copy not found")?;
    require_current(&c, &draft)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(draft)
}
fn recover_copy(
    db: &mut Connection,
    choice: &RecoveryChoice,
    mut checkpoint: impl FnMut(&str) -> Result<(), String>,
) -> Result<String, String> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    require_contract(&tx)?;
    let c = context_in(&tx, &choice.root, &choice.project_id)?;
    if c.revision != choice.project_revision
        || c.document_hash != choice.project_hash
        || choice.copy_id.is_empty()
        || choice.copy_id.len() > 128
    {
        return Err("Project changed since recovery selection".into());
    }
    let operation = payload_hash(&serde_json::to_string(choice).map_err(|e| e.to_string())?);
    // A committed request can be acknowledged again after a lost reply without
    // rechecking a head that may since have changed. Never create a second copy.
    if let Some((draft, stored_operation)) = recovery_in(&tx, &c, &choice.copy_id)? {
        require_current(&c, &draft)?;
        if stored_operation != operation
            || draft.revision != choice.history_revision
            || draft.data_hash != choice.history_hash
        {
            return Err("Recovery identity conflict".into());
        }
        tx.commit().map_err(|e| e.to_string())?;
        return Ok(choice.copy_id.clone());
    }
    if head_hash(&tx, &c)? != choice.head_hash {
        return Err("Current draft changed since recovery selection".into());
    }
    let draft = history_draft(&tx, &c, choice.history_revision)?;
    require_current(&c, &draft)?;
    if draft.data_hash != choice.history_hash {
        return Err("Selected history changed".into());
    }
    tx.execute("INSERT INTO phase_j_sequence_recovery_test(root,project_id,copy_id,project_revision,project_hash,source_revision,data,data_hash,operation_hash) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![c.root,c.project.id,choice.copy_id,c.revision,c.document_hash,draft.revision,draft.data,draft.data_hash,operation]).map_err(|e|e.to_string())?;
    checkpoint("after_copy")?;
    checkpoint("before_copy_commit")?;
    tx.commit().map_err(|e| e.to_string())?;
    checkpoint("after_copy_commit")?;
    Ok(choice.copy_id.clone())
}
