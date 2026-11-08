use super::*;

#[test]
fn portable_files_are_bounded_validated_and_published_without_overwriting_existing_data() {
    let (directory, settings, projects, id) = render_fixture(false);
    let current = load(&settings, &projects, &id);
    let document = edit(
        &settings,
        &projects,
        "one",
        Action::Export {
            id: id.clone(),
            expected_project: current.project.clone(),
            expected_revision: 1,
        },
    )
    .unwrap()
    .portable
    .unwrap();
    let copy = directory.join("proposal.json");
    write_portable_copy(&document, &copy).unwrap();
    assert!(write_portable_copy(&document, &copy).is_err());
    assert_eq!(fs::read_to_string(&copy).unwrap(), document);
    assert!(write_portable_copy(&document, &directory.join("wrong.mp4")).is_err());
    let cleared = save(&settings, &projects, &id, &current, vec![], false).unwrap();
    let imported = edit(
        &settings,
        &projects,
        "one",
        Action::ImportFile {
            id: id.clone(),
            expected_project: cleared.project,
            expected_revision: 2,
            path: copy.to_string_lossy().into_owned(),
            rebase: false,
        },
    )
    .unwrap();
    assert_eq!(imported.revision, 3);
    let mut future: serde_json::Value = serde_json::from_str(&document).unwrap();
    future["schema_version"] = serde_json::json!(99);
    let raw = future.to_string();
    let path = directory.join("future.json");
    fs::write(&path, &raw).unwrap();
    assert!(write_portable_copy(&raw, &directory.join("unsupported-copy.json")).is_err());
    assert!(edit(
        &settings,
        &projects,
        "one",
        Action::ImportFile {
            id: id.clone(),
            expected_project: imported.project.clone(),
            expected_revision: 3,
            path: path.to_string_lossy().into_owned(),
            rebase: true
        }
    )
    .is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), raw);
    let too_large = directory.join("large.json");
    fs::write(&too_large, vec![b' '; MAX_PORTABLE + 1]).unwrap();
    assert!(edit(
        &settings,
        &projects,
        "one",
        Action::ImportFile {
            id: id.clone(),
            expected_project: imported.project,
            expected_revision: 3,
            path: too_large.to_string_lossy().into_owned(),
            rebase: false
        }
    )
    .is_err());
    assert_eq!(load(&settings, &projects, &id).revision, 3);
    assert!(!directory.join("unsupported-copy.json").exists());
    drop(settings);
    drop(projects);
    fs::remove_dir_all(directory).unwrap();
}

#[cfg(windows)]
fn transition_junction(link: &Path, target: &Path) {
    use std::os::windows::process::CommandExt;
    let output = std::process::Command::new("cmd.exe")
        .args(["/c", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .creation_flags(0x08000000)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}
fn render_fixture(alias: bool) -> (PathBuf, Connection, Connection, String) {
    let (settings, projects, id) = fixture();
    let directory = std::env::temp_dir().join(format!(
        "openphoto-transition-authority-{}",
        video_project::id()
    ));
    fs::create_dir(&directory).unwrap();
    let a = directory.join("root-a");
    let b = directory.join("root-b");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    image::RgbImage::from_pixel(32, 48, image::Rgb([10, 30, 50]))
        .save(a.join("offline.png"))
        .unwrap();
    fs::copy(a.join("offline.png"), b.join("offline.png")).unwrap();
    let root = if alias {
        let path = directory.join("root-alias");
        #[cfg(windows)]
        transition_junction(&path, &a);
        path
    } else {
        a
    };
    settings
        .execute(
            "UPDATE user_data SET value=?1",
            [serde_json::json!([{"id":"one","root":root}]).to_string()],
        )
        .unwrap();
    let mut record = super::super::read(&projects, "one", &id).unwrap();
    let (bytes, hash) = video_project::fingerprint(&root.join("offline.png"), || false).unwrap();
    record.project.assets[0].bytes = bytes;
    record.project.assets[0].sha256 = hash;
    record
        .bindings
        .insert("photo".into(), root.to_string_lossy().into_owned());
    super::super::store(&projects, "one", &record.project, &record.bindings, Some(1)).unwrap();
    let initial = load(&settings, &projects, &id);
    save(
        &settings,
        &projects,
        &id,
        &initial,
        vec![edge(&projects, &id)],
        false,
    )
    .unwrap();
    let settings_path = directory.join("settings.sqlite");
    let projects_path = directory.join("projects.sqlite");
    settings
        .execute("VACUUM INTO ?1", [settings_path.to_str().unwrap()])
        .unwrap();
    projects
        .execute("VACUUM INTO ?1", [projects_path.to_str().unwrap()])
        .unwrap();
    (
        directory,
        Connection::open(settings_path).unwrap(),
        Connection::open(projects_path).unwrap(),
        id,
    )
}
#[test]
fn production_render_binding_rechecks_revocation_same_revision_bindings_and_draft_selection() {
    let (directory, settings, projects, id) = render_fixture(false);
    let current = load(&settings, &projects, &id);
    let snapshot = render_transition_snapshot(
        &settings,
        &projects,
        "one",
        &id,
        &current.project,
        current.revision,
        || false,
    )
    .unwrap();
    let binding = snapshot.transition.as_ref().unwrap();
    assert!(binding.current().unwrap());
    let saved: String = settings
        .query_row("SELECT value FROM user_data", [], |r| r.get(0))
        .unwrap();
    settings
        .execute("UPDATE user_data SET value='[]'", [])
        .unwrap();
    assert!(binding.current().is_err());
    settings
        .execute("UPDATE user_data SET value=?1", [saved])
        .unwrap();
    let bindings: String = projects
        .query_row("SELECT bindings FROM workspace_video_projects", [], |r| {
            r.get(0)
        })
        .unwrap();
    projects
        .execute("UPDATE workspace_video_projects SET bindings='{}'", [])
        .unwrap();
    assert!(!binding.current().unwrap());
    projects
        .execute(
            "UPDATE workspace_video_projects SET bindings=?1",
            [bindings],
        )
        .unwrap();
    let raw: String = projects
        .query_row("SELECT data FROM workspace_video_projects", [], |r| {
            r.get(0)
        })
        .unwrap();
    let mut changed = video_sequence::parse(raw.as_bytes()).unwrap();
    changed.title = "same revision edit".into();
    projects
        .execute(
            "UPDATE workspace_video_projects SET data=?1",
            [serde_json::to_string(&changed).unwrap()],
        )
        .unwrap();
    assert!(!binding.current().unwrap());
    projects
        .execute("UPDATE workspace_video_projects SET data=?1", [raw])
        .unwrap();
    assert!(binding.current().unwrap());
    save(&settings, &projects, &id, &current, vec![], false).unwrap();
    assert!(!binding.current().unwrap());
    assert!(snapshot.revalidate(|| false).is_err());
    drop(snapshot);
    drop(settings);
    drop(projects);
    fs::remove_dir_all(directory).unwrap();
}
#[cfg(windows)]
#[test]
fn production_render_binding_rejects_changed_canonical_root_even_with_identical_settings_and_bytes()
{
    let (directory, settings, projects, id) = render_fixture(true);
    let current = load(&settings, &projects, &id);
    let snapshot = render_transition_snapshot(
        &settings,
        &projects,
        "one",
        &id,
        &current.project,
        current.revision,
        || false,
    )
    .unwrap();
    let binding = snapshot.transition.as_ref().unwrap();
    assert!(binding.current().unwrap());
    let alias = directory.join("root-alias");
    fs::remove_dir(&alias).unwrap();
    transition_junction(&alias, &directory.join("root-b"));
    assert!(!binding.current().unwrap());
    assert!(snapshot.revalidate(|| false).is_err());
    assert_eq!(
        fs::read(directory.join("root-a/offline.png")).unwrap(),
        fs::read(directory.join("root-b/offline.png")).unwrap()
    );
    fs::remove_dir(alias).unwrap();
    drop(snapshot);
    drop(settings);
    drop(projects);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn portable_roundtrip_refuses_old_client_parsing_and_never_imports_access_grants() {
    let (settings, db, id) = fixture();
    let initial = load(&settings, &db, &id);
    let saved = save(&settings, &db, &id, &initial, vec![edge(&db, &id)], false).unwrap();
    let exported = edit(
        &settings,
        &db,
        "one",
        Action::Export {
            id: id.clone(),
            expected_project: saved.project.clone(),
            expected_revision: 1,
        },
    )
    .unwrap()
    .portable
    .unwrap();
    assert!(video_sequence::parse(exported.as_bytes()).is_err());
    assert!(!exported.contains("offline-root"));
    let cleared = save(&settings, &db, &id, &saved, vec![], false).unwrap();
    let imported = edit(
        &settings,
        &db,
        "one",
        Action::Import {
            id: id.clone(),
            expected_project: cleared.project,
            expected_revision: 2,
            document: exported.clone(),
            rebase: false,
        },
    )
    .unwrap();
    assert_eq!(imported.revision, 3);
    assert_eq!(
        imported.record.as_ref().unwrap().edges,
        vec![edge(&db, &id)]
    );
    let mut future: serde_json::Value = serde_json::from_str(&exported).unwrap();
    future["schema_version"] = serde_json::json!(99);
    assert!(edit(
        &settings,
        &db,
        "one",
        Action::Import {
            id: id.clone(),
            expected_project: imported.project.clone(),
            expected_revision: 3,
            document: future.to_string(),
            rebase: true
        }
    )
    .is_err());
    let mut portable: PortableTransitions = serde_json::from_str(&exported).unwrap();
    let mut project = video_sequence::parse(portable.project_json.as_bytes()).unwrap();
    project.assets[0].relative_path = "untrusted-import.png".into();
    portable.project_json = serde_json::to_string(&project).unwrap();
    let changed = serde_json::to_string(&portable).unwrap();
    assert!(edit(
        &settings,
        &db,
        "one",
        Action::Import {
            id: id.clone(),
            expected_project: imported.project.clone(),
            expected_revision: 3,
            document: changed.clone(),
            rebase: false
        }
    )
    .is_err());
    // Explicitly adapting edit decisions still cannot import the portable media path or bindings.
    edit(
        &settings,
        &db,
        "one",
        Action::Import {
            id: id.clone(),
            expected_project: imported.project,
            expected_revision: 3,
            document: changed,
            rebase: true,
        },
    )
    .unwrap();
    let stored = super::super::read(&db, "one", &id).unwrap();
    assert_eq!(stored.project.assets[0].relative_path, "offline.png");
    assert!(stored.bindings.is_empty());
}
#[test]
fn failed_head_publication_rolls_back_the_immutable_revision_and_reopen_keeps_receipts() {
    let (settings, db, id) = fixture();
    let initial = load(&settings, &db, &id);
    db.execute_batch("CREATE TRIGGER transition_test_failure BEFORE INSERT ON workspace_transition_heads BEGIN SELECT RAISE(ABORT,'fault'); END").unwrap();
    assert!(save(&settings, &db, &id, &initial, vec![edge(&db, &id)], false).is_err());
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM workspace_transition_revisions",
            [],
            |r| r.get::<_, u64>(0)
        )
        .unwrap(),
        0
    );
    db.execute_batch("DROP TRIGGER transition_test_failure")
        .unwrap();
    let directory = std::env::temp_dir().join(format!(
        "openphoto-transition-reopen-{}",
        video_project::id()
    ));
    fs::create_dir(&directory).unwrap();
    let path = directory.join("projects.sqlite");
    db.execute("VACUUM INTO ?1", [path.to_str().unwrap()])
        .unwrap();
    let first = Connection::open(&path).unwrap();
    let second = Connection::open(&path).unwrap();
    let a = load(&settings, &first, &id);
    let b = load(&settings, &second, &id);
    save(&settings, &first, &id, &a, vec![edge(&first, &id)], false).unwrap();
    assert!(save(&settings, &second, &id, &b, vec![], false).is_err());
    drop(first);
    drop(second);
    let reopened = Connection::open(&path).unwrap();
    let after = load(&settings, &reopened, &id);
    assert_eq!(after.revision, 1);
    assert_eq!(after.record.unwrap().edges, vec![edge(&reopened, &id)]);
    drop(reopened);
    fs::remove_file(path).unwrap();
    fs::remove_dir(directory).unwrap();
}
#[test]
fn recovery_requires_explicit_rebase_after_project_changes_and_revision_exhaustion_is_read_only() {
    let (settings, db, id) = fixture();
    let initial = load(&settings, &db, &id);
    let saved = save(&settings, &db, &id, &initial, vec![edge(&db, &id)], false).unwrap();
    let mut project = super::super::read(&db, "one", &id).unwrap().project;
    project.title = "Changed project".into();
    super::super::store(&db, "one", &project, &Bindings::new(), Some(1)).unwrap();
    let stale = load(&settings, &db, &id);
    assert!(edit(
        &settings,
        &db,
        "one",
        Action::Recover {
            id: id.clone(),
            expected_project: stale.project.clone(),
            expected_revision: 1,
            history_revision: 1,
            rebase: false
        }
    )
    .is_err());
    let recovered = edit(
        &settings,
        &db,
        "one",
        Action::Recover {
            id: id.clone(),
            expected_project: stale.project,
            expected_revision: 1,
            history_revision: 1,
            rebase: true,
        },
    )
    .unwrap();
    assert_eq!(recovered.revision, 2);
    let mut maximum = recovered.record.unwrap();
    maximum.revision = SAFE_REVISION;
    db.execute(
        "INSERT INTO workspace_transition_revisions VALUES('one',?1,?2,?3)",
        params![id, SAFE_REVISION, serde_json::to_string(&maximum).unwrap()],
    )
    .unwrap();
    db.execute(
        "UPDATE workspace_transition_heads SET revision=?1",
        [SAFE_REVISION],
    )
    .unwrap();
    let current = load(&settings, &db, &id);
    assert!(save(&settings, &db, &id, &current, vec![], false).is_err());
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM workspace_transition_revisions",
            [],
            |r| r.get::<_, u64>(0)
        )
        .unwrap(),
        3
    );
    assert_eq!(saved.revision, 1);
}

#[test]
fn production_seek_uses_saved_workspace_clock_and_rejects_stale_and_exclusive_end() {
    let (settings, db, id) = fixture();
    let initial = load(&settings, &db, &id);
    let saved = save(&settings, &db, &id, &initial, vec![edge(&db, &id)], false).unwrap();
    let seek_action = |point| Action::Seek {
        id: id.clone(),
        expected_project: saved.project.clone(),
        expected_revision: 1,
        point,
    };
    let frame = edit(
        &settings,
        &db,
        "one",
        seek_action(crate::sequence_frame_seek::SeekPoint::Frame(84)),
    )
    .unwrap()
    .seek
    .unwrap();
    assert_eq!(frame.video.len(), 2);
    assert_eq!(frame.video[1].weight.numerator, 1);
    assert_eq!(frame.video[1].weight.denominator, 12);
    assert_eq!(frame.samples.start, 134400);
    assert_eq!(frame.samples.end, 136000);
    let seconds = edit(
        &settings,
        &db,
        "one",
        seek_action(crate::sequence_frame_seek::SeekPoint::Seconds(2.8)),
    )
    .unwrap()
    .seek
    .unwrap();
    assert_eq!(frame, seconds);
    assert!(edit(
        &settings,
        &db,
        "one",
        seek_action(crate::sequence_frame_seek::SeekPoint::Frame(174))
    )
    .is_err());
    assert!(edit(
        &settings,
        &db,
        "one",
        seek_action(crate::sequence_frame_seek::SeekPoint::Seconds(f64::NAN))
    )
    .is_err());
    save(&settings, &db, &id, &saved, vec![], false).unwrap();
    assert!(edit(
        &settings,
        &db,
        "one",
        seek_action(crate::sequence_frame_seek::SeekPoint::Frame(84))
    )
    .is_err());
}

fn fixture() -> (Connection, Connection, String) {
    let settings = Connection::open_in_memory().unwrap();
    settings
        .execute_batch("CREATE TABLE user_data(key TEXT PRIMARY KEY,value TEXT NOT NULL)")
        .unwrap();
    settings
        .execute(
            "INSERT INTO user_data VALUES('openphoto.workspaces',?1)",
            [
                serde_json::json!([{"id":"one","roots":["offline-root","second-offline-root"]}])
                    .to_string(),
            ],
        )
        .unwrap();
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE projects(id TEXT PRIMARY KEY,root TEXT,title TEXT,revision INTEGER,data TEXT,updated INTEGER); CREATE TABLE project_history(id INTEGER PRIMARY KEY,project_id TEXT,revision INTEGER,saved_at INTEGER,data TEXT)").unwrap();
    super::super::initialize(&db).unwrap();
    let mut record = super::super::new_record();
    let asset = crate::sequence_media::Asset {
        id: "photo".into(),
        relative_path: "offline.png".into(),
        bytes: 1,
        sha256: "0".repeat(64),
        media: crate::sequence_media::Media::Photo {
            width: 32,
            height: 48,
        },
    };
    let first = crate::sequence_media::default_clip(&asset).unwrap();
    let mut second = first.clone();
    second.id = "second".into();
    record.project.assets.push(asset);
    record.project.clips = vec![first, second];
    super::super::store(&db, "one", &record.project, &record.bindings, None).unwrap();
    (settings, db, record.project.id)
}
fn edge(db: &Connection, id: &str) -> Edge {
    let project = super::super::read(db, "one", id).unwrap().project;
    Edge {
        from_clip_id: project.clips[0].id.clone(),
        to_clip_id: project.clips[1].id.clone(),
        frames: 6,
    }
}
fn load(settings: &Connection, db: &Connection, id: &str) -> State {
    edit(settings, db, "one", Action::Load { id: id.into() }).unwrap()
}
fn save(
    settings: &Connection,
    db: &Connection,
    id: &str,
    current: &State,
    edges: Vec<Edge>,
    rebase: bool,
) -> Result<State, String> {
    edit(
        settings,
        db,
        "one",
        Action::Save {
            id: id.into(),
            expected_project: current.project.clone(),
            expected_revision: current.revision,
            edges,
            rebase,
        },
    )
}
#[test]
fn production_store_roundtrips_offline_multi_root_proposals_without_touching_v3_projects() {
    let (settings, db, id) = fixture();
    let original: String = db
        .query_row("SELECT data FROM workspace_video_projects", [], |r| {
            r.get(0)
        })
        .unwrap();
    let empty = load(&settings, &db, &id);
    assert_eq!(empty.revision, 0);
    let stored = save(&settings, &db, &id, &empty, vec![edge(&db, &id)], false).unwrap();
    assert_eq!(stored.revision, 1);
    assert!(!stored.stale);
    assert_eq!(stored.timing.unwrap().total_frames, 174);
    let loaded = load(&settings, &db, &id);
    assert_eq!(loaded.record.unwrap().edges, vec![edge(&db, &id)]);
    assert_eq!(
        original,
        db.query_row("SELECT data FROM workspace_video_projects", [], |r| r
            .get::<_, String>(0))
            .unwrap()
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM workspace_video_history", [], |r| r
            .get::<_, u64>(0))
            .unwrap(),
        0
    );
    // No physical originals are required or accessed by edit-decision storage.
    assert!(!Path::new("offline-root/offline.png").exists());
}
#[test]
fn production_store_requires_exact_project_and_draft_receipts_and_explicit_rebase() {
    let (settings, db, id) = fixture();
    let initial = load(&settings, &db, &id);
    let saved = save(&settings, &db, &id, &initial, vec![edge(&db, &id)], false).unwrap();
    assert!(save(&settings, &db, &id, &initial, vec![], false).is_err());
    // Retry is idempotent when the current receipt and normalized edits agree.
    assert_eq!(
        save(&settings, &db, &id, &saved, vec![edge(&db, &id)], false)
            .unwrap()
            .revision,
        1
    );
    let mut project = super::super::read(&db, "one", &id).unwrap().project;
    project.title = "same revision change".into();
    db.execute(
        "UPDATE workspace_video_projects SET data=?1",
        [serde_json::to_string(&project).unwrap()],
    )
    .unwrap();
    assert!(save(&settings, &db, &id, &saved, vec![], true).is_err());
    let stale = load(&settings, &db, &id);
    assert!(stale.stale);
    assert!(stale.timing.is_none());
    assert!(save(&settings, &db, &id, &stale, vec![], false).is_err());
    let rebased = save(&settings, &db, &id, &stale, vec![], true).unwrap();
    assert_eq!(rebased.revision, 2);
    assert!(!rebased.stale);
    settings
        .execute("UPDATE user_data SET value='[]'", [])
        .unwrap();
    assert!(save(&settings, &db, &id, &rebased, vec![], false).is_err());
}
#[test]
fn production_history_is_append_only_and_recovery_appends_with_provenance() {
    let (settings, db, id) = fixture();
    let initial = load(&settings, &db, &id);
    let saved = save(&settings, &db, &id, &initial, vec![edge(&db, &id)], false).unwrap();
    let cleared = save(&settings, &db, &id, &saved, vec![], false).unwrap();
    assert!(db
        .execute("DELETE FROM workspace_transition_revisions", [])
        .is_err());
    assert!(db
        .execute("UPDATE workspace_transition_revisions SET data='{}'", [])
        .is_err());
    let recovered = edit(
        &settings,
        &db,
        "one",
        Action::Recover {
            id: id.clone(),
            expected_project: cleared.project,
            expected_revision: 2,
            history_revision: 1,
            rebase: false,
        },
    )
    .unwrap();
    assert_eq!(recovered.revision, 3);
    let record = recovered.record.unwrap();
    assert_eq!(record.recovered_from, Some(1));
    assert_eq!(record.edges, vec![edge(&db, &id)]);
    let first = edit(
        &settings,
        &db,
        "one",
        Action::History {
            id: id.clone(),
            before_revision: None,
            limit: 2,
        },
    )
    .unwrap();
    assert_eq!(
        first.history.iter().map(|r| r.revision).collect::<Vec<_>>(),
        vec![3, 2]
    );
    let second = edit(
        &settings,
        &db,
        "one",
        Action::History {
            id,
            before_revision: Some(2),
            limit: 2,
        },
    )
    .unwrap();
    assert_eq!(
        second
            .history
            .iter()
            .map(|r| r.revision)
            .collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM workspace_transition_revisions",
            [],
            |r| r.get::<_, u64>(0)
        )
        .unwrap(),
        3
    );
}
#[test]
fn future_records_and_altered_schemas_are_preserved_and_refused() {
    let (settings, db, id) = fixture();
    let initial = load(&settings, &db, &id);
    let saved = save(&settings, &db, &id, &initial, vec![edge(&db, &id)], false).unwrap();
    let mut future = serde_json::to_value(saved.record.unwrap()).unwrap();
    future["schema_version"] = serde_json::json!(99);
    future["revision"] = serde_json::json!(2);
    let raw = future.to_string();
    db.execute(
        "INSERT INTO workspace_transition_revisions VALUES('one',?1,2,?2)",
        params![id, raw],
    )
    .unwrap();
    db.execute("UPDATE workspace_transition_heads SET revision=2", [])
        .unwrap();
    assert!(edit(&settings, &db, "one", Action::Load { id: id.clone() }).is_err());
    assert!(save(&settings, &db, &id, &initial, vec![], true).is_err());
    assert_eq!(
        db.query_row(
            "SELECT data FROM workspace_transition_revisions WHERE revision=2",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        raw
    );
    db.execute_batch("DROP TRIGGER workspace_transition_no_delete")
        .unwrap();
    assert!(edit(&settings, &db, "one", Action::Load { id }).is_err());
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM workspace_transition_revisions",
            [],
            |r| r.get::<_, u64>(0)
        )
        .unwrap(),
        2
    );
}
#[test]
fn invalid_edges_layer_overruns_and_cross_workspace_reads_leave_history_unchanged() {
    let (settings, db, id) = fixture();
    let initial = load(&settings, &db, &id);
    let valid = edge(&db, &id);
    for invalid in [
        Edge {
            frames: 0,
            ..valid.clone()
        },
        Edge {
            frames: 90,
            ..valid.clone()
        },
        Edge {
            from_clip_id: valid.to_clip_id.clone(),
            to_clip_id: valid.from_clip_id.clone(),
            frames: 6,
        },
    ] {
        assert!(save(&settings, &db, &id, &initial, vec![invalid], false).is_err());
    }
    assert!(save(
        &settings,
        &db,
        &id,
        &initial,
        vec![valid.clone(), valid.clone()],
        false
    )
    .is_err());
    assert!(edit(&settings, &db, "other", Action::Load { id: id.clone() }).is_err());
    let mut project = super::super::read(&db, "one", &id).unwrap().project;
    project.titles.push(video_sequence::TitleOverlay {
        id: "title".into(),
        text: "End".into(),
        start_seconds: 5.8,
        end_seconds: 6.0,
        x_percent: 50,
        y_percent: 50,
    });
    super::super::store(&db, "one", &project, &Bindings::new(), Some(1)).unwrap();
    let changed = load(&settings, &db, &id);
    assert!(save(&settings, &db, &id, &changed, vec![valid], false).is_err());
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM workspace_transition_revisions",
            [],
            |r| r.get::<_, u64>(0)
        )
        .unwrap(),
        0
    );
}
#[test]
fn an_empty_or_short_main_timeline_can_load_and_explicitly_clear_its_stale_proposal() {
    let (settings, db, id) = fixture();
    let initial = load(&settings, &db, &id);
    save(&settings, &db, &id, &initial, vec![edge(&db, &id)], false).unwrap();
    let mut project = super::super::read(&db, "one", &id).unwrap().project;
    project.clips.clear();
    super::super::store(&db, "one", &project, &Bindings::new(), Some(1)).unwrap();
    let stale = load(&settings, &db, &id);
    assert!(stale.stale);
    let cleared = save(&settings, &db, &id, &stale, vec![], true).unwrap();
    assert!(!cleared.stale);
    assert!(cleared.timing.is_none());
    assert!(cleared.record.unwrap().edges.is_empty());
    project
        .clips
        .push(crate::sequence_media::default_clip(&project.assets[0]).unwrap());
    project.clips[0].source_out =
        video_project::VideoTime::from_seconds(0.08, "1/1000000").unwrap();
    super::super::store(&db, "one", &project, &Bindings::new(), Some(2)).unwrap();
    let short = load(&settings, &db, &id);
    let cleared = save(&settings, &db, &id, &short, vec![], true).unwrap();
    assert_eq!(cleared.timing.as_ref().unwrap().total_frames, 2);
    let loaded = load(&settings, &db, &id);
    assert!(!loaded.stale);
    assert_eq!(loaded.revision, 3);
}
#[test]
fn immutable_proposal_bases_survive_rolling_timeline_history_and_recover_atomically() {
    let (settings, db, id) = fixture();
    let initial = load(&settings, &db, &id);
    let original = super::super::read(&db, "one", &id).unwrap().project;
    let saved = save(&settings, &db, &id, &initial, vec![edge(&db, &id)], false).unwrap();
    let mut project = original.clone();
    for revision in 1..=25 {
        project.title = format!("Later timeline {revision}");
        super::super::store(&db, "one", &project, &Bindings::new(), Some(revision)).unwrap();
    }
    let current = load(&settings, &db, &id);
    assert!(current.stale);
    let portable = edit(
        &settings,
        &db,
        "one",
        Action::ExportHistory {
            id: id.clone(),
            expected_project: current.project.clone(),
            expected_revision: saved.revision,
            history_revision: 1,
        },
    )
    .unwrap()
    .portable
    .unwrap();
    let historical = parse_portable(&portable).unwrap();
    assert_eq!(
        video_sequence::parse(historical.project_json.as_bytes())
            .unwrap()
            .title,
        original.title
    );
    let request = TransitionRenderRequest {
        workspace_id: "one".into(),
        id: id.clone(),
        expected_project: current.project,
        expected_revision: 1,
    };
    let recovered = recover_project_copy(&settings, &db, request, 1).unwrap();
    assert_ne!(recovered.project.id, id);
    assert_eq!(recovered.project.clips, original.clips);
    assert!(recovered.transition_proposal);
    assert_eq!(
        load(&settings, &db, &recovered.project.id)
            .record
            .unwrap()
            .edges,
        saved.record.unwrap().edges
    );
    assert_eq!(
        super::super::read(&db, "one", &id).unwrap().project.title,
        "Later timeline 25"
    );
    let roots = user_data::workspace_roots_from(&settings, "one").unwrap();
    let count: i64 = db
        .query_row("SELECT count(*) FROM workspace_video_projects", [], |r| {
            r.get(0)
        })
        .unwrap();
    db.execute_batch("CREATE TRIGGER reject_new_transition BEFORE INSERT ON workspace_transition_revisions BEGIN SELECT RAISE(ABORT,'injected publication failure'); END;").unwrap();
    assert!(import_project_copy(&settings, &db, "one", &roots, portable.as_bytes()).is_err());
    assert_eq!(
        db.query_row("SELECT count(*) FROM workspace_video_projects", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        count
    );
    assert!(db
        .execute("UPDATE workspace_transition_bases SET data='{}'", [])
        .is_err());
    assert!(db
        .execute("DELETE FROM workspace_transition_bases", [])
        .is_err());
}
