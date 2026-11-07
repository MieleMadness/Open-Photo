#[cfg(test)]
mod paging_tests {
    use super::*;
    fn fixture() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        schema(&db).unwrap();
        db.execute_batch("CREATE TABLE user_data(key TEXT PRIMARY KEY,value TEXT NOT NULL)")
            .unwrap();
        db.execute(
            "INSERT INTO user_data VALUES('openphoto.workspaces',?1)",
            [serde_json::json!([
                {"id":"w","roots":["unavailable-root"]}, {"id":"other","roots":[]}
            ])
            .to_string()],
        )
        .unwrap();
        db
    }
    fn add(db: &Connection, workspace: &str, count: i64) {
        db.execute("WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM seq WHERE n<?2)
          INSERT INTO xmp_operations(workspace,root,path,candidate,candidate_hash,state)
          SELECT ?1,'unavailable-root','photo-' || ?1 || '-' || n || '-' || (SELECT count(*) FROM xmp_operations), 'candidate', ?3, 'review_required' FROM seq",
          params![workspace,count,hash(b"candidate")]).unwrap();
    }
    #[test]
    fn all_older_reviews_are_reachable_beyond_the_previous_thousand_record_limit() {
        let db = fixture();
        add(&db, "w", 1205);
        add(&db, "other", 70);
        let mut before = None;
        let mut seen = Vec::new();
        loop {
            let page = review_page_on(&db, "w", false, before).unwrap();
            assert!(page.items.len() <= REVIEW_PAGE_SIZE);
            assert!(page.items.iter().all(|r| r.path.starts_with("photo-w-")));
            seen.extend(page.items.iter().map(|r| r.id));
            if page.next_before.is_none() {
                break;
            }
            assert_eq!(page.next_before, page.items.last().map(|r| r.id));
            before = page.next_before;
        }
        assert_eq!(seen.len(), 1205);
        assert!(seen.windows(2).all(|p| p[0] > p[1]));
        assert_eq!(seen.last(), Some(&1));
    }
    #[test]
    fn summaries_do_not_load_xml_and_one_bad_document_does_not_hide_the_page() {
        let db = fixture();
        add(&db, "w", 2);
        // The metadata query must not even deserialize these large/wrong-type
        // snapshots. Opening that individual record will refuse the bad value.
        db.execute("UPDATE xmp_operations SET candidate=zeroblob(1048576),original=zeroblob(1048576) WHERE id=1", []).unwrap();
        let page = review_page_on(&db, "w", false, None).unwrap();
        assert_eq!(page.items.len(), 2);
        let json = serde_json::to_value(&page).unwrap();
        for row in json["items"].as_array().unwrap() {
            assert_eq!(row.as_object().unwrap().len(), 4);
            assert!(row.get("candidate").is_none());
            assert!(row.get("original").is_none());
        }
        assert!(review_record_on(&db, "w", 1).is_err());
        assert!(review_record_on(&db, "w", 2)
            .unwrap()
            .integrity_error
            .is_none());
    }
    #[test]
    fn pending_filter_keeps_interruptions_and_history_includes_completed_records() {
        let db = fixture();
        add(&db, "w", 9);
        for (i, state) in [
            "prepared",
            "review_required",
            "applying",
            "backed_up",
            "restoring",
            "published",
            "applied",
            "restored",
            "cancelled",
        ]
        .iter()
        .enumerate()
        {
            db.execute(
                "UPDATE xmp_operations SET state=?1 WHERE id=?2",
                params![state, i as i64 + 1],
            )
            .unwrap();
        }
        let pending = review_page_on(&db, "w", false, None).unwrap();
        assert_eq!(
            pending.items.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec![5, 4, 3, 2, 1]
        );
        assert_eq!(review_page_on(&db, "w", true, None).unwrap().items.len(), 9);
        for before in [0, -1] {
            assert!(review_page_on(&db, "w", true, Some(before)).is_err());
        }
    }
    #[test]
    fn fresh_inserts_do_not_duplicate_older_pages_and_appear_after_refresh() {
        let db = fixture();
        add(&db, "w", 51);
        let first = review_page_on(&db, "w", false, None).unwrap();
        assert_eq!(first.items.len(), 50);
        add(&db, "w", 1);
        let older = review_page_on(&db, "w", false, first.next_before).unwrap();
        assert_eq!(older.items.len(), 1);
        assert_eq!(older.items[0].id, 1);
        assert!(older.next_before.is_none());
        let refreshed = review_page_on(&db, "w", false, None).unwrap();
        assert_eq!(refreshed.items[0].id, 52);
    }
    #[test]
    fn record_details_remain_workspace_scoped_offline_and_integrity_checked() {
        let db = fixture();
        add(&db, "w", 1);
        db.execute(
            "UPDATE xmp_operations SET candidate='damaged' WHERE id=1",
            [],
        )
        .unwrap();
        assert!(review_record_on(&db, "w", 1)
            .unwrap()
            .integrity_error
            .is_some());
        assert!(review_record_on(&db, "other", 1).is_err());
        assert!(review_record_on(&db, "missing", 1).is_err());
        assert!(review_page_on(&db, "missing", true, None).is_err());
        assert!(review_page_on(&db, "other", true, None)
            .unwrap()
            .items
            .is_empty());
    }

    #[test]
    fn removed_root_preserves_stored_versions_and_journal_only_cancellation() {
        let mut db = fixture();
        add(&db, "w", 1);
        db.execute(
            "UPDATE xmp_operations SET original='original',original_hash=?1 WHERE id=1",
            [hash(b"original")],
        )
        .unwrap();
        db.execute(
            "UPDATE user_data SET value=?1 WHERE key='openphoto.workspaces'",
            [serde_json::json!([{"id":"w","roots":[]}]).to_string()],
        )
        .unwrap();
        let saved = review_record_on(&db, "w", 1).unwrap();
        assert!(saved.integrity_error.is_none());
        assert_eq!(saved.original.as_deref(), Some("original"));
        assert_eq!(saved.candidate, "candidate");
        assert!(review_snapshot_on(&mut db, "w", 1).is_err());
        review_decide_on(&mut db, "w", 1, "cancelled", &saved.candidate_hash).unwrap();
        let retained = review_record_on(&db, "w", 1).unwrap();
        assert_eq!(retained.state, "cancelled");
        assert_eq!(retained.original, saved.original);
        assert_eq!(retained.candidate, saved.candidate);
    }

    #[test]
    fn deleting_workspace_denies_stored_versions_and_decisions_without_changing_records() {
        let mut db = fixture();
        add(&db, "w", 1);
        let saved = review_record_on(&db, "w", 1).unwrap();
        db.execute(
            "UPDATE user_data SET value='[]' WHERE key='openphoto.workspaces'",
            [],
        )
        .unwrap();
        assert!(review_record_on(&db, "w", 1).is_err());
        assert!(review_decide_on(&mut db, "w", 1, "cancelled", &saved.candidate_hash).is_err());
        assert_eq!(load_review(&db, "w", 1).unwrap().state, "review_required");
    }
}
