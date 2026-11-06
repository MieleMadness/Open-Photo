#[cfg(test)]
#[cfg(windows)]
mod staging_observation_tests {
    use super::step2_apply_tests::Fixture;
    use super::*;

    #[test]
    fn handled_failure_stages_are_visible_without_changing_approval_or_retry() {
        let mut fixture = Fixture::new();
        let (row, before) = fixture.proposal();
        apply_on(
            &mut fixture.db,
            "w",
            row.id,
            &before.fingerprint,
            &row.candidate_hash,
            |point, _| {
                if point == "after_stage_write" {
                    Err("disposable stage-write interruption".into())
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        fixture.reopen();
        let after = review_snapshot_on(&mut fixture.db, "w", row.id).unwrap();
        assert!(after.can_apply);
        assert_eq!(
            before.fingerprint, after.fingerprint,
            "observations are not authorization"
        );
        let inventory = serde_json::to_value(after.staging.as_ref().unwrap()).unwrap();
        assert_eq!(inventory["entries"].as_array().unwrap().len(), 1);
        assert_eq!(inventory["entries"][0]["kind"], "file");
        let stage = fixture
            .root
            .join(inventory["entries"][0]["name"].as_str().unwrap());
        assert_eq!(fs::read_to_string(&stage).unwrap(), row.candidate);
        assert_eq!(
            fs::read_to_string(fixture.sidecar()).unwrap(),
            row.original.clone().unwrap()
        );
        fixture.apply(&row, &after).unwrap();
        assert_eq!(fs::read_to_string(stage).unwrap(), row.candidate);
        assert_eq!(
            fs::read_to_string(fixture.sidecar()).unwrap(),
            row.candidate
        );
    }

    #[test]
    fn actual_process_exit_keeps_stage_observable_and_explicit_restore_preserves_it() {
        let mut fixture = Fixture::new();
        let (row, _) = fixture.proposal();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "xmp_transactions::step2_apply_tests::abrupt_apply_exit_child",
                "--nocapture",
            ])
            .env("OPENPHOTO_APPLY_CRASH_ROOT", &fixture.root)
            .env("OPENPHOTO_APPLY_CRASH_BOUNDARY", "after_stage_flush")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(77), "{output:?}");
        fixture.reopen();
        fixture.reopen();
        let observed = review_snapshot_on(&mut fixture.db, "w", row.id).unwrap();
        assert!(!observed.can_apply);
        assert!(observed.can_restore);
        assert!(!fixture.sidecar().exists());
        let inventory = serde_json::to_value(observed.staging.as_ref().unwrap()).unwrap();
        assert_eq!(inventory["entries"].as_array().unwrap().len(), 1);
        let stage = fixture
            .root
            .join(inventory["entries"][0]["name"].as_str().unwrap());
        assert_eq!(fs::read_to_string(&stage).unwrap(), row.candidate);
        restore_on(
            &mut fixture.db,
            "w",
            row.id,
            &observed.fingerprint,
            &row.candidate_hash,
            |_, _| Ok(()),
        )
        .unwrap();
        assert_eq!(fs::read_to_string(stage).unwrap(), row.candidate);
        assert_eq!(
            fs::read_to_string(fixture.sidecar()).unwrap(),
            row.original.unwrap()
        );
    }
}
