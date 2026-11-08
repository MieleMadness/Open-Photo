fn admission_settings(roots: &[String]) -> Connection {
    let settings = Connection::open_in_memory().unwrap();
    settings
        .execute_batch("CREATE TABLE user_data(key TEXT PRIMARY KEY,value TEXT NOT NULL)")
        .unwrap();
    let data = serde_json::json!([{"id":"one","root":roots[0],"roots":roots}]).to_string();
    settings
        .execute(
            "INSERT INTO user_data VALUES('openphoto.workspaces',?1)",
            [data],
        )
        .unwrap();
    settings
}
#[test]
fn admission_rejects_revoked_workspace_stale_revision_and_same_revision_changes() {
    let (root, record) = fixture();
    let projects = db();
    store(&projects, "one", &record.project, &record.bindings, None).unwrap();
    let settings = admission_settings(&[root.to_string_lossy().into_owned()]);
    let (receipt, _) =
        phase_j_workspace_admission(&settings, &projects, "one", &record.project.id, 1).unwrap();
    assert!(phase_j_workspace_current(&settings, &projects, &receipt).unwrap());
    assert!(
        phase_j_workspace_admission(&settings, &projects, "two", &record.project.id, 1).is_err()
    );
    assert!(
        phase_j_workspace_admission(&settings, &projects, "one", &record.project.id, 2).is_err()
    );
    let mut changed = record.project.clone();
    changed.title = "same revision edit".into();
    projects
        .execute(
            "UPDATE workspace_video_projects SET data=?1",
            [serde_json::to_string(&changed).unwrap()],
        )
        .unwrap();
    assert!(!phase_j_workspace_current(&settings, &projects, &receipt).unwrap());
    let mut future = serde_json::to_value(&changed).unwrap();
    future["schema_version"] = serde_json::json!(999);
    let preserved = future.to_string();
    projects
        .execute("UPDATE workspace_video_projects SET data=?1", [&preserved])
        .unwrap();
    assert!(
        phase_j_workspace_admission(&settings, &projects, "one", &record.project.id, 1).is_err()
    );
    assert_eq!(
        projects
            .query_row("SELECT data FROM workspace_video_projects", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        preserved
    );
    settings
        .execute("UPDATE user_data SET value='[]'", [])
        .unwrap();
    assert!(phase_j_workspace_current(&settings, &projects, &receipt).is_err());
    assert_eq!(
        fs::read(root.join("source.mp4")).unwrap(),
        b"original unchanged"
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn admission_preserves_multi_root_bindings_and_rejects_removed_or_changed_sources() {
    let (root, mut record) = fixture();
    let (second, other) = fixture();
    let mut asset = other.project.assets[0].clone();
    asset.id = "second-asset".into();
    fs::write(second.join("source.mp4"), b"second original unchanged").unwrap();
    let (bytes, hash) = video_project::fingerprint(&second.join("source.mp4"), || false).unwrap();
    asset.bytes = bytes;
    asset.sha256 = hash;
    let mut clip = other.project.clips[0].clone();
    clip.id = "second-clip".into();
    clip.asset_id = asset.id.clone();
    record
        .bindings
        .insert(asset.id.clone(), second.to_string_lossy().into_owned());
    record.project.assets.push(asset);
    record.project.clips.push(clip);
    let projects = db();
    store(&projects, "one", &record.project, &record.bindings, None).unwrap();
    let settings = admission_settings(&[
        root.to_string_lossy().into_owned(),
        second.to_string_lossy().into_owned(),
    ]);
    let (receipt, snapshot) =
        phase_j_workspace_admission(&settings, &projects, "one", &record.project.id, 1).unwrap();
    assert_eq!(snapshot.roots.len(), 2);
    let old = settings
        .query_row("SELECT value FROM user_data", [], |r| r.get::<_, String>(0))
        .unwrap();
    settings
        .execute(
            "UPDATE user_data SET value=?1",
            [serde_json::json!([{"id":"one","root":root.to_str().unwrap()}]).to_string()],
        )
        .unwrap();
    assert!(phase_j_workspace_current(&settings, &projects, &receipt).is_err());
    settings
        .execute("UPDATE user_data SET value=?1", [old])
        .unwrap();
    fs::write(second.join("source.mp4"), b"changed source").unwrap();
    assert!(phase_j_workspace_current(&settings, &projects, &receipt).is_err());
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(second).unwrap();
}
