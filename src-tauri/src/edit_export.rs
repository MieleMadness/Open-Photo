//! Phase 4 non-destructive photo edits.
//! The source media is always read-only. Edits are written only to a new, explicitly chosen file.
use image::{DynamicImage, ImageFormat};
use serde::Serialize;
use std::{fs, path::PathBuf};

const MAX_EDIT_SOURCE_BYTES: u64 = 250 * 1024 * 1024;

#[derive(Serialize)]
pub struct EditedExportResult {
    path: String,
    width: u32,
    height: u32,
    bytes: u64,
}

fn validate_rotation(rotation: i32) -> Result<i32, String> {
    match rotation.rem_euclid(360) {
        0 => Ok(0),
        90 => Ok(90),
        180 => Ok(180),
        270 => Ok(270),
        _ => Err("Rotation must be 0, 90, 180, or 270 degrees".into()),
    }
}

fn apply_edits(
    mut image: DynamicImage,
    rotation: i32,
    flip_horizontal: bool,
    flip_vertical: bool,
    brightness: i32,
    contrast: f32,
) -> Result<DynamicImage, String> {
    let rotation = validate_rotation(rotation)?;
    image = match rotation {
        90 => image.rotate90(),
        180 => image.rotate180(),
        270 => image.rotate270(),
        _ => image,
    };
    if flip_horizontal {
        image = image.fliph();
    }
    if flip_vertical {
        image = image.flipv();
    }
    if !(-100..=100).contains(&brightness) {
        return Err("Brightness must be between -100 and 100".into());
    }
    if !(-100.0..=100.0).contains(&contrast) || !contrast.is_finite() {
        return Err("Contrast must be between -100 and 100".into());
    }
    if brightness != 0 {
        image = image.brighten(brightness);
    }
    if contrast.abs() > f32::EPSILON {
        image = image.adjust_contrast(contrast);
    }
    Ok(image)
}

fn export_format(destination: &std::path::Path) -> Result<ImageFormat, String> {
    let extension = destination
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "jpg" | "jpeg" => Ok(ImageFormat::Jpeg),
        "png" => Ok(ImageFormat::Png),
        "webp" => Ok(ImageFormat::WebP),
        "bmp" => Ok(ImageFormat::Bmp),
        "tif" | "tiff" => Ok(ImageFormat::Tiff),
        _ => Err("Export as JPG, PNG, WebP, BMP, or TIFF".into()),
    }
}

#[tauri::command]
pub fn export_edited_copy(
    root: String,
    path: String,
    destination: String,
    rotation: i32,
    flip_horizontal: bool,
    flip_vertical: bool,
    brightness: i32,
    contrast: f32,
) -> Result<EditedExportResult, String> {
    let source = crate::safe_media_path(&root, &path)?;
    let source_extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(
        source_extension.as_str(),
        "jpg" | "jpeg" | "png" | "webp" | "gif" | "tif" | "tiff" | "bmp" | "avif"
    ) {
        return Err("This media type is not supported by the Phase 4 image editor".into());
    }
    let source_bytes = fs::metadata(&source).map_err(|e| e.to_string())?.len();
    if source_bytes > MAX_EDIT_SOURCE_BYTES {
        return Err("Editing is limited to source images up to 250 MB".into());
    }

    let destination = PathBuf::from(destination);
    if destination.as_os_str().is_empty() {
        return Err("Choose an export destination".into());
    }
    let parent = destination
        .parent()
        .ok_or("Choose a destination inside an existing folder")?;
    let parent = fs::canonicalize(parent).map_err(|_| "Export folder not found".to_string())?;
    let file_name = destination
        .file_name()
        .ok_or("Choose a valid export filename")?;
    let destination = parent.join(file_name);
    if destination == source {
        return Err("Edited exports must use a new file; the original is read-only".into());
    }
    if destination.exists() {
        return Err("That export file already exists; choose a new filename".into());
    }
    let format = export_format(&destination)?;

    let image = image::ImageReader::open(&source)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| format!("Cannot decode this image for editing: {e}"))?;
    let edited = apply_edits(
        image,
        rotation,
        flip_horizontal,
        flip_vertical,
        brightness,
        contrast,
    )?;
    let width = edited.width();
    let height = edited.height();

    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .map_err(|e| format!("Cannot create edited export: {e}"))?;
    if let Err(error) = edited.write_to(&mut output, format) {
        drop(output);
        let _ = fs::remove_file(&destination);
        return Err(format!("Cannot encode edited export: {error}"));
    }
    if let Err(error) = output.sync_all() {
        drop(output);
        let _ = fs::remove_file(&destination);
        return Err(format!("Cannot finish edited export: {error}"));
    }
    drop(output);
    let bytes = fs::metadata(&destination).map_err(|e| e.to_string())?.len();

    Ok(EditedExportResult {
        path: destination.to_string_lossy().into_owned(),
        width,
        height,
        bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_quarter_turn_rotations() {
        assert_eq!(validate_rotation(90).unwrap(), 90);
        assert_eq!(validate_rotation(-90).unwrap(), 270);
        assert!(validate_rotation(45).is_err());
    }

    #[test]
    fn edit_limits_are_enforced() {
        let image = DynamicImage::new_rgb8(4, 3);
        assert!(apply_edits(image.clone(), 0, false, false, 101, 0.0).is_err());
        assert!(apply_edits(image, 0, false, false, 0, 101.0).is_err());
    }

    #[test]
    fn quarter_turn_changes_dimensions() {
        let image = DynamicImage::new_rgb8(4, 3);
        let edited = apply_edits(image, 90, false, false, 0, 0.0).unwrap();
        assert_eq!((edited.width(), edited.height()), (3, 4));
    }
}
