use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::{path::PathBuf, sync::Mutex};
use tauri::Emitter;

#[derive(Default)]
pub struct WatchState {
    watchers: Vec<RecommendedWatcher>,
}

/// Install filesystem notifications for the active workspace.
/// Replaces the previous watcher set without touching library files.
#[tauri::command]
pub fn set_watched_folders(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<WatchState>>,
    roots: Vec<String>,
) -> Result<(), String> {
    if roots.len() > 64 {
        return Err("Too many folders to watch".into());
    }
    let mut next = Vec::new();
    for root in roots {
        let path = PathBuf::from(root)
            .canonicalize()
            .map_err(|error| error.to_string())?;
        if !path.is_dir() {
            return Err("Watched root is not a folder".into());
        }
        let emitter = app.clone();
        let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if event.is_ok() {
                let _ = emitter.emit("openphoto-folder-changed", ());
            }
        })
        .map_err(|error| error.to_string())?;
        watcher
            .watch(&path, RecursiveMode::Recursive)
            .map_err(|error| error.to_string())?;
        next.push(watcher);
    }
    let mut guard = state.lock().map_err(|error| error.to_string())?;
    guard.watchers = next;
    Ok(())
}
