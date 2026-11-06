//! Authorize individual validated videos for Tauri's range-aware asset protocol.
use tauri::Manager;

#[tauri::command]
pub fn video_source(app: tauri::AppHandle, root: String, path: String) -> Result<String, String> {
    let path = validated_video(&root, &path)?;
    app.asset_protocol_scope()
        .allow_file(&path)
        .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

pub(crate) fn validated_video(root: &str, path: &str) -> Result<std::path::PathBuf, String> {
    let path = crate::safe_media_path(root, path)?;
    if !matches!(
        path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "mov" | "mp4" | "m4v" | "avi" | "mkv" | "webm"
    ) {
        return Err("Select a video for playback".into());
    }
    Ok(path)
}
