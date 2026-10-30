//! Local People indexing built on the pinned YuNet detector and SFace embedder.
//! Original media is read-only; face boxes and embeddings live in local application data.
use image::{imageops::FilterType, GenericImageView, Rgb, RgbImage};
use ort::session::Session;
use rusqlite::{params, Connection};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    time::UNIX_EPOCH,
};

const DETECTOR_SIZE: usize = 320;
const EMBED_SIZE: usize = 112;
const FACE_SCORE_THRESHOLD: f32 = 0.65;
const NMS_THRESHOLD: f32 = 0.30;
const PEOPLE_COSINE_THRESHOLD: f32 = 0.42;
const MAX_BATCH: usize = 512;
const MAX_GROUP_FACES: usize = 10_000;

const ARCFACE_TEMPLATE: [[f64; 2]; 5] = [
    [38.2946, 51.6963],
    [73.5318, 51.5014],
    [56.0252, 71.7366],
    [41.5493, 92.3655],
    [70.7299, 92.2041],
];

#[derive(Clone, Serialize)]
pub struct FaceDetection {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) score: f32,
    pub(crate) landmarks: [f32; 10],
}

#[derive(Serialize)]
pub struct FaceIndexResult {
    requested: usize,
    indexed_images: usize,
    skipped_images: usize,
    failed_images: usize,
    faces: usize,
    errors: Vec<String>,
}

#[derive(Clone, Serialize)]
pub struct PersonFace {
    root: String,
    path: String,
    face_index: usize,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    score: f32,
}

#[derive(Serialize)]
pub struct PersonGroup {
    id: String,
    name: String,
    legacy_id: String,
    members: Vec<PersonFace>,
}

#[derive(Serialize)]
pub struct FaceIndexStatus {
    indexed_images: usize,
    faces: usize,
}

struct FaceRecord {
    detection: FaceDetection,
    embedding: Vec<f32>,
}

pub(crate) fn database() -> Result<Connection, String> {
    let directory = dirs::data_local_dir()
        .ok_or("Windows local data directory unavailable")?
        .join("Open-Photo");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let connection =
        Connection::open(directory.join("ai-faces.sqlite")).map_err(|e| e.to_string())?;
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS face_index_state (
                root TEXT NOT NULL,
                path TEXT NOT NULL,
                modified_unix INTEGER NOT NULL,
                detector_key TEXT NOT NULL,
                embedder_key TEXT NOT NULL,
                faces INTEGER NOT NULL,
                PRIMARY KEY(root, path)
            );
            CREATE TABLE IF NOT EXISTS face_embeddings (
                root TEXT NOT NULL,
                path TEXT NOT NULL,
                face_index INTEGER NOT NULL,
                modified_unix INTEGER NOT NULL,
                detector_key TEXT NOT NULL,
                embedder_key TEXT NOT NULL,
                x REAL NOT NULL,
                y REAL NOT NULL,
                width REAL NOT NULL,
                height REAL NOT NULL,
                score REAL NOT NULL,
                landmarks BLOB NOT NULL,
                dimensions INTEGER NOT NULL,
                vector BLOB NOT NULL,
                PRIMARY KEY(root, path, face_index)
            );
            CREATE INDEX IF NOT EXISTS idx_face_embedder
              ON face_embeddings(embedder_key);
            CREATE INDEX IF NOT EXISTS idx_face_state_models
              ON face_index_state(detector_key, embedder_key);",
        )
        .map_err(|e| e.to_string())?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS people (
        id TEXT PRIMARY KEY, name TEXT NOT NULL DEFAULT '');
        CREATE TABLE IF NOT EXISTS person_faces (
        face_key TEXT PRIMARY KEY, person_id TEXT NOT NULL REFERENCES people(id));",
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

fn encode_f32(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn decode_f32(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if !bytes.len().is_multiple_of(4) {
        return Err("Stored face embedding is corrupt".into());
    }
    Ok(bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect())
}

fn normalize(values: &mut [f32]) -> Result<(), String> {
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm == 0.0 || !norm.is_finite() {
        return Err("Face embedding has no usable magnitude".into());
    }
    for value in values {
        *value /= norm;
    }
    Ok(())
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.len() != right.len() || left.is_empty() {
        return -1.0;
    }
    left.iter().zip(right).map(|(a, b)| a * b).sum::<f32>()
}

fn face_sessions(device: Option<&str>) -> Result<(Session, Session, String, String), String> {
    let (detector_key, embedder_key) = crate::ai_models::validate_supported_face_bundle()?;
    let environment = crate::ai_runtime::environment()?;

    let detector_path = crate::ai_models::installed_model_path("faces")?;
    let detector_builder = Session::builder(&environment).map_err(|e| e.to_string())?;
    let mut detector_builder = crate::ai_runtime::configure_session(detector_builder, device)?;
    let detector = detector_builder
        .commit_from_file(detector_path)
        .map_err(|e| format!("Cannot load YuNet face detector: {e}"))?;

    if detector.inputs().len() != 1 || detector.outputs().len() != 12 {
        return Err("Supported YuNet detector must expose one input and 12 output heads".into());
    }
    let required = [
        "cls_8", "cls_16", "cls_32", "obj_8", "obj_16", "obj_32", "bbox_8", "bbox_16", "bbox_32",
        "kps_8", "kps_16", "kps_32",
    ];
    for name in required {
        if !detector
            .outputs()
            .iter()
            .any(|output| output.name() == name)
        {
            return Err(format!("Supported YuNet output {name} is missing"));
        }
    }

    let embedder_path = crate::ai_models::installed_model_path("face_embedder")?;
    let embedder_builder = Session::builder(&environment).map_err(|e| e.to_string())?;
    let mut embedder_builder = crate::ai_runtime::configure_session(embedder_builder, device)?;
    let embedder = embedder_builder
        .commit_from_file(embedder_path)
        .map_err(|e| format!("Cannot load SFace embedder: {e}"))?;
    if embedder.inputs().len() != 1 || embedder.outputs().len() != 1 {
        return Err("Supported SFace embedder must expose one input and one output".into());
    }

    Ok((detector, embedder, detector_key, embedder_key))
}

fn detector_tensor(image: &image::DynamicImage) -> Result<(Vec<f32>, f32), String> {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Err("Cannot detect faces in an empty image".into());
    }

    let scale = (DETECTOR_SIZE as f32 / width as f32).min(DETECTOR_SIZE as f32 / height as f32);
    let new_width = ((width as f32 * scale).round() as u32).clamp(1, DETECTOR_SIZE as u32);
    let new_height = ((height as f32 * scale).round() as u32).clamp(1, DETECTOR_SIZE as u32);
    let resized = image
        .resize_exact(new_width, new_height, FilterType::Triangle)
        .to_rgb8();

    let plane = DETECTOR_SIZE * DETECTOR_SIZE;
    let mut tensor = vec![0.0_f32; plane * 3];
    for y in 0..new_height as usize {
        for x in 0..new_width as usize {
            let pixel = resized.get_pixel(x as u32, y as u32);
            let index = y * DETECTOR_SIZE + x;
            tensor[index] = f32::from(pixel[2]);
            tensor[index + plane] = f32::from(pixel[1]);
            tensor[index + plane * 2] = f32::from(pixel[0]);
        }
    }

    Ok((tensor, scale))
}

fn clamp(value: f32, min: f32, max: f32) -> f32 {
    value.max(min).min(max)
}

fn iou(left: &FaceDetection, right: &FaceDetection) -> f32 {
    let left_x2 = left.x + left.width;
    let left_y2 = left.y + left.height;
    let right_x2 = right.x + right.width;
    let right_y2 = right.y + right.height;
    let x1 = left.x.max(right.x);
    let y1 = left.y.max(right.y);
    let x2 = left_x2.min(right_x2);
    let y2 = left_y2.min(right_y2);
    let intersection = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    let union = left.width * left.height + right.width * right.height - intersection;
    if union <= 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn non_max_suppression(mut faces: Vec<FaceDetection>) -> Vec<FaceDetection> {
    faces.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut kept: Vec<FaceDetection> = Vec::with_capacity(faces.len().min(100));
    for face in faces {
        if kept
            .iter()
            .any(|existing| iou(existing, &face) >= NMS_THRESHOLD)
        {
            continue;
        }
        kept.push(face);
        if kept.len() >= 100 {
            break;
        }
    }
    kept
}

fn detect_faces(
    session: &mut Session,
    image: &image::DynamicImage,
) -> Result<Vec<FaceDetection>, String> {
    let (original_width, original_height) = image.dimensions();
    let (values, scale) = detector_tensor(image)?;
    let tensor = ort::value::Tensor::from_array((
        [1usize, 3, DETECTOR_SIZE, DETECTOR_SIZE],
        values.into_boxed_slice(),
    ))
    .map_err(|e| e.to_string())?;
    let output_names: Vec<String> = session
        .outputs()
        .iter()
        .map(|output| output.name().to_string())
        .collect();
    let outputs = session
        .run(ort::inputs![tensor])
        .map_err(|e| format!("YuNet face detection failed: {e}"))?;
    let read_output = |name: &str| -> Result<Vec<f32>, String> {
        let index = output_names
            .iter()
            .position(|candidate| candidate == name)
            .ok_or_else(|| format!("YuNet output {name} is missing"))?;
        let (_, data) = outputs[index]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("YuNet output {name} is not float32: {e}"))?;
        Ok(data.to_vec())
    };

    let mut detections = Vec::new();
    for stride in [8usize, 16, 32] {
        let cls = read_output(&format!("cls_{stride}"))?;
        let obj = read_output(&format!("obj_{stride}"))?;
        let bbox = read_output(&format!("bbox_{stride}"))?;
        let kps = read_output(&format!("kps_{stride}"))?;
        let feature_width = DETECTOR_SIZE / stride;
        let feature_height = DETECTOR_SIZE / stride;
        let cells = feature_width * feature_height;
        if cls.len() != cells
            || obj.len() != cells
            || bbox.len() != cells * 4
            || kps.len() != cells * 10
        {
            return Err(format!(
                "Unexpected YuNet stride {stride} output dimensions"
            ));
        }

        let stride_f = stride as f32;
        for index in 0..cells {
            let score = (clamp(cls[index], 0.0, 1.0) * clamp(obj[index], 0.0, 1.0)).sqrt();
            if score < FACE_SCORE_THRESHOLD {
                continue;
            }
            let col = (index % feature_width) as f32;
            let row = (index / feature_width) as f32;
            let cx = (col + bbox[index * 4]) * stride_f;
            let cy = (row + bbox[index * 4 + 1]) * stride_f;
            let width = bbox[index * 4 + 2].exp() * stride_f;
            let height = bbox[index * 4 + 3].exp() * stride_f;

            let x1 = clamp((cx - width / 2.0) / scale, 0.0, original_width as f32);
            let y1 = clamp((cy - height / 2.0) / scale, 0.0, original_height as f32);
            let x2 = clamp((cx + width / 2.0) / scale, 0.0, original_width as f32);
            let y2 = clamp((cy + height / 2.0) / scale, 0.0, original_height as f32);
            if x2 <= x1 || y2 <= y1 {
                continue;
            }

            let mut landmarks = [0.0_f32; 10];
            for point in 0..5 {
                landmarks[point * 2] = (kps[index * 10 + point * 2] + col) * stride_f / scale;
                landmarks[point * 2 + 1] =
                    (kps[index * 10 + point * 2 + 1] + row) * stride_f / scale;
            }
            detections.push(FaceDetection {
                x: x1,
                y: y1,
                width: x2 - x1,
                height: y2 - y1,
                score,
                landmarks,
            });
        }
    }

    Ok(non_max_suppression(detections))
}

type Affine = [f64; 6];

fn similarity_transform(source: &[[f64; 2]; 5]) -> Result<Affine, String> {
    let mut source_mean = [0.0_f64; 2];
    let mut target_mean = [0.0_f64; 2];
    for index in 0..5 {
        source_mean[0] += source[index][0];
        source_mean[1] += source[index][1];
        target_mean[0] += ARCFACE_TEMPLATE[index][0];
        target_mean[1] += ARCFACE_TEMPLATE[index][1];
    }
    source_mean[0] /= 5.0;
    source_mean[1] /= 5.0;
    target_mean[0] /= 5.0;
    target_mean[1] /= 5.0;

    let mut a = 0.0_f64;
    let mut b = 0.0_f64;
    let mut variance = 0.0_f64;
    for index in 0..5 {
        let sx = source[index][0] - source_mean[0];
        let sy = source[index][1] - source_mean[1];
        let dx = ARCFACE_TEMPLATE[index][0] - target_mean[0];
        let dy = ARCFACE_TEMPLATE[index][1] - target_mean[1];
        a += sx * dx + sy * dy;
        b += sx * dy - sy * dx;
        variance += sx * sx + sy * sy;
    }
    if variance <= f64::EPSILON {
        return Err("Face landmarks are coincident".into());
    }
    a /= variance;
    b /= variance;
    Ok([
        a,
        -b,
        target_mean[0] - (a * source_mean[0] - b * source_mean[1]),
        b,
        a,
        target_mean[1] - (b * source_mean[0] + a * source_mean[1]),
    ])
}

fn invert_affine(matrix: Affine) -> Result<Affine, String> {
    let determinant = matrix[0] * matrix[4] - matrix[1] * matrix[3];
    if determinant.abs() <= f64::EPSILON {
        return Err("Face alignment transform is singular".into());
    }
    let inverse = 1.0 / determinant;
    let a = matrix[4] * inverse;
    let b = -matrix[1] * inverse;
    let d = -matrix[3] * inverse;
    let e = matrix[0] * inverse;
    Ok([
        a,
        b,
        -(a * matrix[2] + b * matrix[5]),
        d,
        e,
        -(d * matrix[2] + e * matrix[5]),
    ])
}

fn bilinear_sample(image: &RgbImage, x: f64, y: f64) -> Rgb<u8> {
    let width = image.width() as i32;
    let height = image.height() as i32;
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let fx = x - f64::from(x0);
    let fy = y - f64::from(y0);
    let samples = [
        (x0, y0, (1.0 - fx) * (1.0 - fy)),
        (x0 + 1, y0, fx * (1.0 - fy)),
        (x0, y0 + 1, (1.0 - fx) * fy),
        (x0 + 1, y0 + 1, fx * fy),
    ];
    let mut channels = [0.0_f64; 3];
    for (px, py, weight) in samples {
        if px < 0 || py < 0 || px >= width || py >= height || weight <= 0.0 {
            continue;
        }
        let pixel = image.get_pixel(px as u32, py as u32);
        for channel in 0..3 {
            channels[channel] += f64::from(pixel[channel]) * weight;
        }
    }
    Rgb([
        channels[0].round().clamp(0.0, 255.0) as u8,
        channels[1].round().clamp(0.0, 255.0) as u8,
        channels[2].round().clamp(0.0, 255.0) as u8,
    ])
}

fn aligned_crop(image: &RgbImage, detection: &FaceDetection) -> Result<RgbImage, String> {
    let mut source = [[0.0_f64; 2]; 5];
    for (index, point) in source.iter_mut().enumerate() {
        point[0] = f64::from(detection.landmarks[index * 2]);
        point[1] = f64::from(detection.landmarks[index * 2 + 1]);
    }
    let inverse = invert_affine(similarity_transform(&source)?)?;
    let mut output = RgbImage::new(EMBED_SIZE as u32, EMBED_SIZE as u32);
    for y in 0..EMBED_SIZE {
        for x in 0..EMBED_SIZE {
            let source_x = inverse[0] * x as f64 + inverse[1] * y as f64 + inverse[2];
            let source_y = inverse[3] * x as f64 + inverse[4] * y as f64 + inverse[5];
            output.put_pixel(
                x as u32,
                y as u32,
                bilinear_sample(image, source_x, source_y),
            );
        }
    }
    Ok(output)
}

fn face_embedding(session: &mut Session, crop: &RgbImage) -> Result<Vec<f32>, String> {
    let plane = EMBED_SIZE * EMBED_SIZE;
    let mut values = vec![0.0_f32; plane * 3];
    for y in 0..EMBED_SIZE {
        for x in 0..EMBED_SIZE {
            let pixel = crop.get_pixel(x as u32, y as u32);
            let index = y * EMBED_SIZE + x;
            values[index] = f32::from(pixel[0]);
            values[index + plane] = f32::from(pixel[1]);
            values[index + plane * 2] = f32::from(pixel[2]);
        }
    }
    let tensor = ort::value::Tensor::from_array((
        [1usize, 3, EMBED_SIZE, EMBED_SIZE],
        values.into_boxed_slice(),
    ))
    .map_err(|e| e.to_string())?;
    let outputs = session
        .run(ort::inputs![tensor])
        .map_err(|e| format!("SFace embedding inference failed: {e}"))?;
    if outputs.len() != 1 {
        return Err("SFace returned an unexpected output count".into());
    }
    let (_, data) = outputs[0]
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("SFace output is not float32: {e}"))?;
    if data.len() != 128 {
        return Err(format!(
            "Supported SFace model must return 128 values, got {}",
            data.len()
        ));
    }
    let mut embedding = data.to_vec();
    normalize(&mut embedding)?;
    Ok(embedding)
}

fn process_image(
    detector: &mut Session,
    embedder: &mut Session,
    root: &str,
    path: &str,
) -> Result<(i64, Vec<FaceRecord>), String> {
    let source = crate::safe_media_path(root, path)?;
    let modified = modified_unix(&source)?;
    let image = crate::image_decode::decode(&source)?;
    let rgb = image.to_rgb8();
    let detections = detect_faces(detector, &image)?;
    let mut faces = Vec::with_capacity(detections.len());
    for detection in detections {
        let crop = aligned_crop(&rgb, &detection)?;
        let embedding = face_embedding(embedder, &crop)?;
        faces.push(FaceRecord {
            detection,
            embedding,
        });
    }
    Ok((modified, faces))
}

fn index_state_is_current(
    db: &Connection,
    root: &str,
    path: &str,
    modified: i64,
    detector_key: &str,
    embedder_key: &str,
) -> Result<bool, String> {
    db.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM face_index_state
            WHERE root=?1 AND path=?2 AND modified_unix=?3
              AND detector_key=?4 AND embedder_key=?5
        )",
        params![root, path, modified, detector_key, embedder_key],
        |row| row.get::<_, i64>(0),
    )
    .map(|exists| exists != 0)
    .map_err(|e| e.to_string())
}

fn store_faces(
    db: &mut Connection,
    root: &str,
    path: &str,
    modified: i64,
    detector_key: &str,
    embedder_key: &str,
    faces: &[FaceRecord],
) -> Result<(), String> {
    let transaction = db.transaction().map_err(|e| e.to_string())?;
    transaction
        .execute(
            "DELETE FROM face_embeddings WHERE root=?1 AND path=?2",
            params![root, path],
        )
        .map_err(|e| e.to_string())?;
    for (face_index, face) in faces.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO face_embeddings(
                    root,path,face_index,modified_unix,detector_key,embedder_key,
                    x,y,width,height,score,landmarks,dimensions,vector
                 ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
                params![
                    root,
                    path,
                    face_index as i64,
                    modified,
                    detector_key,
                    embedder_key,
                    face.detection.x,
                    face.detection.y,
                    face.detection.width,
                    face.detection.height,
                    face.detection.score,
                    encode_f32(&face.detection.landmarks),
                    face.embedding.len() as i64,
                    encode_f32(&face.embedding)
                ],
            )
            .map_err(|e| e.to_string())?;
    }
    transaction
        .execute(
            "INSERT INTO face_index_state(root,path,modified_unix,detector_key,embedder_key,faces)
             VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(root,path) DO UPDATE SET
               modified_unix=excluded.modified_unix,
               detector_key=excluded.detector_key,
               embedder_key=excluded.embedder_key,
               faces=excluded.faces",
            params![
                root,
                path,
                modified,
                detector_key,
                embedder_key,
                faces.len() as i64
            ],
        )
        .map_err(|e| e.to_string())?;
    transaction.commit().map_err(|e| e.to_string())
}

fn index_batch_blocking(
    root: String,
    paths: Vec<String>,
    device: Option<String>,
) -> Result<FaceIndexResult, String> {
    let (mut detector, mut embedder, detector_key, embedder_key) =
        face_sessions(device.as_deref())?;
    let mut db = database()?;
    let mut result = FaceIndexResult {
        requested: paths.len(),
        indexed_images: 0,
        skipped_images: 0,
        failed_images: 0,
        faces: 0,
        errors: Vec::new(),
    };

    for path in paths {
        let source = match crate::safe_media_path(&root, &path) {
            Ok(source) => source,
            Err(error) => {
                result.failed_images += 1;
                result.errors.push(format!("{path}: {error}"));
                continue;
            }
        };
        let modified = match modified_unix(&source) {
            Ok(value) => value,
            Err(error) => {
                result.failed_images += 1;
                result.errors.push(format!("{path}: {error}"));
                continue;
            }
        };
        match index_state_is_current(&db, &root, &path, modified, &detector_key, &embedder_key) {
            Ok(true) => {
                result.skipped_images += 1;
                continue;
            }
            Ok(false) => {}
            Err(error) => {
                result.failed_images += 1;
                result.errors.push(format!("{path}: {error}"));
                continue;
            }
        }

        match process_image(&mut detector, &mut embedder, &root, &path) {
            Ok((processed_modified, faces)) => {
                if let Err(error) = store_faces(
                    &mut db,
                    &root,
                    &path,
                    processed_modified,
                    &detector_key,
                    &embedder_key,
                    &faces,
                ) {
                    result.failed_images += 1;
                    result.errors.push(format!("{path}: {error}"));
                } else {
                    result.indexed_images += 1;
                    result.faces += faces.len();
                }
            }
            Err(error) => {
                result.failed_images += 1;
                result.errors.push(format!("{path}: {error}"));
            }
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn ai_index_faces_batch(
    root: String,
    paths: Vec<String>,
    device: Option<String>,
) -> Result<FaceIndexResult, String> {
    if paths.is_empty() || paths.len() > MAX_BATCH {
        return Err(format!(
            "Select between 1 and {MAX_BATCH} photos per People indexing batch"
        ));
    }
    tauri::async_runtime::spawn_blocking(move || index_batch_blocking(root, paths, device))
        .await
        .map_err(|e| format!("People indexing worker failed: {e}"))?
}

#[tauri::command]
pub fn ai_detect_faces(
    root: String,
    path: String,
    device: Option<String>,
) -> Result<Vec<FaceDetection>, String> {
    let (mut detector, _, _, _) = face_sessions(device.as_deref())?;
    let source = crate::safe_media_path(&root, &path)?;
    let image = crate::image_decode::decode(&source)?;
    detect_faces(&mut detector, &image)
}

#[derive(Clone)]
struct GroupCandidate {
    face: PersonFace,
}

#[cfg(test)]
pub(crate) fn bundled_inference_acceptance() -> Result<(), String> {
    let (mut detector, mut embedder, _, _) = face_sessions(Some("cpu"))?;
    let image =
        image::DynamicImage::ImageRgb8(RgbImage::from_pixel(320, 320, image::Rgb([120, 140, 160])));
    let _ = detect_faces(&mut detector, &image)?;
    let crop = RgbImage::from_pixel(
        EMBED_SIZE as u32,
        EMBED_SIZE as u32,
        image::Rgb([120, 140, 160]),
    );
    let values = face_embedding(&mut embedder, &crop)?;
    if values.len() != 128 || values.iter().any(|v| !v.is_finite()) {
        return Err("Bundled SFace inference failed".into());
    }
    Ok(())
}

struct WorkingGroup {
    id: String,
    name: String,
    members: Vec<GroupCandidate>,
    centroid: Vec<f32>,
}

fn stable_group_id(group: &WorkingGroup) -> String {
    let first = &group.members[0].face;
    let mut hash = Sha256::new();
    hash.update(first.root.as_bytes());
    hash.update([0]);
    hash.update(first.path.as_bytes());
    hash.update([0]);
    hash.update(first.face_index.to_le_bytes());
    let digest = format!("{:x}", hash.finalize());
    format!("person-{}", &digest[..12])
}

pub(crate) fn face_key(model: &str, vector: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(model.as_bytes());
    hash.update([0]);
    hash.update(vector);
    format!("{:x}", hash.finalize())
}

fn person_for_face(db: &Connection, key: &str) -> Result<Option<(String, String)>, String> {
    use rusqlite::OptionalExtension;
    db.query_row("SELECT p.id,p.name FROM people p JOIN person_faces f ON p.id=f.person_id WHERE f.face_key=?1",[key],|row| Ok((row.get(0)?,row.get(1)?)))
        .optional().map_err(|e|e.to_string())
}

#[tauri::command]
pub fn ai_name_person(id: String, name: String) -> Result<(), String> {
    if name.len() > 200 || name.chars().any(char::is_control) {
        return Err("Use a person name of up to 200 characters".into());
    }
    let db = database()?;
    if db
        .execute(
            "UPDATE people SET name=?2 WHERE id=?1",
            params![id, name.trim()],
        )
        .map_err(|e| e.to_string())?
        != 1
    {
        return Err("Person no longer available; refresh People".into());
    }
    Ok(())
}

#[tauri::command]
pub fn ai_people_groups(
    roots: Vec<String>,
    legacy_names: Option<HashMap<String, String>>,
) -> Result<Vec<PersonGroup>, String> {
    if roots.is_empty() {
        return Ok(Vec::new());
    }
    let (_, embedder_key) = crate::ai_models::validate_supported_face_bundle()?;
    let mut db = database()?;
    // Filter roots before applying the cap, so other workspaces cannot crowd this one out.
    let candidates = {
        let mut statement = db
            .prepare(
                "SELECT root,path,face_index,x,y,width,height,score,vector FROM face_embeddings
            WHERE embedder_key=?1 AND root IN (SELECT value FROM json_each(?2))
            ORDER BY root,path,face_index LIMIT ?3",
            )
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map(
                params![
                    embedder_key,
                    serde_json::to_string(&roots).map_err(|e| e.to_string())?,
                    MAX_GROUP_FACES as i64 + 1
                ],
                |row| {
                    Ok((
                        PersonFace {
                            root: row.get(0)?,
                            path: row.get(1)?,
                            face_index: row.get::<_, i64>(2)? as usize,
                            x: row.get(3)?,
                            y: row.get(4)?,
                            width: row.get(5)?,
                            height: row.get(6)?,
                            score: row.get(7)?,
                        },
                        row.get::<_, Vec<u8>>(8)?,
                    ))
                },
            )
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };
    if candidates.len() > MAX_GROUP_FACES {
        return Err("People grouping currently supports up to 10,000 faces per workspace; choose fewer folders".into());
    }
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let mut groups: Vec<WorkingGroup> = Vec::new();
    for (face, blob) in candidates {
        if crate::safe_media_path(&face.root, &face.path).is_err() {
            continue;
        }
        let vector = decode_f32(&blob)?;
        if vector.len() != 128 {
            continue;
        }
        let key = face_key(&embedder_key, &blob);
        let prior = person_for_face(&tx, &key)?;
        let target = if let Some((id, _)) = &prior {
            groups.iter().position(|group| &group.id == id)
        } else {
            groups
                .iter()
                .enumerate()
                .filter(|(_, group)| group.name.is_empty())
                .map(|(index, group)| (index, cosine_similarity(&vector, &group.centroid)))
                .filter(|(_, score)| *score >= PEOPLE_COSINE_THRESHOLD)
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(index, _)| index)
        };
        let index = if let Some(index) = target {
            index
        } else {
            let (id, name) = if let Some(prior) = prior {
                prior
            } else {
                let id: String = tx
                    .query_row(
                        "SELECT 'person-' || lower(hex(randomblob(16)))",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(|e| e.to_string())?;
                tx.execute("INSERT INTO people(id) VALUES(?1)", [&id])
                    .map_err(|e| e.to_string())?;
                (id, String::new())
            };
            groups.push(WorkingGroup {
                id,
                name,
                members: Vec::new(),
                centroid: vector.clone(),
            });
            groups.len() - 1
        };
        let group = &mut groups[index];
        tx.execute(
            "INSERT OR IGNORE INTO person_faces(face_key,person_id) VALUES(?1,?2)",
            params![key, group.id],
        )
        .map_err(|e| e.to_string())?;
        let count = group.members.len() as f32;
        for (centroid, value) in group.centroid.iter_mut().zip(&vector) {
            *centroid = (*centroid * count + *value) / (count + 1.0);
        }
        normalize(&mut group.centroid)?;
        group.members.push(GroupCandidate { face });
    }
    let legacy = legacy_names.unwrap_or_default();
    for group in &mut groups {
        if group.name.is_empty() {
            if let Some(name) = legacy
                .get(&stable_group_id(group))
                .filter(|name| name.len() <= 200)
            {
                group.name = name.clone();
                tx.execute(
                    "UPDATE people SET name=?2 WHERE id=?1",
                    params![group.id, group.name],
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    groups.sort_by_key(|group| std::cmp::Reverse(group.members.len()));
    Ok(groups
        .into_iter()
        .map(|group| PersonGroup {
            legacy_id: stable_group_id(&group),
            id: group.id,
            name: group.name,
            members: group
                .members
                .into_iter()
                .map(|member| member.face)
                .collect(),
        })
        .collect())
}

#[tauri::command]
pub fn ai_face_index_status(roots: Vec<String>) -> Result<FaceIndexStatus, String> {
    if roots.is_empty() {
        return Ok(FaceIndexStatus {
            indexed_images: 0,
            faces: 0,
        });
    }
    let (detector_key, embedder_key) = crate::ai_models::validate_supported_face_bundle()?;
    let allowed: HashSet<String> = roots.into_iter().collect();
    let db = database()?;
    let mut statement = db
        .prepare(
            "SELECT root,faces FROM face_index_state
             WHERE detector_key=?1 AND embedder_key=?2",
        )
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map(params![detector_key, embedder_key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|e| e.to_string())?;
    let mut indexed_images = 0usize;
    let mut faces = 0usize;
    for row in rows {
        let (root, face_count) = row.map_err(|e| e.to_string())?;
        if allowed.contains(&root) {
            indexed_images += 1;
            faces += face_count.max(0) as usize;
        }
    }
    Ok(FaceIndexStatus {
        indexed_images,
        faces,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewed_person_identity_is_independent_of_filename_or_first_member() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE people(id TEXT PRIMARY KEY,name TEXT);
          CREATE TABLE person_faces(face_key TEXT PRIMARY KEY,person_id TEXT);
          INSERT INTO people VALUES('immutable-id','Alex');",
        )
        .unwrap();
        let key = face_key("model", &[1, 2, 3]);
        db.execute("INSERT INTO person_faces VALUES(?1,'immutable-id')", [&key])
            .unwrap();
        // File/root names and group order are deliberately absent from the identity key.
        assert_eq!(
            person_for_face(&db, &face_key("model", &[1, 2, 3])).unwrap(),
            Some(("immutable-id".into(), "Alex".into()))
        );
        assert!(person_for_face(&db, &face_key("other-model", &[1, 2, 3]))
            .unwrap()
            .is_none());
    }

    #[test]
    fn cosine_similarity_prefers_identical_face_vectors() {
        assert!(
            cosine_similarity(&[1.0, 0.0], &[1.0, 0.0])
                > cosine_similarity(&[1.0, 0.0], &[0.0, 1.0])
        );
    }

    #[test]
    fn non_max_suppression_removes_overlapping_lower_score_face() {
        let landmarks = [0.0; 10];
        let result = non_max_suppression(vec![
            FaceDetection {
                x: 10.0,
                y: 10.0,
                width: 100.0,
                height: 100.0,
                score: 0.9,
                landmarks,
            },
            FaceDetection {
                x: 12.0,
                y: 12.0,
                width: 98.0,
                height: 98.0,
                score: 0.8,
                landmarks,
            },
        ]);
        assert_eq!(result.len(), 1);
        assert!((result[0].score - 0.9).abs() < 0.001);
    }

    #[test]
    fn arcface_template_fits_itself() {
        let transform = similarity_transform(&ARCFACE_TEMPLATE).unwrap();
        assert!((transform[0] - 1.0).abs() < 0.0001);
        assert!(transform[1].abs() < 0.0001);
        assert!(transform[2].abs() < 0.0001);
    }
}
