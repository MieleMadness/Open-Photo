use serde::Serialize;
use base64::Engine;
use image::ImageReader;
use std::io::Cursor;
use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

const MAX_ENTRIES: usize = 40_000;
const SUPPORTED: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "gif", "tif", "tiff", "bmp", "heic", "heif", "avif", "dng",
    "cr2", "nef", "arw",
];

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
        let entries =
            fs::read_dir(&dir).map_err(|e| format!("Unable to read {}: {}", dir.display(), e))?;
        for entry in entries {
            if scanned_files >= MAX_ENTRIES {
                truncated = true;
                break 'scan;
            }
            scanned_files += 1;
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let kind = match entry.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            if kind.is_symlink() {
                continue;
            }
            let path = entry.path();
            if kind.is_dir() {
                pending.push(path);
                continue;
            }
            if !kind.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !SUPPORTED.contains(&ext.as_str()) {
                continue;
            }
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let modified_unix = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            photos.push(Photo {
                path: path.to_string_lossy().into_owned(),
                name: entry.file_name().to_string_lossy().into_owned(),
                extension: ext,
                bytes: metadata.len(),
                modified_unix,
            });
        }
    }
    photos.sort_unstable_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.path.cmp(&b.path))
    });
    Ok(ScanResult {
        photos,
        scanned_files,
        truncated,
    })
}

/// Decodes images locally and returns bounded image previews without exposing the filesystem.
#[tauri::command]
fn preview_image(root: String, path: String, max_edge: u32) -> Result<String, String> {
    let root = fs::canonicalize(root).map_err(|_| "Library folder not found".to_string())?;
    let file = fs::canonicalize(path).map_err(|_| "Photo not found".to_string())?;
    if !file.starts_with(&root) || !file.is_file() {
        return Err("Photo is outside the selected library".into());
    }
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if !SUPPORTED.contains(&ext.as_str()) {
        return Err("Unsupported media type".into());
    }
    let size = fs::metadata(&file).map_err(|e| e.to_string())?.len();
    if size > 75 * 1024 * 1024 {
        return Err("Preview unavailable for files larger than 75 MB".into());
    }
    let edge = max_edge.clamp(64, 1920);
    let reader = ImageReader::open(&file).map_err(|e| e.to_string())?
        .with_guessed_format().map_err(|e| e.to_string())?;
    let decoded = reader.decode().map_err(|_| "This format cannot be previewed yet".to_string())?;
    let thumb = decoded.thumbnail(edge, edge);
    let mut bytes = Cursor::new(Vec::new());
    thumb.write_to(&mut bytes, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes.into_inner())))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![scan_library, preview_image])
        .run(tauri::generate_context!())
        .expect("error while running Open-Photo");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_root() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = env::temp_dir().join(format!("openphoto-scan-{}-{}", std::process::id(), stamp));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rejects_missing_directory() {
        assert!(scan_library("this-path-should-not-exist-openphoto-TEST".into()).is_err());
    }

    #[test]
    fn only_indexes_supported_images_and_never_modifies_files() {
        let root = temp_root();
        let nested = root.join("subfolder");
        fs::create_dir_all(&nested).unwrap();
        let original = b"test original bytes";
        let image = nested.join("picture.JPG");
        fs::write(&image, original).unwrap();
        fs::write(root.join("notes.txt"), b"not a photo").unwrap();
        let result = scan_library(root.to_string_lossy().into_owned()).unwrap();
        assert_eq!(result.photos.len(), 1);
        assert_eq!(result.photos[0].name, "picture.JPG");
        assert_eq!(result.photos[0].extension, "jpg");
        assert_eq!(fs::read(&image).unwrap(), original);
        assert!(root.join("notes.txt").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn preview_rejects_files_outside_selected_library() {
        let root = temp_root();
        let other = temp_root();
        let image = other.join("photo.jpg");
        fs::write(&image, b"test").unwrap();
        let result = preview_image(
            root.to_string_lossy().into_owned(),
            image.to_string_lossy().into_owned(),
            400,
        );
        assert!(result.is_err());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(other).unwrap();
    }

    #[test]
    fn empty_folder_returns_empty_result() {
        let root = temp_root();
        let result = scan_library(root.to_string_lossy().into_owned()).unwrap();
        assert!(result.photos.is_empty());
        assert!(!result.truncated);
        fs::remove_dir_all(root).unwrap();
    }
}
