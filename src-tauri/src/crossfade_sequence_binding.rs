//! Phase J gated adapter: actual saved-sequence reader/schema + detached drafts.
//! Included only under cfg(test). Never opens the application database, changes
//! a production project, reads original media bytes or routes preview/export.
use super::*;
use crate::crossfade_native_snapshot;
use rusqlite::TransactionBehavior;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const JS_SAFE: u64 = 9_007_199_254_740_991;
#[derive(Clone)]
struct Context {
    root: String,
    project: Sequence,
    revision: u64,
    document_hash: String,
    source: Value,
    clips: Vec<(String, u64)>,
}
impl Context {
    fn validate_snapshot(&self, snapshot: &str) -> Result<(), String> {
        let clips = self
            .clips
            .iter()
            .map(|(id, frames)| (id.as_str(), *frames))
            .collect::<Vec<_>>();
        crossfade_native_snapshot::validate_snapshot(
            snapshot,
            self.revision,
            &self.source,
            &self.project.id,
            u64::from(self.project.frame_rate),
            &clips,
        )?;
        Ok(())
    }
    fn encode(&self, boundaries: &[(&str, &str, u64)]) -> Result<String, String> {
        if boundaries.len() > self.clips.len().saturating_sub(1) {
            return Err("Too many draft boundaries".into());
        }
        let draft=json!({"version":1,"project_id":self.project.id,"fps":self.project.frame_rate,
            "clips":self.clips.iter().map(|(id,frames)|json!({"id":id,"frames":frames})).collect::<Vec<_>>(),
            "boundaries":boundaries.iter().map(|(from,to,frames)|json!({"from_clip_id":from,"to_clip_id":to,"frames":frames})).collect::<Vec<_>>()}).to_string();
        let snapshot=json!({"version":1,"project_revision":self.revision,"source_identity":self.source,"draft":draft}).to_string();
        self.validate_snapshot(&snapshot)?;
        Ok(snapshot)
    }
}
// Caller holds one SQLite transaction for both native reads. Full stored JSON
// is bound as well as the revision: edits with an unchanged revision are stale.
fn context_in(db: &Connection, root: &str, id: &str) -> Result<Context, String> {
    let key = root_key(root)?;
    let (project, revision) = read(db, &key, id)?;
    if project.id != id || revision == 0 || revision > JS_SAFE {
        return Err("Invalid authoritative project identity/revision".into());
    }
    if project.clips.len() < 2
        || !project.transitions.is_empty()
        || !project.titles.is_empty()
        || !project.audio_tracks.is_empty()
        || !project.audio_sources.is_empty()
    {
        return Err("This gated draft adapter requires a video-only hard-cut project with at least two clips".into());
    }
    let frames = crate::video_sequence_render::frame_allocation(&project)?;
    let mut sources = Vec::with_capacity(project.clips.len());
    for clip in &project.clips {
        let asset = project
            .assets
            .iter()
            .find(|asset| asset.id == clip.asset_id)
            .ok_or("Missing saved asset")?;
        if !matches!(asset.media, Media::Video { .. }) || asset.bytes > JS_SAFE {
            return Err("Unsupported saved source for a gated crossfade draft".into());
        }
        sources.push(json!({"clip_id":clip.id,"asset_id":asset.id,"sha256":asset.sha256.to_ascii_lowercase(),"bytes":asset.bytes,"source_in":clip.source_in,"source_out":clip.source_out}));
    }
    let raw: String = db
        .query_row(
            "SELECT data FROM video_sequences WHERE id=?1 AND root=?2",
            params![id, key],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let context = Context {
        root: key,
        revision,
        document_hash: format!("{:x}", Sha256::digest(raw.as_bytes())),
        clips: project
            .clips
            .iter()
            .zip(frames)
            .map(|(clip, frames)| (clip.id.clone(), frames))
            .collect(),
        project,
        source: json!({"version":1,"clips":sources}),
    };
    // Reuse the existing strict wire/source/rational/frame validators even for
    // an empty draft; a valid hard-cut project is not automatically eligible.
    context.encode(&[])?;
    Ok(context)
}
fn capture(db: &mut Connection, root: &str, id: &str) -> Result<Context, String> {
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let context = context_in(&tx, root, id)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(context)
}
#[derive(Debug, PartialEq, Eq)]
struct SavedLayerPlan {
    root: String,
    project_id: String,
    revision: u64,
    document_hash: String,
    timing: crate::crossfade_layer_clock::Plan,
}
// A separate read-only planning path: do not weaken the v1 draft snapshot's
// video-only restrictions or persist unsupported layer data in that format.
fn capture_layer_plan(
    db: &mut Connection,
    root: &str,
    id: &str,
    edges: &[crate::crossfade_clock::Boundary<'_>],
) -> Result<SavedLayerPlan, String> {
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let key = root_key(root)?;
    let (project, revision) = read(&tx, &key, id)?;
    if project.id != id || revision == 0 || revision > JS_SAFE {
        return Err("Invalid authoritative layer project identity/revision".into());
    }
    let raw: String = tx
        .query_row(
            "SELECT data FROM video_sequences WHERE root=?1 AND id=?2",
            params![key, id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let timing = crate::crossfade_layer_clock::plan(&project, edges)?;
    let result = SavedLayerPlan {
        root: key,
        project_id: project.id,
        revision,
        document_hash: payload_hash(&raw),
        timing,
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(result)
}
fn initialize_detached(db: &Connection) -> Result<(), String> {
    initialize_contract(db)
}
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
struct Draft {
    project_revision: u64,
    project_hash: String,
    revision: u64,
    data: String,
    data_hash: String,
}
fn stored(db: &Connection, context: &Context) -> Result<Option<Draft>, String> {
    db.query_row("SELECT project_revision,project_hash,revision,data,data_hash FROM phase_j_sequence_drafts_test WHERE root=?1 AND project_id=?2",params![context.root,context.project.id],|row|Ok(Draft {project_revision:row.get(0)?,project_hash:row.get(1)?,revision:row.get(2)?,data:row.get(3)?,data_hash:row.get(4)?})).optional().map_err(|e|e.to_string())
}
fn require_current(context: &Context, draft: &Draft) -> Result<(), String> {
    if draft.project_revision != context.revision
        || draft.project_hash != context.document_hash
        || draft.revision == 0
        || draft.revision > JS_SAFE
    {
        return Err("Saved draft no longer matches the authoritative video project".into());
    }
    if payload_hash(&draft.data) != draft.data_hash {
        return Err("Draft payload checksum failed; preserve the stored data".into());
    }
    context.validate_snapshot(&draft.data)
}
fn load(db: &mut Connection, root: &str, id: &str) -> Result<Option<Draft>, String> {
    let tx = db.transaction().map_err(|e| e.to_string())?;
    require_contract(&tx)?;
    let context = context_in(&tx, root, id)?;
    let draft = stored(&tx, &context)?;
    if let Some(draft) = &draft {
        require_current(&context, draft)?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(draft)
}
fn commit(
    db: &mut Connection,
    expected: &Context,
    expected_draft: Option<u64>,
    next: &str,
    mut checkpoint: impl FnMut(&str) -> Result<(), String>,
) -> Result<u64, String> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    require_contract(&tx)?;
    let current = context_in(&tx, &expected.root, &expected.project.id)?;
    if current.revision != expected.revision || current.document_hash != expected.document_hash {
        return Err("Video project changed; reload the draft context".into());
    }
    current.validate_snapshot(next)?;
    let old = stored(&tx, &current)?;
    let revision = match (old, expected_draft) {
        (None, None) => 1,
        (Some(old), Some(expected_revision)) if expected_revision == old.revision => {
            require_current(&current, &old)?;
            if old.data == next {
                tx.commit().map_err(|e| e.to_string())?;
                return Ok(old.revision);
            }
            let revision = old
                .revision
                .checked_add(1)
                .filter(|r| *r <= JS_SAFE)
                .ok_or("Draft revision overflow")?;
            archive_draft(&tx, &current, &old)?;
            checkpoint("after_history")?;
            revision
        }
        _ => return Err("Crossfade draft changed elsewhere; reload".into()),
    };
    tx.execute("INSERT INTO phase_j_sequence_drafts_test(root,project_id,project_revision,project_hash,revision,data,data_hash) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(root,project_id) DO UPDATE SET project_revision=excluded.project_revision,project_hash=excluded.project_hash,revision=excluded.revision,data=excluded.data,data_hash=excluded.data_hash",params![current.root,current.project.id,current.revision,current.document_hash,revision,next,payload_hash(next)]).map_err(|e|e.to_string())?;
    checkpoint("after_draft")?;
    checkpoint("before_commit")?;
    tx.commit().map_err(|e| e.to_string())?;
    checkpoint("after_commit")?;
    Ok(revision)
}

include!("crossfade_store_contract.rs");
include!("crossfade_layer_manifest.rs");
include!("crossfade_editor_session.rs");
include!("crossfade_session_checkpoint.rs");
include!("crossfade_session_seek.rs");
include!("crossfade_preview_requests.rs");
#[cfg(windows)]
include!("crossfade_preview_decode.rs");
#[cfg(windows)]
include!("crossfade_composed_preview.rs");
#[cfg(windows)]
include!("crossfade_av_preview.rs");

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        root: PathBuf,
        source: PathBuf,
        db: Connection,
        project: Sequence,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir()
                .join(format!("phase-j-sequence-binding-{}", video_project::id()));
            fs::create_dir(&root).unwrap();
            let source = root.join("original.mp4");
            fs::write(&source, b"disposable-original-media").unwrap();
            let info=video_probe::parse_probe(br#"{"streams":[{"index":0,"codec_type":"video","codec_name":"h264","width":96,"height":64,"time_base":"1/90000"}],"format":{"duration":"2"}}"#).unwrap();
            let legacy = video_project::new_project(root.to_str().unwrap(), &source, info).unwrap();
            let mut project = migrate(legacy).unwrap();
            let mut second = project.clips[0].clone();
            second.id = "clip-b".into();
            project.clips.push(second);
            let db = Connection::open(root.join("prototype.db")).unwrap();
            db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")
                .unwrap();
            initialize(&db).unwrap();
            initialize_detached(&db).unwrap();
            store(&db, root.to_str().unwrap(), &project, None).unwrap();
            Self {
                root,
                source,
                db,
                project,
            }
        }
        fn context(&mut self) -> Context {
            capture(&mut self.db, self.root.to_str().unwrap(), &self.project.id).unwrap()
        }
        fn snapshot(&self, context: &Context, frames: u64) -> String {
            context
                .encode(&[(&self.project.clips[0].id, &self.project.clips[1].id, frames)])
                .unwrap()
        }
        fn reopen(&mut self) {
            self.db = Connection::open(self.root.join("prototype.db")).unwrap();
        }
        fn history(&self) -> i64 {
            self.db
                .query_row(
                    "SELECT count(*) FROM phase_j_sequence_history_test",
                    [],
                    |r| r.get(0),
                )
                .unwrap()
        }
        fn raw_project(&self) -> String {
            self.db
                .query_row(
                    "SELECT data FROM video_sequences WHERE id=?1",
                    [&self.project.id],
                    |r| r.get(0),
                )
                .unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            assert_eq!(
                fs::read(&self.source).unwrap(),
                b"disposable-original-media"
            );
            self.db = Connection::open_in_memory().unwrap();
            fs::remove_dir_all(&self.root).unwrap();
        }
    }
    #[test]
    fn binds_the_real_saved_reader_and_cumulative_frame_allocation() {
        let mut f = Fixture::new();
        let mut third = f.project.clips[0].clone();
        third.id = "clip-c".into();
        f.project.clips.push(third);
        for clip in &mut f.project.clips {
            clip.source_out.ticks = "22500".into();
        }
        store(&f.db, f.root.to_str().unwrap(), &f.project, Some(1)).unwrap();
        let context = f.context();
        assert_eq!(context.revision, 2);
        assert_eq!(
            context.clips.iter().map(|c| c.1).collect::<Vec<_>>(),
            vec![8, 7, 8]
        );
        assert_eq!(context.source["clips"][0]["source_out"]["ticks"], "22500");
        assert_eq!(
            context.source["clips"][0]["sha256"],
            f.project.assets[0].sha256
        );
    }
    #[test]
    fn detached_drafts_survive_cold_reopens_without_updating_the_project() {
        let mut f = Fixture::new();
        let raw = f.raw_project();
        let context = f.context();
        let snapshot = f.snapshot(&context, 6);
        assert_eq!(
            commit(&mut f.db, &context, None, &snapshot, |_| Ok(())).unwrap(),
            1
        );
        f.reopen();
        f.reopen();
        assert_eq!(
            load(&mut f.db, f.root.to_str().unwrap(), &f.project.id)
                .unwrap()
                .unwrap()
                .data,
            snapshot
        );
        assert_eq!(f.raw_project(), raw);
        assert_eq!(
            read(&f.db, f.root.to_str().unwrap(), &f.project.id)
                .unwrap()
                .1,
            1
        );
        assert_eq!(
            f.db.query_row("SELECT count(*) FROM video_sequence_history", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn independent_connections_refuse_stale_writers_and_noop_does_not_add_history() {
        let mut f = Fixture::new();
        let context = f.context();
        let first = f.snapshot(&context, 6);
        let next = f.snapshot(&context, 9);
        commit(&mut f.db, &context, None, &first, |_| Ok(())).unwrap();
        let mut other = Connection::open(f.root.join("prototype.db")).unwrap();
        assert_eq!(
            commit(&mut f.db, &context, Some(1), &next, |_| Ok(())).unwrap(),
            2
        );
        assert!(commit(&mut other, &context, Some(1), &first, |_| Ok(())).is_err());
        assert_eq!(
            commit(&mut other, &context, Some(2), &next, |_| Ok(())).unwrap(),
            2
        );
        assert!(commit(&mut other, &context, Some(1), &next, |_| Ok(())).is_err());
        drop(other);
        assert_eq!(f.history(), 1);
        assert_eq!(
            load(&mut f.db, f.root.to_str().unwrap(), &f.project.id)
                .unwrap()
                .unwrap()
                .data,
            next
        );
    }
    #[test]
    fn same_revision_document_changes_are_stale_even_with_identical_sources_and_frames() {
        let mut f = Fixture::new();
        let context = f.context();
        let saved = f.snapshot(&context, 6);
        commit(&mut f.db, &context, None, &saved, |_| Ok(())).unwrap();
        let mut changed = f.project.clone();
        changed.clips[0].edits.brightness = 1;
        f.db.execute(
            "UPDATE video_sequences SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&changed).unwrap(), f.project.id],
        )
        .unwrap();
        let fresh = f.context();
        assert_eq!(context.revision, fresh.revision);
        assert_eq!(context.clips, fresh.clips);
        assert_eq!(context.source, fresh.source);
        assert_ne!(context.document_hash, fresh.document_hash);
        assert!(commit(&mut f.db, &context, Some(1), &saved, |_| Ok(())).is_err());
        assert!(load(&mut f.db, f.root.to_str().unwrap(), &f.project.id).is_err());
        assert_eq!(f.history(), 0);
    }
    #[test]
    fn invalid_snapshots_wrong_roots_and_deleted_projects_are_not_empty_drafts() {
        let mut f = Fixture::new();
        let context = f.context();
        let snapshot = f.snapshot(&context, 6);
        let mut forged: Value = serde_json::from_str(&snapshot).unwrap();
        forged["source_identity"]["clips"][0]["sha256"] = json!("a".repeat(64));
        assert!(commit(&mut f.db, &context, None, &forged.to_string(), |_| Ok(())).is_err());
        let other = f.root.join("other");
        fs::create_dir(&other).unwrap();
        assert!(capture(&mut f.db, other.to_str().unwrap(), &f.project.id).is_err());
        commit(&mut f.db, &context, None, &snapshot, |_| Ok(())).unwrap();
        f.db.execute("UPDATE phase_j_sequence_drafts_test SET data='corrupt'", [])
            .unwrap();
        assert!(load(&mut f.db, f.root.to_str().unwrap(), &f.project.id).is_err());
        f.db.execute("DELETE FROM video_sequences", []).unwrap();
        assert!(load(&mut f.db, f.root.to_str().unwrap(), &f.project.id).is_err());
        assert_eq!(
            f.db.query_row(
                "SELECT count(*) FROM phase_j_sequence_drafts_test",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
    }
    #[test]
    fn rollback_preserves_current_and_history_across_file_backed_reopens() {
        for boundary in ["after_history", "after_draft", "before_commit"] {
            let mut f = Fixture::new();
            let context = f.context();
            let first = f.snapshot(&context, 6);
            let next = f.snapshot(&context, 9);
            commit(&mut f.db, &context, None, &first, |_| Ok(())).unwrap();
            assert!(commit(&mut f.db, &context, Some(1), &next, |point| {
                if point == boundary {
                    Err("disposable interruption".into())
                } else {
                    Ok(())
                }
            })
            .is_err());
            f.reopen();
            f.reopen();
            let saved = load(&mut f.db, f.root.to_str().unwrap(), &f.project.id)
                .unwrap()
                .unwrap();
            assert_eq!(saved.revision, 1);
            assert_eq!(saved.data, first);
            assert_eq!(f.history(), 0);
        }
    }
    #[test]
    fn layered_projects_and_dips_remain_unsupported_by_this_gated_adapter() {
        let mut f = Fixture::new();
        let mut changed = f.project.clone();
        changed.transitions.push(CutTransition {
            from_clip_id: changed.clips[0].id.clone(),
            to_clip_id: changed.clips[1].id.clone(),
            kind: CutTransitionKind::DipToBlack,
            duration_seconds: 0.4,
        });
        store(&f.db, f.root.to_str().unwrap(), &changed, Some(1)).unwrap();
        assert!(capture(&mut f.db, f.root.to_str().unwrap(), &changed.id).is_err());
        changed = f.project.clone();
        changed.titles.push(TitleOverlay {
            id: "title-1".into(),
            text: "Preserve title".into(),
            start_seconds: 0.0,
            end_seconds: 1.0,
            x_percent: 50,
            y_percent: 50,
        });
        store(&f.db, f.root.to_str().unwrap(), &changed, Some(2)).unwrap();
        assert!(capture(&mut f.db, f.root.to_str().unwrap(), &changed.id).is_err());
        assert_eq!(
            read(&f.db, f.root.to_str().unwrap(), &changed.id)
                .unwrap()
                .0,
            changed
        );
    }
    #[test]
    fn legacy_v2_is_read_through_native_migration_without_rewriting_saved_json() {
        let mut f = Fixture::new();
        let mut old = serde_json::to_value(&f.project).unwrap();
        old["schema_version"] = json!(2);
        for asset in old["assets"].as_array_mut().unwrap() {
            let media = asset.as_object_mut().unwrap().remove("media").unwrap();
            asset["info"] = media["info"].clone();
        }
        f.db.execute(
            "UPDATE video_sequences SET data=?1 WHERE id=?2",
            params![old.to_string(), f.project.id],
        )
        .unwrap();
        let raw = f.raw_project();
        let context = f.context();
        assert_eq!(context.project.schema_version, 3);
        let snapshot = f.snapshot(&context, 6);
        commit(&mut f.db, &context, None, &snapshot, |_| Ok(())).unwrap();
        assert_eq!(f.raw_project(), raw);
    }

    #[test]
    fn photo_and_audio_projects_are_preserved_instead_of_partially_adapted() {
        {
            let mut f = Fixture::new();
            let mut photo = f.project.clone();
            photo.assets[0].media = Media::Photo {
                width: 96,
                height: 64,
            };
            photo.assets[0].relative_path = "image.png".into();
            for clip in &mut photo.clips {
                clip.source_in = video_project::VideoTime {
                    ticks: "0".into(),
                    time_base: "1/1000000".into(),
                };
                clip.source_out = video_project::VideoTime {
                    ticks: "2000000".into(),
                    time_base: "1/1000000".into(),
                };
            }
            store(&f.db, f.root.to_str().unwrap(), &photo, Some(1)).unwrap();
            assert!(capture(&mut f.db, f.root.to_str().unwrap(), &photo.id).is_err());
            assert_eq!(
                read(&f.db, f.root.to_str().unwrap(), &photo.id).unwrap().0,
                photo
            );
        }
        let mut f = Fixture::new();
        let mut audio = f.project.clone();
        audio.audio_sources.push(AudioSource {
            id: "music-1".into(),
            relative_path: "soundtrack.wav".into(),
            sha256: "b".repeat(64),
            bytes: 123,
            info: video_probe::AudioOnlyInfo {
                duration_seconds: 2.0,
                stream: 0,
                codec: "pcm_s16le".into(),
                channels: 2,
                sample_rate: 48000,
            },
        });
        audio.audio_tracks.push(AudioTrack {
            id: "track-1".into(),
            asset_id: "music-1".into(),
            start_seconds: 0.0,
            source_in_seconds: 0.0,
            duration_seconds: 1.0,
            gain_percent: 100,
            fade_in_seconds: 0.0,
            fade_out_seconds: 0.0,
        });
        store(&f.db, f.root.to_str().unwrap(), &audio, Some(1)).unwrap();
        assert!(capture(&mut f.db, f.root.to_str().unwrap(), &audio.id).is_err());
        assert_eq!(
            read(&f.db, f.root.to_str().unwrap(), &audio.id).unwrap().0,
            audio
        );
    }

    #[test]
    fn unsafe_revisions_and_mismatched_document_ids_fail_closed() {
        let mut f = Fixture::new();
        let context = f.context();
        let first = f.snapshot(&context, 6);
        let next = f.snapshot(&context, 9);
        commit(&mut f.db, &context, None, &first, |_| Ok(())).unwrap();
        f.db.execute(
            "UPDATE phase_j_sequence_drafts_test SET revision=?1",
            [JS_SAFE],
        )
        .unwrap();
        assert!(commit(&mut f.db, &context, Some(JS_SAFE), &next, |_| Ok(())).is_err());
        assert_eq!(f.history(), 0);
        assert_eq!(stored(&f.db, &context).unwrap().unwrap().revision, JS_SAFE);
        f.db.execute("UPDATE video_sequences SET revision=?1", [JS_SAFE + 1])
            .unwrap();
        assert!(capture(&mut f.db, f.root.to_str().unwrap(), &f.project.id).is_err());
        let mut wrong = serde_json::to_value(&f.project).unwrap();
        wrong["id"] = json!("different-project");
        f.db.execute(
            "UPDATE video_sequences SET revision=1,data=?1",
            [wrong.to_string()],
        )
        .unwrap();
        assert!(capture(&mut f.db, f.root.to_str().unwrap(), &f.project.id).is_err());
    }
    include!("crossfade_store_contract_tests.rs");
    include!("crossfade_layer_clock_tests.rs");
    include!("crossfade_layer_manifest_tests.rs");
    include!("crossfade_editor_session_tests.rs");
    include!("crossfade_session_checkpoint_tests.rs");
    include!("crossfade_session_seek_tests.rs");
    include!("crossfade_preview_requests_tests.rs");
    #[cfg(windows)]
    include!("crossfade_preview_decode_tests.rs");
    #[cfg(windows)]
    include!("crossfade_composed_preview_tests.rs");
    #[cfg(windows)]
    include!("crossfade_av_preview_tests.rs");
}
