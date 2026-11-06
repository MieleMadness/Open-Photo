fn manifest_fixture() -> (Fixture, String) {
    let mut f = Fixture::new();
    add_photo(&mut f);
    add_title(&mut f, 1.5, 2.5);
    add_audio(&mut f);
    f.project.transitions.push(CutTransition {
        from_clip_id: f.project.clips[1].id.clone(),
        to_clip_id: "photo-clip".into(),
        kind: CutTransitionKind::DipToBlack,
        duration_seconds: 0.4,
    });
    store(&f.db, f.root.to_str().unwrap(), &f.project, Some(1)).unwrap();
    initialize_layer_store(&f.db).unwrap();
    let from = f.project.clips[0].id.clone();
    let to = f.project.clips[1].id.clone();
    let data = prepare_layer_manifest(
        &mut f.db,
        f.root.to_str().unwrap(),
        &f.project.id,
        &[Boundary {
            from_clip_id: &from,
            to_clip_id: &to,
            frames: 15,
        }],
    )
    .unwrap();
    (f, data)
}
fn manifest_count(f: &Fixture) -> i64 {
    f.db.query_row(
        "SELECT count(*) FROM phase_j_layer_manifests_test",
        [],
        |r| r.get(0),
    )
    .unwrap()
}
#[test]
fn mixed_manifests_roundtrip_immutably_without_changing_native_or_v1_storage() {
    let (mut f, data) = manifest_fixture();
    let raw = f.raw_project();
    f.db.execute_batch("PRAGMA user_version=42").unwrap();
    initialize_layer_store(&f.db).unwrap();
    let id = save_layer_manifest(
        &mut f.db,
        f.root.to_str().unwrap(),
        &f.project.id,
        &data,
        |_| Ok(()),
    )
    .unwrap();
    f.reopen();
    f.reopen();
    let loaded =
        load_layer_manifest(&mut f.db, f.root.to_str().unwrap(), &f.project.id, &id).unwrap();
    assert_eq!(encode_layer_manifest(&loaded).unwrap(), data);
    assert_eq!(loaded.project_json, raw);
    assert_eq!(loaded.timing.total_frames, 165);
    assert!(loaded.timing.clips[2].photo);
    assert_eq!(loaded.timing.titles.len(), 1);
    assert_eq!(loaded.timing.audio.len(), 1);
    assert_eq!(f.raw_project(), raw);
    assert_eq!(manifest_count(&f), 1);
    assert_eq!(
        f.db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        42
    );
    assert!(capture(&mut f.db, f.root.to_str().unwrap(), &f.project.id).is_err());
    assert_eq!(f.history(), 0);
    assert_eq!(
        f.db.query_row(
            "SELECT count(*) FROM phase_j_sequence_drafts_test",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}
#[test]
fn manifest_retries_canonicalize_json_and_preserve_distinct_proposals() {
    let (mut f, data) = manifest_fixture();
    let root = f.root.to_str().unwrap().to_owned();
    let project = f.project.id.clone();
    let id = save_layer_manifest(&mut f.db, &root, &project, &data, |_| Ok(())).unwrap();
    let pretty =
        serde_json::to_string_pretty(&serde_json::from_str::<Value>(&data).unwrap()).unwrap();
    let mut other = Connection::open(f.root.join("prototype.db")).unwrap();
    assert_eq!(
        save_layer_manifest(&mut other, &root, &project, &pretty, |_| panic!(
            "retry writes"
        ))
        .unwrap(),
        id
    );
    let proposal = prepare_layer_manifest(&mut other, &root, &project, &[]).unwrap();
    let next = save_layer_manifest(&mut other, &root, &project, &proposal, |_| Ok(())).unwrap();
    assert_ne!(id, next);
    assert_eq!(manifest_count(&f), 2);
    assert_eq!(
        load_layer_manifest(&mut f.db, &root, &project, &id)
            .unwrap()
            .timing
            .total_frames,
        165
    );
    assert_eq!(
        load_layer_manifest(&mut f.db, &root, &project, &next)
            .unwrap()
            .timing
            .total_frames,
        180
    );
    f.project.transitions.clear();
    f.project.frame_rate = 60;
    store(&f.db, &root, &f.project, Some(2)).unwrap();
    let from = f.project.clips[0].id.clone();
    let middle = f.project.clips[1].id.clone();
    let first = Boundary {
        from_clip_id: &from,
        to_clip_id: &middle,
        frames: 15,
    };
    let second = Boundary {
        from_clip_id: &middle,
        to_clip_id: "photo-clip",
        frames: 15,
    };
    let forward =
        prepare_layer_manifest(&mut f.db, &root, &project, &[first.clone(), second.clone()])
            .unwrap();
    let reverse = prepare_layer_manifest(&mut f.db, &root, &project, &[second, first]).unwrap();
    assert_eq!(forward, reverse);
    let id60 = save_layer_manifest(&mut f.db, &root, &project, &forward, |_| Ok(())).unwrap();
    assert_eq!(
        load_layer_manifest(&mut f.db, &root, &project, &id60)
            .unwrap()
            .timing
            .total_frames,
        330
    );
    let mut reordered: Value = serde_json::from_str(&forward).unwrap();
    reordered["boundaries"].as_array_mut().unwrap().reverse();
    assert!(
        save_layer_manifest(&mut f.db, &root, &project, &reordered.to_string(), |_| Ok(
            ()
        ))
        .is_err()
    );
    assert_eq!(manifest_count(&f), 3);
}
#[test]
fn forged_manifest_fields_and_unknown_or_duplicate_fields_are_refused() {
    let (mut f, data) = manifest_fixture();
    let original: Value = serde_json::from_str(&data).unwrap();
    for (pointer, replacement) in [
        ("/version", json!(2)),
        ("/timing_policy", json!(2)),
        ("/root", json!("another-root")),
        ("/project_id", json!("another-project")),
        ("/project_revision", json!(JS_SAFE + 1)),
        ("/document_hash", json!("0".repeat(64))),
        ("/project_json", json!("{}")),
        ("/boundaries/0/frames", json!(0)),
        ("/boundaries/0/to_clip_id", json!("missing")),
        ("/timing/total_frames", json!(166)),
        ("/timing/total_samples", json!(0)),
        ("/timing/clips/2/photo", json!(false)),
        ("/timing/clips/0/frames/end", json!(59)),
        ("/timing/audio/0/source_samples/start", json!(0)),
        ("/timing/audio/0/gain_percent", json!(200)),
        ("/timing/titles/0/frames/start", json!(0)),
        ("/timing/clips/1/dip_out_frames", json!(0)),
    ] {
        let mut value = original.clone();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            save_layer_manifest(
                &mut f.db,
                f.root.to_str().unwrap(),
                &f.project.id,
                &value.to_string(),
                |_| Ok(())
            )
            .is_err(),
            "{pointer}"
        );
    }
    for pointer in [
        "",
        "/boundaries/0",
        "/timing",
        "/timing/clips/0",
        "/timing/clips/0/frames",
        "/timing/titles/0",
        "/timing/audio/0",
    ] {
        let mut value = original.clone();
        value
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("future_field".into(), json!(1));
        assert!(save_layer_manifest(
            &mut f.db,
            f.root.to_str().unwrap(),
            &f.project.id,
            &value.to_string(),
            |_| Ok(())
        )
        .is_err());
    }
    let duplicate = data.replacen("{", "{\"version\":1,", 1);
    for invalid in [duplicate, " ".repeat(LAYER_MANIFEST_LIMIT + 1)] {
        assert!(save_layer_manifest(
            &mut f.db,
            f.root.to_str().unwrap(),
            &f.project.id,
            &invalid,
            |_| Ok(())
        )
        .is_err());
    }
    assert_eq!(manifest_count(&f), 0);
}
#[test]
fn same_revision_layer_and_source_edits_invalidate_saved_and_pending_manifests() {
    let (mut f, data) = manifest_fixture();
    let raw = f.raw_project();
    let original: Value = serde_json::from_str(&raw).unwrap();
    let id = save_layer_manifest(
        &mut f.db,
        f.root.to_str().unwrap(),
        &f.project.id,
        &data,
        |_| Ok(()),
    )
    .unwrap();
    for (pointer, replacement) in [
        ("/titles/0/text", json!("changed")),
        ("/audio_tracks/0/gain_percent", json!(90)),
        ("/audio_sources/0/sha256", json!("d".repeat(64))),
        ("/assets/1/media/width", json!(128)),
        ("/assets/0/sha256", json!("e".repeat(64))),
        ("/clips/0/edits/brightness", json!(1)),
        ("/transitions/0/duration_seconds", json!(0.2)),
    ] {
        let mut value = original.clone();
        *value.pointer_mut(pointer).unwrap() = replacement;
        f.db.execute(
            "UPDATE video_sequences SET data=?1 WHERE id=?2",
            params![value.to_string(), f.project.id],
        )
        .unwrap();
        assert!(
            load_layer_manifest(&mut f.db, f.root.to_str().unwrap(), &f.project.id, &id).is_err(),
            "{pointer}"
        );
        assert!(
            save_layer_manifest(
                &mut f.db,
                f.root.to_str().unwrap(),
                &f.project.id,
                &data,
                |_| Ok(())
            )
            .is_err(),
            "{pointer}"
        );
        assert_eq!(manifest_count(&f), 1);
    }
    f.db.execute(
        "UPDATE video_sequences SET data=?1 WHERE id=?2",
        params![raw, f.project.id],
    )
    .unwrap();
    store(&f.db, f.root.to_str().unwrap(), &f.project, Some(2)).unwrap();
    assert!(load_layer_manifest(&mut f.db, f.root.to_str().unwrap(), &f.project.id, &id).is_err());
}
#[test]
fn corrupt_future_and_foreign_records_are_preserved_instead_of_replaced() {
    let (mut f, data) = manifest_fixture();
    let root = f.root.to_str().unwrap().to_owned();
    let project = f.project.id.clone();
    let id = save_layer_manifest(&mut f.db, &root, &project, &data, |_| Ok(())).unwrap();
    let other = Fixture::new();
    assert!(load_layer_manifest(&mut f.db, other.root.to_str().unwrap(), &project, &id).is_err());
    assert!(load_layer_manifest(&mut f.db, &root, "another-project", &id).is_err());
    assert!(load_layer_manifest(&mut f.db, &root, &project, "../bad").is_err());
    for (version, stored) in [(2, data.as_str()), (1, "broken payload")] {
        f.db.execute(
            "UPDATE phase_j_layer_manifests_test SET version=?1,data=?2",
            params![version, stored],
        )
        .unwrap();
        assert!(load_layer_manifest(&mut f.db, &root, &project, &id).is_err());
        assert!(save_layer_manifest(&mut f.db, &root, &project, &data, |_| Ok(())).is_err());
        assert_eq!(
            layer_row(&f.db, &root_key(&root).unwrap(), &project, &id)
                .unwrap()
                .unwrap(),
            (version, stored.to_owned())
        );
    }
    f.db.execute_batch("ALTER TABLE phase_j_layer_manifests_test ADD COLUMN future TEXT")
        .unwrap();
    assert!(initialize_layer_store(&f.db).is_err());
    assert!(save_layer_manifest(&mut f.db, &root, &project, &data, |_| Ok(())).is_err());
    assert_eq!(manifest_count(&f), 1);
}
#[test]
fn manifest_faults_rollback_and_lost_reply_retries_return_the_same_id() {
    for at in [
        "after_manifest_insert",
        "before_manifest_commit",
        "after_manifest_commit",
    ] {
        let (mut f, data) = manifest_fixture();
        let raw = f.raw_project();
        assert!(save_layer_manifest(
            &mut f.db,
            f.root.to_str().unwrap(),
            &f.project.id,
            &data,
            |point| if point == at {
                Err("injected".into())
            } else {
                Ok(())
            }
        )
        .is_err());
        f.reopen();
        f.reopen();
        assert_eq!(manifest_count(&f), i64::from(at == "after_manifest_commit"));
        let id = save_layer_manifest(
            &mut f.db,
            f.root.to_str().unwrap(),
            &f.project.id,
            &data,
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(id, payload_hash(&data));
        assert_eq!(manifest_count(&f), 1);
        assert_eq!(f.raw_project(), raw);
    }
}
#[test]
fn layer_manifest_crash_child() {
    let Ok(root) = std::env::var("OPEN_PHOTO_LAYER_CRASH_ROOT") else {
        return;
    };
    let id = std::env::var("OPEN_PHOTO_LAYER_CRASH_ID").unwrap();
    let at = std::env::var("OPEN_PHOTO_LAYER_CRASH_AT").unwrap();
    let mut db = Connection::open(PathBuf::from(&root).join("prototype.db")).unwrap();
    let (project, _) = read(&db, &root, &id).unwrap();
    let data = prepare_layer_manifest(
        &mut db,
        &root,
        &id,
        &[Boundary {
            from_clip_id: &project.clips[0].id,
            to_clip_id: &project.clips[1].id,
            frames: 15,
        }],
    )
    .unwrap();
    save_layer_manifest(&mut db, &root, &id, &data, |point| {
        if point == at {
            std::process::exit(78);
        }
        Ok(())
    })
    .unwrap();
    panic!("checkpoint not reached");
}
#[test]
fn abrupt_manifest_exit_preserves_atomicity_and_idempotent_retries() {
    for at in [
        "after_manifest_insert",
        "before_manifest_commit",
        "after_manifest_commit",
    ] {
        let (mut f, data) = manifest_fixture();
        let raw = f.raw_project();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "video_sequence::phase_j_sequence_binding::tests::layer_manifest_crash_child",
                "--nocapture",
            ])
            .env("OPEN_PHOTO_LAYER_CRASH_ROOT", &f.root)
            .env("OPEN_PHOTO_LAYER_CRASH_ID", &f.project.id)
            .env("OPEN_PHOTO_LAYER_CRASH_AT", at)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(78),
            "{at}: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        f.reopen();
        f.reopen();
        assert_eq!(manifest_count(&f), i64::from(at == "after_manifest_commit"));
        let id = save_layer_manifest(
            &mut f.db,
            f.root.to_str().unwrap(),
            &f.project.id,
            &data,
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(manifest_count(&f), 1);
        assert_eq!(f.raw_project(), raw);
        load_layer_manifest(&mut f.db, f.root.to_str().unwrap(), &f.project.id, &id).unwrap();
    }
}
