//! Fixed signed channels. Choosing Preview never changes Stable's major-only policy.
pub(crate) fn selected() -> Result<String, String> {
    let default = if option_env!("OPENPHOTO_UPDATE_CHANNEL") == Some("preview") {
        "preview"
    } else {
        "stable"
    };
    let value = crate::user_data::setting("openphoto.updateChannel")?.unwrap_or(default.into());
    if matches!(value.as_str(), "stable" | "preview") {
        Ok(value)
    } else {
        Err("Choose Stable or Preview updates".into())
    }
}
pub(crate) fn endpoint(channel: &str) -> Result<&'static str, String> {
    match channel{
    "stable"=>Ok("https://github.com/MieleMadness/Open-Photo/releases/download/stable-channel/latest.json"),
    "preview"=>Ok("https://github.com/MieleMadness/Open-Photo/releases/download/preview-channel/latest.json"),
    _=>Err("Unknown update channel".into())}
}
fn version(value: &str) -> Option<(u64, u64, u64)> {
    let parts = value.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts.iter().any(|p| {
            p.is_empty()
                || p.len() > 1 && p.starts_with('0')
                || !p.bytes().all(|b| b.is_ascii_digit())
        })
    {
        return None;
    }
    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    ))
}
pub(crate) fn accepts(channel: &str, current: &str, remote: &str) -> bool {
    let (Some(current), Some(remote)) = (version(current), version(remote)) else {
        return false;
    };
    match channel {
        "preview" => remote > current,
        "stable" => remote.0 > current.0 && remote.1 == 0 && remote.2 == 0,
        _ => false,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_accepts_newer_patches_without_downgrades_or_unvalidated_version_forms() {
        assert!(accepts("preview", "0.56.22", "0.56.23"));
        for bad in ["0.56.22", "0.56.21", "0.56.23-beta.1", "00.56.23", "x"] {
            assert!(!accepts("preview", "0.56.22", bad));
        }
    }
    #[test]
    fn stable_remains_major_only_and_endpoints_are_whitelisted() {
        assert!(accepts("stable", "0.56.22", "1.0.0"));
        for bad in ["0.56.23", "1.1.0", "1.0.1", "1.0.0-beta"] {
            assert!(!accepts("stable", "0.56.22", bad));
        }
        assert!(endpoint("https://other").is_err());
    }
}
