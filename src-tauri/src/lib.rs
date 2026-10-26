mod ai_embeddings;
mod ai_inference;
mod ai_models;
mod ai_preprocess;
mod ai_text;
mod cache_disk;
mod geo;
mod index;
mod similar;
mod watch;
use base64::Engine;
use image::ImageReader;
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    io::{Cursor, Write},
    sync::{Mutex, OnceLock},
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

const MAX_ENTRIES: usize = 40_000;
const PREVIEW_CACHE_LIMIT: usize = 64;
type PreviewCache = (HashMap<String, String>, VecDeque<String>);
static PREVIEW_CACHE: OnceLock<Mutex<PreviewCache>> = OnceLock::new();

fn cache() -> &'static Mutex<PreviewCache> {
    PREVIEW_CACHE.get_or_init(|| Mutex::new((HashMap::new(), VecDeque::new())))
}
fn safe_media_path(root: &str, path: &str) -> Result<PathBuf, String> {
    let root = fs::canonicalize(root).map_err(|_| "Library folder not found".to_string())?;
    let file = fs::canonicalize(path).map_err(|_| "Photo not found".to_string())?;
    if !file.starts_with(root) || !file.is_file() {
        return Err("Photo is outside the selected library".into());
    }
    let extension = file
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !SUPPORTED.contains(&extension.as_str()) {
        return Err("Unsupported media type".into());
    }
    Ok(file)
}
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
#[derive(Serialize)]
struct BulkTagResult {
    created: usize,
    skipped: usize,
    errors: Vec<String>,
}
/// Only create missing XMP sidecars. Never overwrite existing sidecars or original files.
#[tauri::command]
fn bulk_create_tags(
    root: String,
    paths: Vec<String>,
    tags: Vec<String>,
) -> Result<BulkTagResult, String> {
    if paths.is_empty() || paths.len() > 1000 {
        return Err("Select 1–1000 photos per batch".into());
    }
    if tags.is_empty()
        || tags.len() > 50
        || tags.iter().any(|t| {
            t.trim().is_empty()
                || t.len() > 128
                || t.chars().any(|c| c == '\0' || c == '\n' || c == '\r')
        })
    {
        return Err("Invalid tags; use 1–50 tags of up to 128 characters".into());
    }
    let mut output = BulkTagResult {
        created: 0,
        skipped: 0,
        errors: Vec::new(),
    };
    for path in paths {
        let file = match safe_media_path(&root, &path) {
            Ok(file) => file,
            Err(error) => {
                output.errors.push(format!("{path}: {error}"));
                continue;
            }
        };
        let sidecar = PathBuf::from(format!("{}.xmp", file.display()));
        let mut handle = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&sidecar)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                output.skipped += 1;
                continue;
            }
            Err(error) => {
                output
                    .errors
                    .push(format!("{}: {error}", sidecar.display()));
                continue;
            }
        };
        let list = tags
            .iter()
            .map(|tag| format!("<rdf:li>{}</rdf:li>", xml_escape(tag)))
            .collect::<String>();
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:subject><rdf:Bag>{list}</rdf:Bag></dc:subject>
</rdf:Description></rdf:RDF></x:xmpmeta>
"#
        );
        if let Err(error) = handle
            .write_all(xml.as_bytes())
            .and_then(|_| handle.sync_all())
        {
            output
                .errors
                .push(format!("{}: {error}", sidecar.display()));
            drop(handle);
            let _ = fs::remove_file(&sidecar);
        } else {
            output.created += 1;
        }
    }
    Ok(output)
}

const SUPPORTED: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "gif", "tif", "tiff", "bmp", "heic", "heif", "avif", "dng",
    "cr2", "nef", "arw", "mov", "mp4", "m4v", "avi", "mkv", "webm",
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
pub struct ScanResult {
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
/// Read a bounded local iPhone media file for on-device preview or playback.
/// The media stays on the user's machine; originals are never rewritten.
#[tauri::command]
fn read_iphone_media(root: String, path: String) -> Result<String, String> {
    let file = safe_media_path(&root, &path)?;
    let extension = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "heic" | "heif" | "mov" | "mp4" | "m4v") {
        return Err("This format is not eligible for inline iPhone media preview".into());
    }
    let meta = fs::metadata(&file).map_err(|e| e.to_string())?;
    if meta.len() > 48 * 1024 * 1024 {
        return Err(
            "Inline playback is limited to 48 MB; use the Windows player for larger videos".into(),
        );
    }
    let bytes = fs::read(file).map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[tauri::command]
fn preview_image(root: String, path: String, max_edge: u32) -> Result<String, String> {
    let file = safe_media_path(&root, &path)?;
    let size = fs::metadata(&file).map_err(|e| e.to_string())?.len();
    if size > 75 * 1024 * 1024 {
        return Err("Preview unavailable for files larger than 75 MB".into());
    }
    let modified = fs::metadata(&file)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let key = format!("{}:{size}:{modified}:{}", file.display(), max_edge);
    if let Ok(guard) = cache().lock() {
        if let Some(image) = guard.0.get(&key) {
            return Ok(image.clone());
        }
    }
    if let Some(bytes) = cache_disk::read(&key) {
        return Ok(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ));
    }
    let edge = max_edge.clamp(64, 1920);
    let reader = ImageReader::open(&file)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let decoded = reader
        .decode()
        .map_err(|_| "This format cannot be previewed yet".to_string())?;
    let thumb = decoded.thumbnail(edge, edge);
    let mut bytes = Cursor::new(Vec::new());
    thumb
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    let png = bytes.into_inner();
    cache_disk::write(&key, &png);
    let result = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    );
    if let Ok(mut guard) = cache().lock() {
        if !guard.0.contains_key(&key) {
            if guard.1.len() >= PREVIEW_CACHE_LIMIT {
                if let Some(old) = guard.1.pop_front() {
                    guard.0.remove(&old);
                }
            }
            guard.1.push_back(key.clone());
            guard.0.insert(key, result.clone());
        }
    }
    Ok(result)
}

/// Open a selected video with the user's configured operating-system media player.
#[tauri::command]
fn open_video(root: String, path: String) -> Result<(), String> {
    let file = safe_media_path(&root, &path)?;
    let extension = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(
        extension.as_str(),
        "mov" | "mp4" | "m4v" | "avi" | "mkv" | "webm"
    ) {
        return Err("Only video files can be opened from this action".into());
    }
    opener::open(&file).map_err(|e| format!("Unable to open video: {e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(std::sync::Mutex::new(watch::WatchState::default()))
        .invoke_handler(tauri::generate_handler![
            ai_embeddings::ai_embedding_status,
            ai_embeddings::ai_find_similar_by_photo,
            ai_embeddings::ai_index_semantic_batch,
            ai_embeddings::ai_index_semantic_image,
            ai_embeddings::ai_search_semantic_text,
            ai_inference::ai_run_semantic_image,
            ai_preprocess::ai_prepare_image,
            ai_text::ai_run_semantic_text,
            ai_models::ai_model_status,
            ai_models::ai_remove_model,
            ai_models::ai_import_model,
            scan_library,
            preview_image,
            read_iphone_media,
            bulk_create_tags,
            open_video,
            geo::gps_for_photos,
            index::scan_indexed,
            index::scan_indexed_with_changes,
            index::update_indexed_path,
            index::library_health,
            index::find_exact_duplicates,
            index::find_workspace_duplicates,
            watch::set_watched_folders,
            similar::find_similar_photos
        ])
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
    fn tags_do_not_overwrite_existing_sidecars_or_modify_originals() {
        let root = temp_root();
        let image = root.join("sample.jpg");
        fs::write(&image, b"original").unwrap();
        let sidecar = root.join("sample.jpg.xmp");
        let root_arg = root.to_string_lossy().into_owned();
        let paths = vec![image.to_string_lossy().into_owned()];
        let first =
            bulk_create_tags(root_arg.clone(), paths.clone(), vec!["Family".into()]).unwrap();
        assert_eq!(first.created, 1);
        assert!(fs::read_to_string(&sidecar).unwrap().contains("Family"));
        let second = bulk_create_tags(root_arg, paths, vec!["Other".into()]).unwrap();
        assert_eq!(second.skipped, 1);
        assert!(!fs::read_to_string(&sidecar).unwrap().contains("Other"));
        assert_eq!(fs::read(&image).unwrap(), b"original");
        fs::remove_dir_all(root).unwrap();
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
