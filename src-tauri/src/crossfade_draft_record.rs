//! Test-only Rust parity validator for the experimental TypeScript
//! crossfadeDraftRecord v1. No database, renderer or export integration.
use crate::crossfade_clock::{self, Boundary, ClipClock};
use serde_json::Value;
use std::collections::HashSet;

const MAX_CHARS: usize = 65_536;

fn fields(value: &Value, expected: &[&str]) -> bool {
    let Some(map) = value.as_object() else {
        return false;
    };
    map.len() == expected.len() && expected.iter().all(|key| map.contains_key(*key))
}
fn id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 120
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}
fn frame(value: &Value) -> Result<u64, String> {
    let Some(number) = value.as_u64() else {
        return Err("Invalid crossfade frame.".into());
    };
    if number > 9_007_199_254_740_991 {
        return Err("Unsafe crossfade frame.".into());
    }
    Ok(number)
}
/// Validates the exact expected *ordered* clip identity and project clock.
/// An absent record is an empty draft. Corrupt or stale records are errors.
pub(crate) fn validate_record(
    json: Option<&str>,
    expected_project_id: &str,
    expected_fps: u64,
    expected_clips: &[(&str, u64)],
) -> Result<Vec<(String, String, u64)>, String> {
    if !id(expected_project_id) || !matches!(expected_fps, 30 | 60) || expected_clips.len() > 200 {
        return Err("Invalid expected draft identity.".into());
    }
    let mut seen = HashSet::new();
    let mut total = 0_u64;
    for (clip_id, frames) in expected_clips {
        if !id(clip_id) || !seen.insert(*clip_id) || *frames < 2 {
            return Err("Invalid expected draft clip.".into());
        }
        total = total
            .checked_add(*frames)
            .ok_or("Draft frame total overflow.")?;
    }
    if total > 7200 * expected_fps {
        return Err("Draft exceeds two hours.".into());
    }
    let Some(json) = json else {
        return Ok(Vec::new());
    };
    if json.is_empty() || json.len() > MAX_CHARS {
        return Err("Invalid crossfade draft record size.".into());
    }
    let raw: Value = serde_json::from_str(json).map_err(|_| "Invalid crossfade draft JSON.")?;
    if !fields(
        &raw,
        &["version", "project_id", "fps", "clips", "boundaries"],
    ) || raw["version"].as_u64() != Some(1)
        || raw["project_id"].as_str() != Some(expected_project_id)
        || raw["fps"].as_u64() != Some(expected_fps)
    {
        return Err("Unsupported or mismatched crossfade draft.".into());
    }
    let clips = raw["clips"].as_array().ok_or("Malformed draft clips.")?;
    if clips.len() != expected_clips.len() {
        return Err("Stale draft clip count.".into());
    }
    for (actual, (clip_id, frames)) in clips.iter().zip(expected_clips.iter()) {
        if !fields(actual, &["id", "frames"])
            || actual["id"].as_str() != Some(*clip_id)
            || frame(&actual["frames"])? != *frames
        {
            return Err("Stale crossfade draft clip identity.".into());
        }
    }
    let boundaries = raw["boundaries"]
        .as_array()
        .ok_or("Malformed draft transitions.")?;
    if boundaries.len() > expected_clips.len().saturating_sub(1) {
        return Err("Too many draft transitions.".into());
    }
    let mut owned = Vec::with_capacity(boundaries.len());
    for edge in boundaries {
        if !fields(edge, &["from_clip_id", "to_clip_id", "frames"]) {
            return Err("Malformed draft transition fields.".into());
        }
        let from = edge["from_clip_id"]
            .as_str()
            .ok_or("Invalid from clip ID.")?;
        let to = edge["to_clip_id"].as_str().ok_or("Invalid to clip ID.")?;
        let frames = frame(&edge["frames"])?;
        if frames < (expected_fps * 8).div_ceil(100) || frames > expected_fps * 3 / 2 {
            return Err("Draft transition duration is outside supported bounds.".into());
        }
        owned.push((from.to_owned(), to.to_owned(), frames));
    }
    let clock_clips: Vec<_> = expected_clips
        .iter()
        .map(|(clip_id, frames)| ClipClock {
            id: clip_id,
            frames: *frames,
        })
        .collect();
    let clock_edges: Vec<_> = owned
        .iter()
        .map(|(from, to, frames)| Boundary {
            from_clip_id: from,
            to_clip_id: to,
            frames: *frames,
        })
        .collect();
    let clock = crossfade_clock::compile(&clock_clips, &clock_edges)?;
    if clock.total_frames > 7200 * expected_fps {
        return Err("Crossfade draft exceeds two hours.".into());
    }
    Ok(owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    const CLIPS: &[(&str, u64)] = &[("a", 90), ("b", 60), ("c", 90)];
    fn json(edges: &str) -> String {
        format!(
            r#"{{"version":1,"project_id":"project-1","fps":30,"clips":[{{"id":"a","frames":90}},{{"id":"b","frames":60}},{{"id":"c","frames":90}}],"boundaries":{edges}}}"#
        )
    }
    #[test]
    fn parity_with_typescript_record_round_trip_fixture() {
        let edges = r#"[{"from_clip_id":"a","to_clip_id":"b","frames":15},{"from_clip_id":"b","to_clip_id":"c","frames":12}]"#;
        assert_eq!(
            validate_record(Some(&json(edges)), "project-1", 30, CLIPS).unwrap(),
            vec![("a".into(), "b".into(), 15), ("b".into(), "c".into(), 12)]
        );
        assert!(validate_record(None, "project-1", 30, CLIPS)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn rejects_stale_versions_ids_frames_and_extra_fields() {
        let valid = json("[]");
        assert!(validate_record(Some(&valid), "other", 30, CLIPS).is_err());
        assert!(validate_record(Some(&valid), "project-1", 60, CLIPS).is_err());
        assert!(validate_record(
            Some(&valid),
            "project-1",
            30,
            &[("b", 60), ("a", 90), ("c", 90)]
        )
        .is_err());
        assert!(validate_record(
            Some(&valid.replace("\"version\":1", "\"version\":2")),
            "project-1",
            30,
            CLIPS
        )
        .is_err());
        assert!(validate_record(
            Some(&valid.replace("\"frames\":90", "\"frames\":91")),
            "project-1",
            30,
            CLIPS
        )
        .is_err());
        assert!(validate_record(
            Some(&valid.replace("\"fps\":30", "\"fps\":30,\"extra\":1")),
            "project-1",
            30,
            CLIPS
        )
        .is_err());
    }
    #[test]
    fn rejects_bad_edges_and_invalid_sizes() {
        for edges in [
            r#"[{"from_clip_id":"a","to_clip_id":"b","frames":2}]"#,
            r#"[{"from_clip_id":"a","to_clip_id":"c","frames":9}]"#,
            r#"[{"from_clip_id":"a","to_clip_id":"b","frames":46}]"#,
            r#"[{"from_clip_id":"a","to_clip_id":"b","frames":9},{"from_clip_id":"a","to_clip_id":"b","frames":9}]"#,
            r#"[{"from_clip_id":"a","to_clip_id":"b","frames":9,"extra":1}]"#,
            r#"[{"from_clip_id":"a","to_clip_id":"b","frames":1.5}]"#,
        ] {
            assert!(
                validate_record(Some(&json(edges)), "project-1", 30, CLIPS).is_err(),
                "{edges}"
            );
        }
        assert!(validate_record(Some(""), "project-1", 30, CLIPS).is_err());
        assert!(validate_record(Some(&" ".repeat(65537)), "project-1", 30, CLIPS).is_err());
        assert!(validate_record(Some("{"), "project-1", 30, CLIPS).is_err());
    }
    #[test]
    fn refuses_clip_overlap_exhaustion_and_invalid_expected_clock() {
        let edges = r#"[{"from_clip_id":"a","to_clip_id":"b","frames":15},{"from_clip_id":"b","to_clip_id":"c","frames":12}]"#;
        assert!(validate_record(
            Some(&json(edges)),
            "project-1",
            30,
            &[("a", 90), ("b", 20), ("c", 90)]
        )
        .is_err());
        assert!(validate_record(None, "project-1", 30, &[("same", 20), ("same", 20)]).is_err());
        assert!(validate_record(None, "project-1", 24, CLIPS).is_err());
    }
}
