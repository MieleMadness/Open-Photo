mod ai_embeddings;
mod ai_faces;
mod ai_inference;
mod ai_models;
mod ai_ocr;
mod ai_preprocess;
mod ai_runtime;
mod ai_text;
mod ai_validation;
mod cache_disk;
mod edit_export;
mod geo;
mod image_decode;
mod index;
mod media_jobs;
mod media_stream;
mod safe_output;
mod search;
mod search_providers;
mod similar;
mod user_data;
mod video_edit_preview;
mod video_engine;
mod video_export;
#[cfg(test)]
mod video_integration_tests;
mod video_poster;
mod video_presets;
mod video_preview;
mod video_probe;
mod video_project;
mod video_render;
mod video_sequence;
mod video_sequence_render;
mod video_timing;
mod video_workspace;
mod watch;
mod xmp;
use base64::Engine;
use exif::{DateTime as ExifDateTime, In, Reader as ExifReader, Tag, Value};
use nom_exif::{read_exif as read_nom_exif, EntryValue as NomEntryValue, ExifTag as NomExifTag};
use raw_files::{Orientation as RawOrientation, RawFile};
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    io::{BufReader, Cursor, Read, Seek, SeekFrom, Write},
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
    updated: usize,
    skipped: usize,
    errors: Vec<String>,
}

#[cfg(test)]
const OPENPHOTO_XMP_MARKER: &str = "xmp:CreatorTool=\"Open-Photo\"";
const MAX_XMP_SIDECAR_BYTES: u64 = 256 * 1024;

fn render_openphoto_xmp(tags: &[String]) -> String {
    let list = tags
        .iter()
        .map(|tag| format!("<rdf:li>{}</rdf:li>", xml_escape(tag)))
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmp:CreatorTool="Open-Photo">
<dc:subject><rdf:Bag>{list}</rdf:Bag></dc:subject>
</rdf:Description></rdf:RDF></x:xmpmeta>
"#
    )
}

fn openphoto_xmp_tags(xml: &str) -> Option<Vec<String>> {
    xmp::tags(xml)
}

#[derive(Serialize)]
struct ManagedTagEntry {
    path: String,
    tags: Vec<String>,
}

#[tauri::command]
fn read_managed_tags(root: String, paths: Vec<String>) -> Result<Vec<ManagedTagEntry>, String> {
    if paths.len() > 5000 {
        return Err("Read up to 5,000 managed XMP sidecars per request".into());
    }
    let mut entries = Vec::new();
    for path in paths {
        let file = match safe_media_path(&root, &path) {
            Ok(file) => file,
            Err(_) => continue,
        };
        let sidecar = PathBuf::from(format!("{}.xmp", file.display()));
        let Ok(metadata) = fs::symlink_metadata(&sidecar) else {
            continue;
        };
        if !metadata.is_file() || metadata.len() > MAX_XMP_SIDECAR_BYTES {
            continue;
        }
        let Ok(xml) = fs::read_to_string(&sidecar) else {
            continue;
        };
        let Some(tags) = openphoto_xmp_tags(&xml) else {
            continue;
        };
        entries.push(ManagedTagEntry { path, tags });
    }
    Ok(entries)
}

fn replace_managed_sidecar(sidecar: &Path, expected: &str, contents: &str) -> Result<(), String> {
    let file_name = sidecar
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Invalid XMP sidecar filename")?;
    let temp = sidecar.with_file_name(format!("{file_name}.openphoto.tmp"));
    let backup = sidecar.with_file_name(format!("{file_name}.openphoto.bak"));
    if temp.exists() || backup.exists() {
        return Err(
            "Previous Open-Photo XMP recovery file exists; inspect it before retrying".into(),
        );
    }

    {
        let mut handle = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        if let Err(error) = handle
            .write_all(contents.as_bytes())
            .and_then(|_| handle.sync_all())
        {
            drop(handle);
            let _ = fs::remove_file(&temp);
            return Err(error.to_string());
        }
    }

    // Copy into a create-new backup; never remove the normal sidecar path.
    let mut recovery = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&backup)
        .map_err(|error| {
            let _ = fs::remove_file(&temp);
            error.to_string()
        })?;
    recovery
        .write_all(expected.as_bytes())
        .and_then(|_| recovery.sync_all())
        .map_err(|error| error.to_string())?;
    drop(recovery);
    if fs::read_to_string(sidecar).map_err(|error| error.to_string())? != expected {
        let _ = fs::remove_file(&temp);
        let _ = fs::remove_file(&backup);
        return Err("XMP changed outside Open-Photo; refresh before retrying".into());
    }
    if let Err(error) = atomic_replace(&temp, sidecar) {
        let _ = fs::remove_file(&temp);
        return Err(format!(
            "Unable to replace XMP; recovery backup retained: {error}"
        ));
    }
    if let Err(error) = fs::remove_file(&backup) {
        return Err(format!(
            "XMP updated, but the temporary recovery backup could not be removed: {error}"
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn atomic_replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // SAFETY: both paths are live, NUL-terminated UTF-16 buffers; optional arguments are null.
    let result = unsafe {
        windows_sys::Win32::Storage::FileSystem::ReplaceFileW(
            destination.as_ptr(),
            source.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn atomic_replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

static XMP_WRITE_LOCK: Mutex<()> = Mutex::new(());

/// Create new Open-Photo XMP sidecars or merge keywords into sidecars that
/// Open-Photo owns. Unknown/foreign XMP files are never overwritten.
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
        || tags
            .iter()
            .any(|t| t.trim().is_empty() || t.len() > 128 || t.chars().any(char::is_control))
    {
        return Err("Invalid tags; use 1–50 tags of up to 128 characters".into());
    }

    let normalized_tags = tags
        .into_iter()
        .map(|tag| tag.trim().to_string())
        .collect::<Vec<_>>();
    let mut output = BulkTagResult {
        created: 0,
        updated: 0,
        skipped: 0,
        errors: Vec::new(),
    };

    let _write_guard = XMP_WRITE_LOCK.lock().map_err(|error| error.to_string())?;

    for path in paths {
        let file = match safe_media_path(&root, &path) {
            Ok(file) => file,
            Err(error) => {
                output.errors.push(format!("{path}: {error}"));
                continue;
            }
        };
        let sidecar = PathBuf::from(format!("{}.xmp", file.display()));

        if !sidecar.exists() {
            let xml = render_openphoto_xmp(&normalized_tags);
            let result = (|| -> Result<(), String> {
                let mut staged = crate::safe_output::StagedOutput::new(&sidecar)?;
                staged
                    .file()
                    .write_all(xml.as_bytes())
                    .map_err(|error| error.to_string())?;
                staged.commit()?;
                Ok(())
            })();
            match result {
                Ok(()) => output.created += 1,
                Err(error) => output
                    .errors
                    .push(format!("{}: {error}", sidecar.display())),
            }
            continue;
        }

        let metadata = match fs::symlink_metadata(&sidecar) {
            Ok(metadata) => metadata,
            Err(error) => {
                output
                    .errors
                    .push(format!("{}: {error}", sidecar.display()));
                continue;
            }
        };
        if !metadata.is_file() || metadata.len() > MAX_XMP_SIDECAR_BYTES {
            output.skipped += 1;
            continue;
        }
        let xml = match fs::read_to_string(&sidecar) {
            Ok(xml) => xml,
            Err(_) => {
                output.skipped += 1;
                continue;
            }
        };
        let Some(existing) = openphoto_xmp_tags(&xml) else {
            output.skipped += 1;
            continue;
        };

        let mut merged = existing;
        let mut changed = false;
        for tag in &normalized_tags {
            if !merged
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(tag))
            {
                merged.push(tag.clone());
                changed = true;
            }
        }
        if !changed {
            output.skipped += 1;
            continue;
        }
        if merged.len() > 200 {
            output.errors.push(format!(
                "{}: managed sidecar would exceed 200 keywords",
                sidecar.display()
            ));
            continue;
        }
        let replacement = xmp::append_tags(&xml, &normalized_tags)
            .ok_or("Cannot safely preserve this XMP document")?;
        match replace_managed_sidecar(&sidecar, &xml, &replacement) {
            Ok(()) => output.updated += 1,
            Err(error) => output
                .errors
                .push(format!("{}: {error}", sidecar.display())),
        }
    }
    Ok(output)
}

const SUPPORTED: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "gif", "tif", "tiff", "bmp", "heic", "heif", "avif", "dng",
    "cr2", "cr3", "nef", "arw", "raf", "iiq", "mov", "mp4", "m4v", "avi", "mkv", "webm",
];

#[derive(Serialize)]
struct Photo {
    path: String,
    name: String,
    extension: String,
    bytes: u64,
    modified_unix: u64,
    capture_local: Option<String>,
    live_asset_id: Option<String>,
}

#[derive(Serialize)]
pub struct ScanResult {
    photos: Vec<Photo>,
    scanned_files: usize,
    truncated: bool,
    incomplete: bool,
    errors: Vec<String>,
}

fn parse_exif_capture_value(value: &Value) -> Option<String> {
    let Value::Ascii(values) = value else {
        return None;
    };
    let parsed = ExifDateTime::from_ascii(values.first()?).ok()?;
    if parsed.year < 1601
        || !(1..=12).contains(&parsed.month)
        || !(1..=31).contains(&parsed.day)
        || parsed.hour > 23
        || parsed.minute > 59
        || parsed.second > 60
    {
        return None;
    }
    Some(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        parsed.year, parsed.month, parsed.day, parsed.hour, parsed.minute, parsed.second
    ))
}

fn utc_iso_from_unix(unix_seconds: i64) -> Option<String> {
    let days = unix_seconds.div_euclid(86_400);
    let seconds_of_day = unix_seconds.rem_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 }.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    if !(1601..=9999).contains(&year) {
        return None;
    }
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
}

fn quicktime_creation_seconds(path: &Path) -> Option<u64> {
    fn read_box_header(
        file: &mut fs::File,
        position: u64,
        end: u64,
    ) -> Option<(u64, [u8; 4], u64)> {
        if position.checked_add(8)? > end {
            return None;
        }
        file.seek(SeekFrom::Start(position)).ok()?;
        let mut header = [0_u8; 8];
        file.read_exact(&mut header).ok()?;
        let size32 = u32::from_be_bytes(header[..4].try_into().ok()?);
        let kind: [u8; 4] = header[4..8].try_into().ok()?;
        let (size, header_size) = match size32 {
            0 => (end.checked_sub(position)?, 8),
            1 => {
                let mut extended = [0_u8; 8];
                file.read_exact(&mut extended).ok()?;
                (u64::from_be_bytes(extended), 16)
            }
            value => (u64::from(value), 8),
        };
        if size < header_size || position.checked_add(size)? > end {
            return None;
        }
        Some((size, kind, header_size))
    }

    fn read_mvhd_creation(
        file: &mut fs::File,
        payload_start: u64,
        payload_end: u64,
    ) -> Option<u64> {
        if payload_start.checked_add(8)? > payload_end {
            return None;
        }
        file.seek(SeekFrom::Start(payload_start)).ok()?;
        let mut prefix = [0_u8; 12];
        file.read_exact(&mut prefix[..8]).ok()?;
        match prefix[0] {
            0 => Some(u64::from(u32::from_be_bytes(prefix[4..8].try_into().ok()?))),
            1 => {
                if payload_start.checked_add(12)? > payload_end {
                    return None;
                }
                file.read_exact(&mut prefix[8..12]).ok()?;
                Some(u64::from_be_bytes(prefix[4..12].try_into().ok()?))
            }
            _ => None,
        }
    }

    fn find_mvhd(file: &mut fs::File, start: u64, end: u64) -> Option<u64> {
        let mut position = start;
        while position.checked_add(8)? <= end {
            let (size, kind, header_size) = read_box_header(file, position, end)?;
            let payload_start = position.checked_add(header_size)?;
            let box_end = position.checked_add(size)?;
            if &kind == b"mvhd" {
                return read_mvhd_creation(file, payload_start, box_end);
            }
            position = box_end;
        }
        None
    }

    let mut file = fs::File::open(path).ok()?;
    let end = file.metadata().ok()?.len();
    let mut position = 0_u64;
    while position.checked_add(8)? <= end {
        let (size, kind, header_size) = read_box_header(&mut file, position, end)?;
        let payload_start = position.checked_add(header_size)?;
        let box_end = position.checked_add(size)?;
        if &kind == b"moov" {
            return find_mvhd(&mut file, payload_start, box_end);
        }
        position = box_end;
    }
    None
}

fn normalize_metadata_datetime(raw: &str) -> Option<String> {
    let value = raw.trim().trim_matches('"');
    if value.len() < 19 {
        return None;
    }
    let bytes = value.as_bytes();
    let valid_digits = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18]
        .into_iter()
        .all(|index| bytes.get(index).is_some_and(u8::is_ascii_digit));
    if !valid_digits {
        return None;
    }

    let exif_shape = bytes.get(4) == Some(&b':')
        && bytes.get(7) == Some(&b':')
        && bytes.get(10) == Some(&b' ')
        && bytes.get(13) == Some(&b':')
        && bytes.get(16) == Some(&b':');
    let iso_shape = bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && matches!(bytes.get(10).copied(), Some(b'T') | Some(b' '))
        && bytes.get(13) == Some(&b':')
        && bytes.get(16) == Some(&b':');
    if !exif_shape && !iso_shape {
        return None;
    }

    let year: u16 = value[0..4].parse().ok()?;
    let month: u8 = value[5..7].parse().ok()?;
    let day: u8 = value[8..10].parse().ok()?;
    let hour: u8 = value[11..13].parse().ok()?;
    let minute: u8 = value[14..16].parse().ok()?;
    let second: u8 = value[17..19].parse().ok()?;
    if year < 1601
        || !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }

    if exif_shape {
        return Some(format!(
            "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}"
        ));
    }

    let suffix = value.get(19..).unwrap_or("");
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}{suffix}"
    ))
}

fn read_extended_image_capture(path: &Path) -> Option<String> {
    let exif = read_nom_exif(path).ok()?;
    [
        NomExifTag::DateTimeOriginal,
        NomExifTag::CreateDate,
        NomExifTag::ModifyDate,
    ]
    .into_iter()
    .find_map(|tag| {
        exif.get(tag)
            .and_then(|value| normalize_metadata_datetime(&value.to_string()))
    })
}

fn clean_live_asset_id(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim_end_matches('\0').trim();
    if trimmed.is_empty() || trimmed.len() > 128 {
        return None;
    }
    Some(trimmed.to_string())
}

fn apple_makernote_content_id(blob: &[u8]) -> Option<String> {
    const CONTENT_IDENTIFIER_TAG: u16 = 0x0011;
    if blob.len() < 16 || !blob.starts_with(b"Apple iOS\0") {
        return None;
    }

    let big_endian = match blob.get(12..14)? {
        b"MM" => true,
        b"II" => false,
        _ => return None,
    };
    let read_u16 = |offset: usize| -> Option<u16> {
        let bytes: [u8; 2] = blob.get(offset..offset.checked_add(2)?)?.try_into().ok()?;
        Some(if big_endian {
            u16::from_be_bytes(bytes)
        } else {
            u16::from_le_bytes(bytes)
        })
    };
    let read_u32 = |offset: usize| -> Option<u32> {
        let bytes: [u8; 4] = blob.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
        Some(if big_endian {
            u32::from_be_bytes(bytes)
        } else {
            u32::from_le_bytes(bytes)
        })
    };

    let entries = usize::from(read_u16(14)?);
    for index in 0..entries {
        let entry = 16usize.checked_add(index.checked_mul(12)?)?;
        if read_u16(entry)? != CONTENT_IDENTIFIER_TAG {
            continue;
        }
        if read_u16(entry.checked_add(2)?)? != 2 {
            return None;
        }
        let length = usize::try_from(read_u32(entry.checked_add(4)?)?).ok()?;
        let bytes = if length <= 4 {
            blob.get(entry.checked_add(8)?..entry.checked_add(8 + length)?)?
        } else {
            let offset = usize::try_from(read_u32(entry.checked_add(8)?)?).ok()?;
            blob.get(offset..offset.checked_add(length)?)?
        };
        return clean_live_asset_id(bytes);
    }
    None
}

fn still_live_asset_id(path: &Path) -> Option<String> {
    let exif = read_nom_exif(path).ok()?;
    let NomEntryValue::Undefined(bytes) = exif.get(NomExifTag::MakerNote)? else {
        return None;
    };
    apple_makernote_content_id(bytes)
}

fn quicktime_live_asset_id(path: &Path) -> Option<String> {
    const MAX_MOOV_BYTES: u64 = 16 * 1024 * 1024;
    const CONTENT_IDENTIFIER_KEY: &[u8] = b"com.apple.quicktime.content.identifier";

    fn atom_payload<'a>(data: &'a [u8], wanted: &[u8; 4]) -> Option<&'a [u8]> {
        let mut cursor = 0usize;
        while cursor.checked_add(8)? <= data.len() {
            let size32 = u32::from_be_bytes(data.get(cursor..cursor + 4)?.try_into().ok()?);
            let kind: [u8; 4] = data.get(cursor + 4..cursor + 8)?.try_into().ok()?;
            let (size, header) = match size32 {
                0 => (data.len().checked_sub(cursor)?, 8usize),
                1 => {
                    let size = usize::try_from(u64::from_be_bytes(
                        data.get(cursor + 8..cursor + 16)?.try_into().ok()?,
                    ))
                    .ok()?;
                    (size, 16)
                }
                value => (usize::try_from(value).ok()?, 8),
            };
            if size < header || cursor.checked_add(size)? > data.len() {
                return None;
            }
            if &kind == wanted {
                return data.get(cursor + header..cursor + size);
            }
            cursor = cursor.checked_add(size)?;
        }
        None
    }

    fn find_top_level_moov(file: &mut fs::File, end: u64) -> Option<(u64, u64)> {
        let mut position = 0u64;
        while position.checked_add(8)? <= end {
            file.seek(SeekFrom::Start(position)).ok()?;
            let mut header = [0u8; 8];
            file.read_exact(&mut header).ok()?;
            let size32 = u32::from_be_bytes(header[..4].try_into().ok()?);
            let kind: [u8; 4] = header[4..8].try_into().ok()?;
            let (size, header_size) = match size32 {
                0 => (end.checked_sub(position)?, 8u64),
                1 => {
                    let mut extended = [0u8; 8];
                    file.read_exact(&mut extended).ok()?;
                    (u64::from_be_bytes(extended), 16)
                }
                value => (u64::from(value), 8),
            };
            if size < header_size || position.checked_add(size)? > end {
                return None;
            }
            if &kind == b"moov" {
                return Some((position.checked_add(header_size)?, size - header_size));
            }
            position = position.checked_add(size)?;
        }
        None
    }

    fn content_key_index(keys: &[u8]) -> Option<u32> {
        if keys.len() < 8 {
            return None;
        }
        let count = u32::from_be_bytes(keys.get(4..8)?.try_into().ok()?);
        let mut cursor = 8usize;
        for index in 1..=count {
            let size = usize::try_from(u32::from_be_bytes(
                keys.get(cursor..cursor + 4)?.try_into().ok()?,
            ))
            .ok()?;
            if size < 8 || cursor.checked_add(size)? > keys.len() {
                return None;
            }
            if keys.get(cursor + 8..cursor + size)? == CONTENT_IDENTIFIER_KEY {
                return Some(index);
            }
            cursor = cursor.checked_add(size)?;
        }
        None
    }

    fn value_for_index(ilst: &[u8], index: u32) -> Option<String> {
        let wanted = index.to_be_bytes();
        let mut cursor = 0usize;
        while cursor.checked_add(8)? <= ilst.len() {
            let size = usize::try_from(u32::from_be_bytes(
                ilst.get(cursor..cursor + 4)?.try_into().ok()?,
            ))
            .ok()?;
            if size < 8 || cursor.checked_add(size)? > ilst.len() {
                return None;
            }
            if ilst.get(cursor + 4..cursor + 8)? == wanted {
                let item = ilst.get(cursor + 8..cursor + size)?;
                let data = atom_payload(item, b"data")?;
                if data.len() < 8 {
                    return None;
                }
                return clean_live_asset_id(&data[8..]);
            }
            cursor = cursor.checked_add(size)?;
        }
        None
    }

    let mut file = fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    let (moov_start, moov_len) = find_top_level_moov(&mut file, length)?;
    if moov_len > MAX_MOOV_BYTES {
        return None;
    }
    file.seek(SeekFrom::Start(moov_start)).ok()?;
    let mut moov = vec![0u8; usize::try_from(moov_len).ok()?];
    file.read_exact(&mut moov).ok()?;

    let meta = atom_payload(&moov, b"meta").or_else(|| {
        let udta = atom_payload(&moov, b"udta")?;
        atom_payload(udta, b"meta")
    })?;
    let meta_body = if atom_payload(meta, b"keys").is_some() {
        meta
    } else {
        meta.get(4..)?
    };
    let keys = atom_payload(meta_body, b"keys")?;
    let index = content_key_index(keys)?;
    let ilst = atom_payload(meta_body, b"ilst")?;
    value_for_index(ilst, index)
}

fn read_live_asset_id(path: &Path, extension: &str) -> Option<String> {
    if matches!(extension, "mov" | "mp4" | "m4v") {
        return quicktime_live_asset_id(path);
    }
    if matches!(extension, "heic" | "heif" | "jpg" | "jpeg") {
        return still_live_asset_id(path);
    }
    None
}

fn read_capture_local(path: &Path, extension: &str) -> Option<String> {
    if matches!(extension, "mov" | "mp4" | "m4v") {
        const QUICKTIME_TO_UNIX_SECONDS: i64 = 2_082_844_800;
        let quicktime_seconds = quicktime_creation_seconds(path)?;
        let quicktime_seconds = i64::try_from(quicktime_seconds).ok()?;
        return utc_iso_from_unix(quicktime_seconds - QUICKTIME_TO_UNIX_SECONDS);
    }

    if matches!(extension, "heic" | "heif" | "avif" | "cr3" | "raf" | "iiq") {
        return read_extended_image_capture(path);
    }

    if !matches!(
        extension,
        "jpg" | "jpeg" | "tif" | "tiff" | "dng" | "cr2" | "nef" | "arw"
    ) {
        return None;
    }
    let file = fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let exif = ExifReader::new().read_from_container(&mut reader).ok()?;
    [Tag::DateTimeOriginal, Tag::DateTimeDigitized, Tag::DateTime]
        .into_iter()
        .find_map(|tag| {
            exif.get_field(tag, In::PRIMARY)
                .and_then(|field| parse_exif_capture_value(&field.value))
        })
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
    let mut errors = Vec::new();

    'scan: while let Some(dir) = pending.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) => {
                errors.push(format!("Unable to read {}: {error}", dir.display()));
                continue;
            }
        };
        for entry in entries {
            if scanned_files >= MAX_ENTRIES {
                truncated = true;
                break 'scan;
            }
            scanned_files += 1;
            let entry = match entry {
                Ok(e) => e,
                Err(error) => {
                    errors.push(format!("{}: {error}", dir.display()));
                    continue;
                }
            };
            let kind = match entry.file_type() {
                Ok(t) => t,
                Err(error) => {
                    errors.push(format!("{}: {error}", entry.path().display()));
                    continue;
                }
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
                Err(error) => {
                    errors.push(format!("{}: {error}", path.display()));
                    continue;
                }
            };
            let modified_unix = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let capture_local = read_capture_local(&path, &ext);
            let live_asset_id = read_live_asset_id(&path, &ext);
            photos.push(Photo {
                path: path.to_string_lossy().into_owned(),
                name: entry.file_name().to_string_lossy().into_owned(),
                extension: ext,
                bytes: metadata.len(),
                modified_unix,
                capture_local,
                live_asset_id,
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
        incomplete: truncated || !errors.is_empty(),
        errors,
    })
}

fn decode_raw_embedded_preview(path: &Path) -> Result<image::DynamicImage, String> {
    let mut raw = RawFile::open(path).map_err(|e| format!("RAW preview unavailable: {e}"))?;
    let preview = raw
        .largest_preview()
        .ok_or("RAW file does not contain a usable embedded preview")?;
    let orientation = preview.orientation();
    let bytes = raw
        .read_preview(&preview)
        .map_err(|e| format!("Unable to read embedded RAW preview: {e}"))?;
    let image = image::load_from_memory(&bytes)
        .map_err(|_| "Embedded RAW preview is not a supported image encoding".to_string())?;
    Ok(match orientation {
        RawOrientation::NoTransforms => image,
        RawOrientation::FlipHorizontal => image.fliph(),
        RawOrientation::Rotate180 => image.rotate180(),
        RawOrientation::FlipVertical => image.flipv(),
        RawOrientation::Rotate90FlipH => image.rotate90().fliph(),
        RawOrientation::Rotate90 => image.rotate90(),
        RawOrientation::Rotate270FlipH => image.rotate270().fliph(),
        RawOrientation::Rotate270 => image.rotate270(),
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
    let extension = file
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let size = fs::metadata(&file).map_err(|e| e.to_string())?.len();
    let max_source_bytes = if matches!(extension.as_str(), "nef" | "arw") {
        250 * 1024 * 1024
    } else {
        75 * 1024 * 1024
    };
    if size > max_source_bytes {
        return Err(format!(
            "Preview unavailable for this file larger than {} MB",
            max_source_bytes / (1024 * 1024)
        ));
    }
    let modified = fs::metadata(&file)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let key = format!(
        "oriented-v2:{}:{size}:{modified}:{}",
        file.display(),
        max_edge
    );
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
    let decoded = if matches!(extension.as_str(), "nef" | "arw") {
        decode_raw_embedded_preview(&file)?
    } else {
        image_decode::decode(&file)?
    };
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

#[derive(Serialize)]
struct UpdaterChannelStatus {
    enabled: bool,
    policy: &'static str,
}

#[tauri::command]
fn updater_channel_status() -> UpdaterChannelStatus {
    UpdaterChannelStatus {
        enabled: option_env!("OPENPHOTO_UPDATER_PUBLIC_KEY")
            .is_some_and(|key| !key.trim().is_empty()),
        policy: "major-only",
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppUpdateMetadata {
    rid: tauri::ResourceId,
    current_version: String,
    version: String,
    date: Option<String>,
    body: Option<String>,
    raw_json: serde_json::Value,
}

#[tauri::command]
async fn check_app_update(webview: tauri::Webview) -> Result<Option<AppUpdateMetadata>, String> {
    use std::time::Duration;
    use tauri::Manager;
    use tauri_plugin_updater::UpdaterExt;
    if !updater_channel_status().enabled {
        return Ok(None);
    }
    let updater = webview
        .updater_builder()
        .timeout(Duration::from_secs(15))
        .configure_client(|client| {
            client
                .connect_timeout(Duration::from_secs(15))
                .read_timeout(Duration::from_secs(30))
        })
        .build()
        .map_err(|e| e.to_string())?;
    let Some(mut update) = updater.check().await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    // Large signed installers may need many minutes; stop stalled reads instead
    // of imposing an overall deadline on a download that is still progressing.
    update.timeout = None;
    let metadata = AppUpdateMetadata {
        rid: 0,
        current_version: update.current_version.clone(),
        version: update.version.clone(),
        date: update
            .raw_json
            .get("pub_date")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        body: update.body.clone(),
        raw_json: update.raw_json.clone(),
    };
    // Update implements Tauri's Resource; the stock SDK owns and closes it.
    Ok(Some(AppUpdateMetadata {
        rid: webview.resources_table().add(update),
        ..metadata
    }))
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
    let updater = tauri_plugin_updater::Builder::new()
        .pubkey(option_env!("OPENPHOTO_UPDATER_PUBLIC_KEY").unwrap_or(""))
        .default_version_comparator(|current, release| {
            release.version.major > current.major
                && release.version.minor == 0
                && release.version.patch == 0
                && release.version.pre.is_empty()
        })
        .build();

    tauri::Builder::default()
        .plugin(updater)
        .setup(|app| {
            use tauri::Manager;
            let directory = if cfg!(debug_assertions) {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/ai-models")
            } else {
                app.path().resource_dir()?.join("bundled-ai")
            };
            ai_models::configure_bundled_directory(directory);
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .manage(std::sync::Mutex::new(watch::WatchState::default()))
        .manage(media_jobs::ImageJobs::default())
        .manage(video_probe::InspectionJobs::default())
        .manage(video_preview::PreviewJobs::default())
        .manage(video_poster::PosterJobs::default())
        .manage(video_export::ExportJobs::default())
        .manage(search::SearchState::default())
        .invoke_handler(tauri::generate_handler![
            search::search_workspace_evidence,
            search::cancel_workspace_search,
            ai_embeddings::ai_embedding_status,
            ai_embeddings::ai_find_similar_by_photo,
            ai_embeddings::ai_index_semantic_batch,
            ai_embeddings::ai_index_semantic_image,
            ai_embeddings::ai_search_semantic_text,
            ai_faces::ai_detect_faces,
            ai_faces::ai_face_index_status,
            ai_faces::ai_index_faces_batch,
            ai_faces::ai_people_groups,
            ai_inference::ai_run_semantic_image,
            ai_preprocess::ai_prepare_image,
            ai_runtime::ai_runtime_status,
            ai_text::ai_run_semantic_text,
            ai_validation::ai_validation_report,
            ai_models::ai_model_status,
            ai_models::ai_remove_model,
            ai_ocr::ai_index_ocr_batch,
            ai_ocr::ai_ocr_photo,
            ai_ocr::ai_search_ocr,
            ai_models::ai_import_model,
            scan_library,
            preview_image,
            read_iphone_media,
            media_stream::video_source,
            video_poster::video_poster,
            video_probe::inspect_video,
            video_engine::video_engine_status,
            video_preview::start_video_preview,
            video_preview::video_preview_status,
            video_preview::cancel_video_preview,
            video_preview::start_video_project_preview,
            video_sequence::create_video_sequence,
            video_workspace::workspace_video_projects,
            video_workspace::edit_workspace_video_project,
            video_workspace::save_workspace_video_project,
            video_workspace::workspace_video_history,
            video_workspace::start_workspace_video_preview,
            video_workspace::start_workspace_video_export,
            video_sequence::add_sequence_videos,
            video_sequence::save_video_sequence,
            video_sequence::load_video_sequence,
            video_sequence::recent_video_sequences,
            video_sequence::import_video_sequence,
            video_sequence::write_video_sequence_copy,
            video_sequence::remove_sequence_video,
            video_sequence::relink_sequence_video,
            video_project::create_video_project,
            video_project::save_video_project,
            video_project::load_video_project,
            video_project::video_project_history,
            video_project::recover_video_project,
            video_project::recent_video_projects,
            video_project::import_video_project,
            video_project::write_video_project_copy,
            video_project::relink_video_project,
            video_export::start_video_export,
            video_export::video_export_status,
            video_export::cancel_video_export,
            video_export::dismiss_video_export,
            video_export::open_video_export,
            user_data::load_user_data,
            user_data::save_user_data,
            user_data::export_user_data,
            ai_faces::ai_name_person,
            bulk_create_tags,
            read_managed_tags,
            edit_export::export_edited_copy,
            edit_export::preview_edited_photo,
            open_video,
            updater_channel_status,
            check_app_update,
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
    fn managed_merge_preserves_extra_properties_and_rejects_stale_replacement() {
        let root = temp_root();
        let image = root.join("sample.jpg");
        fs::write(&image, b"original").unwrap();
        let sidecar = root.join("sample.jpg.xmp");
        let xml = render_openphoto_xmp(&["Family".into()]).replace(
            "</rdf:Description>",
            "<dc:description>Keep this caption</dc:description></rdf:Description>",
        );
        fs::write(&sidecar, &xml).unwrap();
        let result = bulk_create_tags(
            root.to_string_lossy().into_owned(),
            vec![image.to_string_lossy().into_owned()],
            vec!["Trips".into()],
        )
        .unwrap();
        assert_eq!(result.updated, 1);
        let updated = fs::read_to_string(&sidecar).unwrap();
        assert!(updated.contains("<dc:description>Keep this caption</dc:description>"));
        assert_eq!(
            openphoto_xmp_tags(&updated).unwrap(),
            vec!["Family", "Trips"]
        );
        assert!(replace_managed_sidecar(&sidecar, &xml, "wrong replacement").is_err());
        assert_eq!(fs::read_to_string(&sidecar).unwrap(), updated);
        assert_eq!(fs::read(&image).unwrap(), b"original");
        fs::remove_dir_all(root).unwrap();
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
    fn foreign_sidecars_are_never_overwritten_or_originals_modified() {
        let root = temp_root();
        let image = root.join("sample.jpg");
        fs::write(&image, b"original").unwrap();
        let sidecar = root.join("sample.jpg.xmp");
        fs::write(&sidecar, b"<x:xmpmeta>foreign metadata</x:xmpmeta>").unwrap();
        let result = bulk_create_tags(
            root.to_string_lossy().into_owned(),
            vec![image.to_string_lossy().into_owned()],
            vec!["Family".into()],
        )
        .unwrap();
        assert_eq!(result.created, 0);
        assert_eq!(result.updated, 0);
        assert_eq!(result.skipped, 1);
        assert_eq!(
            fs::read_to_string(&sidecar).unwrap(),
            "<x:xmpmeta>foreign metadata</x:xmpmeta>"
        );
        assert_eq!(fs::read(&image).unwrap(), b"original");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn parses_only_openphoto_managed_xmp_keywords() {
        let xml = render_openphoto_xmp(&["Family".into(), "Trips/Florida".into()]);
        assert_eq!(
            openphoto_xmp_tags(&xml),
            Some(vec!["Family".into(), "Trips/Florida".into()])
        );
        let foreign = xml.replace(OPENPHOTO_XMP_MARKER, "xmp:CreatorTool=\"Other App\"");
        assert!(openphoto_xmp_tags(&foreign).is_none());
    }

    #[test]
    fn reads_only_managed_xmp_tags_for_requested_media() {
        let root = temp_root();
        let managed = root.join("managed.jpg");
        let foreign = root.join("foreign.jpg");
        fs::write(&managed, b"managed").unwrap();
        fs::write(&foreign, b"foreign").unwrap();
        fs::write(
            root.join("managed.jpg.xmp"),
            render_openphoto_xmp(&["People/Family/Marc".into(), "Florida".into()]),
        )
        .unwrap();
        fs::write(
            root.join("foreign.jpg.xmp"),
            "<x:xmpmeta>foreign</x:xmpmeta>",
        )
        .unwrap();

        let entries = read_managed_tags(
            root.to_string_lossy().into_owned(),
            vec![
                managed.to_string_lossy().into_owned(),
                foreign.to_string_lossy().into_owned(),
            ],
        )
        .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, managed.to_string_lossy());
        assert_eq!(entries[0].tags, vec!["People/Family/Marc", "Florida"]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn managed_xmp_update_preserves_source_media_and_merges_tags() {
        let root = temp_root();
        let image = root.join("sample.jpg");
        fs::write(&image, b"original").unwrap();
        let root_arg = root.to_string_lossy().into_owned();
        let paths = vec![image.to_string_lossy().into_owned()];
        let first =
            bulk_create_tags(root_arg.clone(), paths.clone(), vec!["Family".into()]).unwrap();
        assert_eq!(first.created, 1);
        let second = bulk_create_tags(
            root_arg,
            paths,
            vec!["family".into(), "Trips/Florida".into()],
        )
        .unwrap();
        assert_eq!(second.updated, 1);
        let xml = fs::read_to_string(root.join("sample.jpg.xmp")).unwrap();
        let tags = openphoto_xmp_tags(&xml).unwrap();
        assert_eq!(tags, vec!["Family", "Trips/Florida"]);
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

    #[test]
    fn parses_exif_capture_datetime_without_inventing_timezone() {
        let value = Value::Ascii(vec![b"2024:07:04 18:23:59".to_vec()]);
        assert_eq!(
            parse_exif_capture_value(&value).as_deref(),
            Some("2024-07-04T18:23:59")
        );
    }

    #[test]
    fn rejects_invalid_exif_capture_datetime() {
        let value = Value::Ascii(vec![b"2024:13:40 25:99:99".to_vec()]);
        assert!(parse_exif_capture_value(&value).is_none());
    }

    #[test]
    fn normalizes_extended_metadata_datetimes_without_inventing_timezone() {
        assert_eq!(
            normalize_metadata_datetime("2024:07:04 18:23:59").as_deref(),
            Some("2024-07-04T18:23:59")
        );
        assert_eq!(
            normalize_metadata_datetime("2024-07-04 18:23:59").as_deref(),
            Some("2024-07-04T18:23:59")
        );
        assert_eq!(
            normalize_metadata_datetime("2024-07-04T18:23:59+00:00").as_deref(),
            Some("2024-07-04T18:23:59+00:00")
        );
        assert!(normalize_metadata_datetime("2024:13:40 25:99:99").is_none());
    }

    #[test]
    fn parses_apple_live_photo_makernote_identifier() {
        let identifier = "F0652AEA-5229-4BF7-A366-B4C79E90CA1C";
        let mut blob = Vec::new();
        blob.extend_from_slice(b"Apple iOS\0");
        blob.extend_from_slice(&[0, 1]);
        blob.extend_from_slice(b"MM");
        blob.extend_from_slice(&1u16.to_be_bytes());
        let value_offset = 16 + 12 + 4;
        blob.extend_from_slice(&0x0011u16.to_be_bytes());
        blob.extend_from_slice(&2u16.to_be_bytes());
        blob.extend_from_slice(&((identifier.len() + 1) as u32).to_be_bytes());
        blob.extend_from_slice(&(value_offset as u32).to_be_bytes());
        blob.extend_from_slice(&0u32.to_be_bytes());
        blob.extend_from_slice(identifier.as_bytes());
        blob.push(0);
        assert_eq!(
            apple_makernote_content_id(&blob).as_deref(),
            Some(identifier)
        );
    }

    #[test]
    fn rejects_non_apple_or_truncated_live_photo_makernotes() {
        assert!(apple_makernote_content_id(b"Nikon\0metadata").is_none());
        let mut apple = b"Apple iOS\0".to_vec();
        apple.extend_from_slice(&[0, 1, b'M', b'M']);
        assert!(apple_makernote_content_id(&apple).is_none());
    }

    #[test]
    fn converts_quicktime_epoch_to_utc_iso() {
        assert_eq!(
            utc_iso_from_unix(0).as_deref(),
            Some("1970-01-01T00:00:00Z")
        );
        assert_eq!(
            utc_iso_from_unix(1_720_116_239).as_deref(),
            Some("2024-07-04T18:03:59Z")
        );
    }

    #[test]
    fn parses_quicktime_mvhd_creation_time() {
        let root = temp_root();
        let video = root.join("sample.mov");
        let quicktime_seconds = 2_082_844_800_u32 + 3_600;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&28_u32.to_be_bytes());
        bytes.extend_from_slice(b"moov");
        bytes.extend_from_slice(&20_u32.to_be_bytes());
        bytes.extend_from_slice(b"mvhd");
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        bytes.extend_from_slice(&quicktime_seconds.to_be_bytes());
        bytes.extend_from_slice(&0_u32.to_be_bytes());
        fs::write(&video, bytes).unwrap();
        assert_eq!(
            read_capture_local(&video, "mov").as_deref(),
            Some("1970-01-01T01:00:00Z")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
