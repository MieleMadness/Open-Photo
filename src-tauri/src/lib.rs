use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

const MAX_ENTRIES: usize = 40_000;
const SUPPORTED: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "tif", "tiff", "bmp", "heic", "heif", "avif", "dng", "cr2", "nef", "arw"];

#[derive(Serialize)]
struct Photo {
    path: String,
    name: String,
    extension: String,
    bytes: u64,
    modified_unix: u64,
}

#[derive(Serialize)]
struct ScanResult {
    photos: Vec<Photo>,
    scanned_files: usize,
    truncated: bool,
}

/// Read-only folder scan. Never follows directory symlinks and never writes to the media path.
#[tauri::command]
fn scan_library(root: String) -> Result<ScanResult, String> {
    let root_path = Path::new(&root);
    if !root_path.is_dir() {
        return Err("Please select an existing directory.".to_string());
    }
    let mut pending = vec![PathBuf::from(root_path)];
    let mut photos = Vec::new();
    let mut scanned_files = 0usize;
    let mut truncated = false;

    'scan: while let Some(dir) = pending.pop() {
        let entries = fs::read_dir(&dir).map_err(|e| format!("Unable to read {}: {}", dir.display(), e))?;
        for entry in entries {
            if scanned_files >= MAX_ENTRIES {
                truncated = true;
                break 'scan;
            }
            scanned_files += 1;
            let entry = match entry { Ok(e) => e, Err(_) => continue };
            let kind = match entry.file_type() { Ok(t) => t, Err(_) => continue };
            if kind.is_symlink() { continue; }
            let path = entry.path();
            if kind.is_dir() { pending.push(path); continue; }
            if !kind.is_file() { continue; }
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
            if !SUPPORTED.contains(&ext.as_str()) { continue; }
            let metadata = match entry.metadata() { Ok(m) => m, Err(_) => continue };
            let modified_unix = metadata.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            photos.push(Photo {
                path: path.to_string_lossy().into_owned(),
                name: entry.file_name().to_string_lossy().into_owned(),
                extension: ext,
                bytes: metadata.len(),
                modified_unix,
            });
        }
    }
    photos.sort_unstable_by(|a,b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.path.cmp(&b.path)));
    Ok(ScanResult { photos, scanned_files, truncated })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![scan_library])
        .run(tauri::generate_context!())
        .expect("error while running Open-Photo");
}
