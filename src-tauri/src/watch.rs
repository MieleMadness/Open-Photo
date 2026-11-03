use notify::{
    event::{ModifyKind, RenameMode},
    EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use serde::Serialize;
use std::{path::PathBuf, sync::Mutex};
use tauri::Emitter;

#[derive(Serialize, Clone)]
struct RenamedFile {
    from: String,
    to: String,
}

#[derive(Serialize, Clone)]
struct ChangedFile {
    paths: Vec<String>,
}

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
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if let Ok(details) = event {
                    if matches!(
                        details.kind,
                        EventKind::Modify(ModifyKind::Name(RenameMode::Both))
                    ) && details.paths.len() == 2
                    {
                        let rename = RenamedFile {
                            from: details.paths[0].to_string_lossy().into_owned(),
                            to: details.paths[1].to_string_lossy().into_owned(),
                        };
                        let _ = emitter.emit("openphoto-file-renamed", rename);
                    }
                    let _ = emitter.emit(
                        "openphoto-media-event",
                        ChangedFile {
                            paths: details
                                .paths
                                .iter()
                                .map(|path| path.to_string_lossy().into_owned())
                                .collect(),
                        },
                    );
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
