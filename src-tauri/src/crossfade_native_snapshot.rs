//! Test-only crossfade snapshot wire validator. Never writes or reads files.
//! The expected identity must eventually come from a native verified project
//! manifest, NOT from fields supplied by the same untrusted saved snapshot.
use crate::crossfade_draft_record;
use serde_json::Value;

fn fields(value: &Value, expected: &[&str]) -> bool {
    let Some(map) = value.as_object() else {
        return false;
    };
    map.len() == expected.len() && expected.iter().all(|key| map.contains_key(*key))
}
fn valid_source_identity(value: &Value, expected_ids: &[(&str, u64)]) -> bool {
    if !fields(value, &["version", "clips"]) || value["version"].as_u64() != Some(1) {
        return false;
    }
    let Some(clips) = value["clips"].as_array() else {
        return false;
    };
    if clips.len() != expected_ids.len() {
        return false;
    }
    clips.iter().zip(expected_ids).all(|(clip, (id, _))| {
        if !fields(
            clip,
            &[
                "clip_id",
                "asset_id",
                "sha256",
                "bytes",
                "source_in",
                "source_out",
            ],
        ) || clip["clip_id"].as_str() != Some(*id)
            || clip["asset_id"].as_str().is_none_or(str::is_empty)
            || !clip["sha256"].as_str().is_some_and(|hash| {
                hash.len() == 64
                    && hash
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
            || clip["bytes"].as_u64().is_none_or(|bytes| bytes == 0)
        {
            return false;
        }
        for key in ["source_in", "source_out"] {
            let mark = &clip[key];
            if !fields(mark, &["ticks", "time_base"]) {
                return false;
            }
            if mark["ticks"].as_str().is_none_or(|s| {
                s.is_empty() || s.len() > 20 || !s.bytes().all(|b| b.is_ascii_digit())
            }) {
                return false;
            }
            if mark["time_base"].as_str().is_none_or(|base| {
                let Some((numerator, denominator)) = base.split_once('/') else {
                    return true;
                };
                numerator.is_empty()
                    || denominator.is_empty()
                    || numerator.len() > 20
                    || denominator.len() > 20
                    || numerator.starts_with('0')
                    || denominator.starts_with('0')
                    || !numerator.bytes().all(|b| b.is_ascii_digit())
                    || !denominator.bytes().all(|b| b.is_ascii_digit())
            }) {
                return false;
            }
        }
        true
    })
}

/// Validate the full snapshot envelope, saved project revision and native
/// source identity before accepting its embedded versioned crossfade record.
/// The expected source identity is independently supplied by the caller.
/// This function is NOT a native command or production persistence API.
pub(crate) fn validate_snapshot(
    json: &str,
    expected_revision: u64,
    expected_source: &Value,
    expected_project_id: &str,
    expected_fps: u64,
    expected_clips: &[(&str, u64)],
) -> Result<Vec<(String, String, u64)>, String> {
    if json.is_empty() || json.len() > 131_072 {
        return Err("Invalid crossfade snapshot size.".into());
    }
    let value: Value =
        serde_json::from_str(json).map_err(|_| "Invalid crossfade snapshot JSON.")?;
    if !fields(
        &value,
        &["version", "project_revision", "source_identity", "draft"],
    ) || value["version"].as_u64() != Some(1)
        || value["project_revision"].as_u64() != Some(expected_revision)
    {
        return Err("Unsupported or stale crossfade snapshot.".into());
    }
    let saved_source = &value["source_identity"];
    if !valid_source_identity(saved_source, expected_clips)
        || !valid_source_identity(expected_source, expected_clips)
        || saved_source != expected_source
    {
        return Err("Crossfade source identity changed.".into());
    }
    let record = value["draft"]
        .as_str()
        .ok_or("Crossfade snapshot draft must be a JSON string.")?;
    crossfade_draft_record::validate_record(
        Some(record),
        expected_project_id,
        expected_fps,
        expected_clips,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CLIPS: &[(&str, u64)] = &[("a", 60), ("b", 60)];

    fn source() -> Value {
        json!({
            "version": 1,
            "clips": [
                {
                    "clip_id": "a", "asset_id": "asset-a",
                    "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "bytes": 1000,
                    "source_in": {"ticks":"0","time_base":"1/1000000"},
                    "source_out": {"ticks":"2000000","time_base":"1/1000000"}
                },
                {
                    "clip_id": "b", "asset_id": "asset-b",
                    "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "bytes": 2000,
                    "source_in": {"ticks":"0","time_base":"1/1000000"},
                    "source_out": {"ticks":"2000000","time_base":"1/1000000"}
                }
            ]
        })
    }

    fn snapshot(identity: &Value) -> String {
        let draft = json!({
            "version":1, "project_id":"project-1", "fps":30,
            "clips":[{"id":"a","frames":60},{"id":"b","frames":60}],
            "boundaries":[{"from_clip_id":"a","to_clip_id":"b","frames":12}]
        })
        .to_string();
        json!({
            "version":1,
            "project_revision":5,
            "source_identity":identity,
            "draft":draft
        })
        .to_string()
    }

    #[test]
    fn accepts_full_source_and_frame_bound_snapshot() {
        assert_eq!(
            validate_snapshot(&snapshot(&source()), 5, &source(), "project-1", 30, CLIPS),
            Ok(vec![("a".into(), "b".into(), 12)])
        );
    }

    #[test]
    fn rejects_old_revision_wrong_project_and_changed_sources() {
        let saved = snapshot(&source());
        assert!(validate_snapshot(&saved, 6, &source(), "project-1", 30, CLIPS).is_err());
        assert!(validate_snapshot(&saved, 5, &source(), "other", 30, CLIPS).is_err());
        let mut changed = source();
        changed["clips"][0]["sha256"] =
            json!("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc");
        assert!(validate_snapshot(&saved, 5, &changed, "project-1", 30, CLIPS).is_err());
        let mut trimmed = source();
        trimmed["clips"][0]["source_out"]["ticks"] = json!("1999999");
        assert!(validate_snapshot(&saved, 5, &trimmed, "project-1", 30, CLIPS).is_err());
    }

    #[test]
    fn rejects_unsupported_and_malformed_envelopes() {
        let saved = snapshot(&source());
        let mut wrong: Value = serde_json::from_str(&saved).unwrap();
        wrong["version"] = json!(2);
        assert!(validate_snapshot(&wrong.to_string(), 5, &source(), "project-1", 30, CLIPS).is_err());
        wrong["version"] = json!(1);
        wrong["unexpected"] = json!(true);
        assert!(validate_snapshot(&wrong.to_string(), 5, &source(), "project-1", 30, CLIPS).is_err());
        assert!(validate_snapshot("{", 5, &source(), "project-1", 30, CLIPS).is_err());
    }

    #[test]
    fn rejects_invalid_or_extra_source_properties() {
        let mut wrong = source();
        wrong["clips"][0]["bytes"] = json!(0);
        assert!(validate_snapshot(&snapshot(&wrong), 5, &wrong, "project-1", 30, CLIPS).is_err());
        let mut wrong = source();
        wrong["clips"][0]["unexpected"] = json!(true);
        assert!(validate_snapshot(&snapshot(&wrong), 5, &wrong, "project-1", 30, CLIPS).is_err());
        let mut wrong = source();
        wrong["clips"][0]["source_in"]["time_base"] = json!("0/1");
        assert!(validate_snapshot(&snapshot(&wrong), 5, &wrong, "project-1", 30, CLIPS).is_err());
    }

    #[test]
    fn rejects_corrupt_inner_draft_even_when_source_matches() {
        let mut corrupted: Value = serde_json::from_str(&snapshot(&source())).unwrap();
        corrupted["draft"] = json!("{}");
        assert!(validate_snapshot(
            &corrupted.to_string(),
            5,
            &source(),
            "project-1",
            30,
            CLIPS
        )
        .is_err());
    }
}
