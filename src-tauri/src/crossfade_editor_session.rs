// Gated editor session. Checkpoint persistence is explicit and separate from
// apply; no production command, UI, preview or export uses this.
const SESSION_UNDO_LIMIT: usize = 20;
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionReceipt {
    session_id: String,
    revision: u64,
    current_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EditorSession {
    root: String,
    project_id: String,
    receipt: SessionReceipt,
    undo: Vec<String>,
    redo: Vec<String>,
    recovered_from: Option<CheckpointReceipt>,
}
#[derive(Clone, Copy)]
enum SessionAction<'a> {
    Select(&'a str),
    Crossfade {
        from: &'a str,
        to: &'a str,
        frames: Option<u64>,
    },
    Undo,
    Redo,
}
impl EditorSession {
    fn open(db: &mut Connection, root: &str, project: &str, id: &str) -> Result<Self, String> {
        let manifest = load_layer_manifest(db, root, project, id)?;
        Ok(Self {
            root: manifest.root,
            project_id: manifest.project_id,
            receipt: SessionReceipt {
                session_id: video_project::id(),
                revision: 1,
                current_id: id.into(),
            },
            undo: vec![],
            redo: vec![],
            recovered_from: None,
        })
    }
    fn apply(
        &mut self,
        db: &mut Connection,
        expected: &SessionReceipt,
        action: SessionAction<'_>,
        mut checkpoint: impl FnMut(&str) -> Result<(), String>,
    ) -> Result<SessionReceipt, String> {
        if expected != &self.receipt {
            return Err("Editor session changed; reload its current receipt".into());
        }
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let current =
            load_layer_manifest_in(&tx, &self.root, &self.project_id, &self.receipt.current_id)?;
        let mut undo = self.undo.clone();
        let mut redo = self.redo.clone();
        let next = match action {
            SessionAction::Select(id) => {
                load_layer_manifest_in(&tx, &self.root, &self.project_id, id)?;
                id.to_owned()
            }
            SessionAction::Crossfade { from, to, frames } => {
                let (project, _) = read(&tx, &self.root, &self.project_id)?;
                if !project
                    .clips
                    .windows(2)
                    .any(|pair| pair[0].id == from && pair[1].id == to)
                {
                    return Err("Transition edits require an adjacent native clip pair".into());
                }
                let mut boundaries = current.boundaries.clone();
                boundaries.retain(|b| b.from_clip_id != from || b.to_clip_id != to);
                if let Some(frames) = frames {
                    boundaries.push(ManifestEdge {
                        from_clip_id: from.into(),
                        to_clip_id: to.into(),
                        frames,
                    });
                }
                let edges = boundaries
                    .iter()
                    .map(|e| crate::crossfade_clock::Boundary {
                        from_clip_id: &e.from_clip_id,
                        to_clip_id: &e.to_clip_id,
                        frames: e.frames,
                    })
                    .collect::<Vec<_>>();
                let next = layer_manifest_in(&tx, &self.root, &self.project_id, &edges)?;
                let (id, inserted) = persist_layer_manifest_in(&tx, &next)?;
                if inserted {
                    checkpoint("after_session_manifest")?;
                }
                id
            }
            SessionAction::Undo => {
                let id = undo.pop().ok_or("Nothing to undo")?;
                load_layer_manifest_in(&tx, &self.root, &self.project_id, &id)?;
                redo.push(self.receipt.current_id.clone());
                id
            }
            SessionAction::Redo => {
                let id = redo.pop().ok_or("Nothing to redo")?;
                load_layer_manifest_in(&tx, &self.root, &self.project_id, &id)?;
                undo.push(self.receipt.current_id.clone());
                id
            }
        };
        if next == self.receipt.current_id {
            tx.commit().map_err(|e| e.to_string())?;
            return Ok(self.receipt.clone());
        }
        let revision = self
            .receipt
            .revision
            .checked_add(1)
            .filter(|r| *r <= JS_SAFE)
            .ok_or("Editor revision overflow")?;
        if matches!(
            action,
            SessionAction::Select(_) | SessionAction::Crossfade { .. }
        ) {
            undo.push(self.receipt.current_id.clone());
            redo.clear();
        }
        if undo.len() > SESSION_UNDO_LIMIT {
            undo.remove(0);
        }
        checkpoint("before_session_commit")?;
        tx.commit().map_err(|e| e.to_string())?;
        // Install session state only after SQLite commits. A lost reply leaves
        // an advanced receipt; an obsolete receipt cannot apply the action twice.
        self.receipt.current_id = next;
        self.receipt.revision = revision;
        self.undo = undo;
        self.redo = redo;
        checkpoint("after_session_commit")?;
        Ok(self.receipt.clone())
    }
}
