//! English-first local OCR using pinned PP-OCR mobile detector + recognizer models.
//! OCR text is stored in local application data; original media remains read-only.
use image::{imageops::FilterType, GenericImageView, RgbImage};
use ort::session::Session;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::{collections::VecDeque, path::Path, time::UNIX_EPOCH};

const DETECTOR_SIZE: usize = 640;
const DETECTOR_THRESHOLD: f32 = 0.30;
const MIN_COMPONENT_PIXELS: usize = 18;
const MAX_REGIONS: usize = 64;
const RECOGNIZER_HEIGHT: usize = 48;
const RECOGNIZER_WIDTH: usize = 320;
const MAX_BATCH: usize = 256;
const OCR_CHARS: &str =
    "0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_\u{60}abcdefghijklmnopqrstuvwxyz{|}~!\"#$%&'()*+,-./ ";

#[derive(Clone, Serialize)]
pub struct OcrRegion {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    text: String,
    confidence: f32,
}

#[derive(Serialize)]
pub struct OcrPhotoResult {
    root: String,
    path: String,
    text: String,
    regions: Vec<OcrRegion>,
}

#[derive(Serialize)]
pub struct OcrBatchResult {
    requested: usize,
    indexed: usize,
    skipped: usize,
    failed: usize,
    regions: usize,
    errors: Vec<String>,
}

#[derive(Serialize)]
pub struct OcrSearchMatch {
    root: String,
    path: String,
    text: String,
}

fn database() -> Result<Connection, String> {
    let directory = dirs::data_local_dir()
        .ok_or("Windows local data directory unavailable")?
        .join("Open-Photo");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let connection =
        Connection::open(directory.join("ai-ocr.sqlite")).map_err(|e| e.to_string())?;
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS ocr_index (
                root TEXT NOT NULL,
                path TEXT NOT NULL,
                modified_unix INTEGER NOT NULL,
                detector_key TEXT NOT NULL,
                recognizer_key TEXT NOT NULL,
                text TEXT NOT NULL,
                regions_json TEXT NOT NULL,
                PRIMARY KEY(root,path)
            );
            CREATE INDEX IF NOT EXISTS idx_ocr_models
              ON ocr_index(detector_key,recognizer_key);",
        )
        .map_err(|e| e.to_string())?;
    Ok(connection)
}

fn modified_unix(path: &Path) -> Result<i64, String> {
    path.metadata()
        .map_err(|e| e.to_string())?
        .modified()
        .map_err(|e| e.to_string())?
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())
        .map(|duration| duration.as_secs() as i64)
}

fn sessions(device: Option<&str>) -> Result<(Session, Session, String, String), String> {
    let (detector_key, recognizer_key) = crate::ai_models::validate_supported_ocr_bundle()?;
    let environment = ort::init()
        .with_name("Open-Photo-OCR")
        .with_telemetry(false)
        .build()
        .map_err(|e| e.to_string())?;

    let detector_path = crate::ai_models::installed_model_path("ocr")?;
    let detector_builder = Session::builder(&environment).map_err(|e| e.to_string())?;
    let mut detector_builder = crate::ai_runtime::configure_session(detector_builder, device)?;
    let detector = detector_builder
        .commit_from_file(detector_path)
        .map_err(|e| format!("Cannot load PP-OCR detector: {e}"))?;
    if detector.inputs().len() != 1 || detector.outputs().len() != 1 {
        return Err("Supported OCR detector must expose one input and one output".into());
    }

    let recognizer_path = crate::ai_models::installed_model_path("ocr_recognizer")?;
    let recognizer_builder = Session::builder(&environment).map_err(|e| e.to_string())?;
    let mut recognizer_builder = crate::ai_runtime::configure_session(recognizer_builder, device)?;
    let recognizer = recognizer_builder
        .commit_from_file(recognizer_path)
        .map_err(|e| format!("Cannot load PP-OCR recognizer: {e}"))?;
    if recognizer.inputs().len() != 1 || recognizer.outputs().len() != 1 {
        return Err("Supported OCR recognizer must expose one input and one output".into());
    }

    Ok((detector, recognizer, detector_key, recognizer_key))
}

fn detector_tensor(image: &image::DynamicImage) -> Result<Vec<f32>, String> {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Err("Cannot OCR an empty image".into());
    }
    let rgb = image
        .resize_exact(
            DETECTOR_SIZE as u32,
            DETECTOR_SIZE as u32,
            FilterType::Triangle,
        )
        .to_rgb8();
    let mean = [0.485_f32, 0.456, 0.406];
    let std = [0.229_f32, 0.224, 0.225];
    let plane = DETECTOR_SIZE * DETECTOR_SIZE;
    let mut values = vec![0.0_f32; plane * 3];
    for y in 0..DETECTOR_SIZE {
        for x in 0..DETECTOR_SIZE {
            let pixel = rgb.get_pixel(x as u32, y as u32);
            let index = y * DETECTOR_SIZE + x;
            for channel in 0..3 {
                values[index + plane * channel] =
                    (f32::from(pixel[channel]) / 255.0 - mean[channel]) / std[channel];
            }
        }
    }
    Ok(values)
}

#[derive(Clone, Copy)]
struct HeatBox {
    x1: usize,
    y1: usize,
    x2: usize,
    y2: usize,
    pixels: usize,
}

fn detect_regions(
    session: &mut Session,
    image: &image::DynamicImage,
) -> Result<Vec<(u32, u32, u32, u32)>, String> {
    let values = detector_tensor(image)?;
    let tensor = ort::value::Tensor::from_array((
        [1usize, 3, DETECTOR_SIZE, DETECTOR_SIZE],
        values.into_boxed_slice(),
    ))
    .map_err(|e| e.to_string())?;
    let outputs = session
        .run(ort::inputs![tensor])
        .map_err(|e| format!("OCR detection failed: {e}"))?;
    if outputs.len() != 1 {
        return Err("OCR detector returned an unexpected output count".into());
    }
    let (shape, data) = outputs[0]
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("OCR detector output is not float32: {e}"))?;
    let dims: Vec<usize> = shape.iter().map(|value| *value as usize).collect();
    if dims.len() < 2 {
        return Err("OCR detector output shape is invalid".into());
    }
    let map_height = *dims.get(dims.len() - 2).ok_or("Missing OCR map height")?;
    let map_width = *dims.last().ok_or("Missing OCR map width")?;
    if map_width == 0 || map_height == 0 || data.len() < map_width * map_height {
        return Err("OCR detector output map is empty".into());
    }

    let map = &data[data.len() - map_width * map_height..];
    let mut visited = vec![false; map_width * map_height];
    let mut boxes = Vec::new();
    for y in 0..map_height {
        for x in 0..map_width {
            let start = y * map_width + x;
            if visited[start] || map[start] < DETECTOR_THRESHOLD {
                continue;
            }
            visited[start] = true;
            let mut queue = VecDeque::from([(x, y)]);
            let mut component = HeatBox {
                x1: x,
                y1: y,
                x2: x,
                y2: y,
                pixels: 0,
            };
            while let Some((cx, cy)) = queue.pop_front() {
                component.x1 = component.x1.min(cx);
                component.y1 = component.y1.min(cy);
                component.x2 = component.x2.max(cx);
                component.y2 = component.y2.max(cy);
                component.pixels += 1;
                for (nx, ny) in [
                    (cx.wrapping_sub(1), cy),
                    (cx + 1, cy),
                    (cx, cy.wrapping_sub(1)),
                    (cx, cy + 1),
                ] {
                    if nx >= map_width || ny >= map_height {
                        continue;
                    }
                    let index = ny * map_width + nx;
                    if !visited[index] && map[index] >= DETECTOR_THRESHOLD {
                        visited[index] = true;
                        queue.push_back((nx, ny));
                    }
                }
            }
            if component.pixels >= MIN_COMPONENT_PIXELS {
                boxes.push(component);
            }
        }
    }

    boxes.sort_by_key(|region| (region.y1, region.x1));
    boxes.truncate(MAX_REGIONS);
    let (source_width, source_height) = image.dimensions();
    let scale_x = source_width as f32 / map_width as f32;
    let scale_y = source_height as f32 / map_height as f32;
    Ok(boxes
        .into_iter()
        .map(|region| {
            let padding_x = ((region.x2 - region.x1 + 1) as f32 * 0.08).max(2.0);
            let padding_y = ((region.y2 - region.y1 + 1) as f32 * 0.18).max(2.0);
            let x1 = ((region.x1 as f32 - padding_x).max(0.0) * scale_x) as u32;
            let y1 = ((region.y1 as f32 - padding_y).max(0.0) * scale_y) as u32;
            let x2 = (((region.x2 + 1) as f32 + padding_x).min(map_width as f32) * scale_x).ceil()
                as u32;
            let y2 = (((region.y2 + 1) as f32 + padding_y).min(map_height as f32) * scale_y).ceil()
                as u32;
            (
                x1.min(source_width.saturating_sub(1)),
                y1.min(source_height.saturating_sub(1)),
                x2.min(source_width).max(x1 + 1),
                y2.min(source_height).max(y1 + 1),
            )
        })
        .collect())
}

fn recognizer_tensor(crop: &RgbImage) -> Vec<f32> {
    let width = crop.width().max(1);
    let height = crop.height().max(1);
    let scale = RECOGNIZER_HEIGHT as f32 / height as f32;
    let resized_width = ((width as f32 * scale).round() as usize).clamp(1, RECOGNIZER_WIDTH);
    let resized = image::imageops::resize(
        crop,
        resized_width as u32,
        RECOGNIZER_HEIGHT as u32,
        FilterType::Triangle,
    );
    let plane = RECOGNIZER_HEIGHT * RECOGNIZER_WIDTH;
    let mut values = vec![0.0_f32; plane * 3];
    for y in 0..RECOGNIZER_HEIGHT {
        for x in 0..resized_width {
            let pixel = resized.get_pixel(x as u32, y as u32);
            let index = y * RECOGNIZER_WIDTH + x;
            for channel in 0..3 {
                values[index + plane * channel] = f32::from(pixel[channel]) / 127.5 - 1.0;
            }
        }
    }
    values
}

fn recognize_region(session: &mut Session, crop: &RgbImage) -> Result<(String, f32), String> {
    let values = recognizer_tensor(crop);
    let tensor = ort::value::Tensor::from_array((
        [1usize, 3, RECOGNIZER_HEIGHT, RECOGNIZER_WIDTH],
        values.into_boxed_slice(),
    ))
    .map_err(|e| e.to_string())?;
    let outputs = session
        .run(ort::inputs![tensor])
        .map_err(|e| format!("OCR recognition failed: {e}"))?;
    if outputs.len() != 1 {
        return Err("OCR recognizer returned an unexpected output count".into());
    }
    let (shape, data) = outputs[0]
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("OCR recognizer output is not float32: {e}"))?;
    let dims: Vec<usize> = shape.iter().map(|value| *value as usize).collect();
    if dims.len() < 2 {
        return Err("OCR recognizer output shape is invalid".into());
    }
    let classes = *dims.last().ok_or("Missing OCR class dimension")?;
    if classes < 2 || data.len() % classes != 0 {
        return Err("OCR recognizer output classes are invalid".into());
    }
    let steps = data.len() / classes;
    let characters: Vec<char> = OCR_CHARS.chars().collect();
    let mut previous = usize::MAX;
    let mut text = String::new();
    let mut confidence_sum = 0.0_f32;
    let mut confidence_count = 0usize;
    for step in 0..steps {
        let row = &data[step * classes..(step + 1) * classes];
        let (class, score) = row
            .iter()
            .copied()
            .enumerate()
            .max_by(|left, right| left.1.total_cmp(&right.1))
            .ok_or("OCR recognizer returned an empty timestep")?;
        if class != 0 && class != previous {
            if let Some(character) = characters.get(class - 1) {
                text.push(*character);
                confidence_sum += score;
                confidence_count += 1;
            }
        }
        previous = class;
    }
    let confidence = if confidence_count == 0 {
        0.0
    } else {
        confidence_sum / confidence_count as f32
    };
    Ok((text.trim().to_string(), confidence))
}

fn process_photo(
    detector: &mut Session,
    recognizer: &mut Session,
    root: &str,
    path: &str,
) -> Result<(i64, Vec<OcrRegion>), String> {
    let source = crate::safe_media_path(root, path)?;
    let modified = modified_unix(&source)?;
    let image = crate::image_decode::decode(&source)?;
    let rgb = image.to_rgb8();
    let boxes = detect_regions(detector, &image)?;
    let mut regions = Vec::new();
    for (x1, y1, x2, y2) in boxes {
        if x2 <= x1 || y2 <= y1 {
            continue;
        }
        let crop = image::imageops::crop_imm(&rgb, x1, y1, x2 - x1, y2 - y1).to_image();
        let (text, confidence) = recognize_region(recognizer, &crop)?;
        if text.is_empty() {
            continue;
        }
        regions.push(OcrRegion {
            x: x1 as f32,
            y: y1 as f32,
            width: (x2 - x1) as f32,
            height: (y2 - y1) as f32,
            text,
            confidence,
        });
    }
    Ok((modified, regions))
}

fn index_is_current(
    db: &Connection,
    root: &str,
    path: &str,
    modified: i64,
    detector_key: &str,
    recognizer_key: &str,
) -> Result<bool, String> {
    db.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM ocr_index
            WHERE root=?1 AND path=?2 AND modified_unix=?3
              AND detector_key=?4 AND recognizer_key=?5
        )",
        params![root, path, modified, detector_key, recognizer_key],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value != 0)
    .map_err(|e| e.to_string())
}

fn store_result(
    db: &Connection,
    root: &str,
    path: &str,
    modified: i64,
    detector_key: &str,
    recognizer_key: &str,
    regions: &[OcrRegion],
) -> Result<(), String> {
    let text = regions
        .iter()
        .map(|region| region.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let regions_json = serde_json::to_string(regions).map_err(|e| e.to_string())?;
    db.execute(
        "INSERT INTO ocr_index(root,path,modified_unix,detector_key,recognizer_key,text,regions_json)
         VALUES(?1,?2,?3,?4,?5,?6,?7)
         ON CONFLICT(root,path) DO UPDATE SET
           modified_unix=excluded.modified_unix,
           detector_key=excluded.detector_key,
           recognizer_key=excluded.recognizer_key,
           text=excluded.text,
           regions_json=excluded.regions_json",
        params![
            root,
            path,
            modified,
            detector_key,
            recognizer_key,
            text,
            regions_json
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn index_batch_blocking(
    root: String,
    paths: Vec<String>,
    device: Option<String>,
) -> Result<OcrBatchResult, String> {
    let (mut detector, mut recognizer, detector_key, recognizer_key) = sessions(device.as_deref())?;
    let db = database()?;
    let mut result = OcrBatchResult {
        requested: paths.len(),
        indexed: 0,
        skipped: 0,
        failed: 0,
        regions: 0,
        errors: Vec::new(),
    };
    for path in paths {
        let source = match crate::safe_media_path(&root, &path) {
            Ok(source) => source,
            Err(error) => {
                result.failed += 1;
                result.errors.push(format!("{path}: {error}"));
                continue;
            }
        };
        let modified = match modified_unix(&source) {
            Ok(value) => value,
            Err(error) => {
                result.failed += 1;
                result.errors.push(format!("{path}: {error}"));
                continue;
            }
        };
        match index_is_current(&db, &root, &path, modified, &detector_key, &recognizer_key) {
            Ok(true) => {
                result.skipped += 1;
                continue;
            }
            Ok(false) => {}
            Err(error) => {
                result.failed += 1;
                result.errors.push(format!("{path}: {error}"));
                continue;
            }
        }
        match process_photo(&mut detector, &mut recognizer, &root, &path) {
            Ok((processed_modified, regions)) => {
                if let Err(error) = store_result(
                    &db,
                    &root,
                    &path,
                    processed_modified,
                    &detector_key,
                    &recognizer_key,
                    &regions,
                ) {
                    result.failed += 1;
                    result.errors.push(format!("{path}: {error}"));
                } else {
                    result.indexed += 1;
                    result.regions += regions.len();
                }
            }
            Err(error) => {
                result.failed += 1;
                result.errors.push(format!("{path}: {error}"));
            }
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn ai_index_ocr_batch(
    root: String,
    paths: Vec<String>,
    device: Option<String>,
) -> Result<OcrBatchResult, String> {
    if paths.is_empty() || paths.len() > MAX_BATCH {
        return Err(format!(
            "Select between 1 and {MAX_BATCH} photos per OCR indexing batch"
        ));
    }
    tauri::async_runtime::spawn_blocking(move || index_batch_blocking(root, paths, device))
        .await
        .map_err(|e| format!("OCR indexing worker failed: {e}"))?
}

#[tauri::command]
pub fn ai_ocr_photo(
    root: String,
    path: String,
    device: Option<String>,
) -> Result<OcrPhotoResult, String> {
    let (mut detector, mut recognizer, _, _) = sessions(device.as_deref())?;
    let (_, regions) = process_photo(&mut detector, &mut recognizer, &root, &path)?;
    let text = regions
        .iter()
        .map(|region| region.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(OcrPhotoResult {
        root,
        path,
        text,
        regions,
    })
}

#[tauri::command]
pub fn ai_search_ocr(
    roots: Vec<String>,
    query: String,
    limit: usize,
) -> Result<Vec<OcrSearchMatch>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("Enter text to search for in indexed photos".into());
    }
    let (_, recognizer_key) = crate::ai_models::validate_supported_ocr_bundle()?;
    let db = database()?;
    let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
    let mut statement = db
        .prepare(
            "SELECT root,path,text FROM ocr_index
             WHERE recognizer_key=?1 AND text LIKE ?2 ESCAPE '\\'
             ORDER BY modified_unix DESC
             LIMIT ?3",
        )
        .map_err(|e| e.to_string())?;
    let allowed: std::collections::HashSet<String> = roots.into_iter().collect();
    let rows = statement
        .query_map(
            params![recognizer_key, pattern, limit.clamp(1, 100) as i64],
            |row| {
                Ok(OcrSearchMatch {
                    root: row.get(0)?,
                    path: row.get(1)?,
                    text: row.get(2)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;
    let mut matches = Vec::new();
    for row in rows {
        let item = row.map_err(|e| e.to_string())?;
        if allowed.contains(&item.root) {
            matches.push(item);
        }
    }
    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_dictionary_contract_is_stable() {
        assert_eq!(OCR_CHARS.chars().count(), 95);
        assert_eq!(OCR_CHARS.chars().next(), Some('0'));
        assert_eq!(OCR_CHARS.chars().last(), Some(' '));
    }

    #[test]
    fn recognizer_tensor_has_fixed_shape() {
        let image = RgbImage::from_pixel(100, 40, image::Rgb([255, 255, 255]));
        assert_eq!(
            recognizer_tensor(&image).len(),
            3 * RECOGNIZER_HEIGHT * RECOGNIZER_WIDTH
        );
    }
}
