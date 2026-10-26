//! Phase 4 non-destructive photo edits.
//! The source media is always read-only. Edits are written only to a new, explicitly chosen file.
use base64::Engine;
use image::{imageops::FilterType, DynamicImage, GenericImageView, ImageFormat};
use serde::{Deserialize, Serialize};
use std::{fs, io::Cursor, path::PathBuf};

const MAX_EDIT_SOURCE_BYTES: u64 = 250 * 1024 * 1024;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditOptions {
    rotation: i32,
    flip_horizontal: bool,
    flip_vertical: bool,
    brightness: i32,
    contrast: f32,
    #[serde(default)]
    crop_left: f32,
    #[serde(default)]
    crop_top: f32,
    #[serde(default)]
    crop_right: f32,
    #[serde(default)]
    crop_bottom: f32,
    #[serde(default)]
    resize_width: Option<u32>,
    #[serde(default)]
    resize_height: Option<u32>,
}

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

fn apply_edits(mut image: DynamicImage, options: &EditOptions) -> Result<DynamicImage, String> {
    let rotation = validate_rotation(options.rotation)?;
    image = match rotation {
        90 => image.rotate90(),
        180 => image.rotate180(),
        270 => image.rotate270(),
        _ => image,
    };
    if options.flip_horizontal {
        image = image.fliph();
    }
    if options.flip_vertical {
        image = image.flipv();
    }

    let crops = [
        options.crop_left,
        options.crop_top,
        options.crop_right,
        options.crop_bottom,
    ];
    if crops
        .iter()
        .any(|value| !value.is_finite() || !(0.0..=45.0).contains(value))
    {
        return Err("Each crop edge must be between 0 and 45 percent".into());
    }
    if options.crop_left + options.crop_right >= 90.0
        || options.crop_top + options.crop_bottom >= 90.0
    {
        return Err("Crop settings must leave at least 10 percent of the image".into());
    }
    if crops.iter().any(|value| *value > 0.0) {
        let (width, height) = image.dimensions();
        let left = ((width as f32 * options.crop_left / 100.0).round() as u32)
            .min(width.saturating_sub(1));
        let top = ((height as f32 * options.crop_top / 100.0).round() as u32)
            .min(height.saturating_sub(1));
        let right = ((width as f32 * options.crop_right / 100.0).round() as u32)
            .min(width.saturating_sub(left + 1));
        let bottom = ((height as f32 * options.crop_bottom / 100.0).round() as u32)
            .min(height.saturating_sub(top + 1));
        let crop_width = width.saturating_sub(left + right).max(1);
        let crop_height = height.saturating_sub(top + bottom).max(1);
        image = image.crop_imm(left, top, crop_width, crop_height);
    }

    if let Some(width) = options.resize_width {
        if width == 0 || width > 20_000 {
            return Err("Resize width must be between 1 and 20,000 pixels".into());
        }
    }
    if let Some(height) = options.resize_height {
        if height == 0 || height > 20_000 {
            return Err("Resize height must be between 1 and 20,000 pixels".into());
        }
    }
    match (options.resize_width, options.resize_height) {
        (Some(width), Some(height)) => {
            image = image.resize_exact(width, height, FilterType::Lanczos3);
        }
        (Some(width), None) => {
            let height = ((image.height() as f64 * width as f64 / image.width() as f64).round()
                as u32)
                .max(1);
            image = image.resize_exact(width, height, FilterType::Lanczos3);
        }
        (None, Some(height)) => {
            let width = ((image.width() as f64 * height as f64 / image.height() as f64).round()
                as u32)
                .max(1);
            image = image.resize_exact(width, height, FilterType::Lanczos3);
        }
        (None, None) => {}
    }

    if !(-100..=100).contains(&options.brightness) {
        return Err("Brightness must be between -100 and 100".into());
    }
    if !(-100.0..=100.0).contains(&options.contrast) || !options.contrast.is_finite() {
        return Err("Contrast must be between -100 and 100".into());
    }
    if options.brightness != 0 {
        image = image.brighten(options.brightness);
    }
    if options.contrast.abs() > f32::EPSILON {
        image = image.adjust_contrast(options.contrast);
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
pub fn preview_edited_photo(
    root: String,
    path: String,
    options: EditOptions,
    max_edge: u32,
) -> Result<String, String> {
    let source = crate::safe_media_path(&root, &path)?;
    if !(128..=2_048).contains(&max_edge) {
        return Err("Preview size must be between 128 and 2048 pixels".into());
    }
    let source_bytes = fs::metadata(&source).map_err(|e| e.to_string())?.len();
    if source_bytes > MAX_EDIT_SOURCE_BYTES {
        return Err("Editing is limited to source images up to 250 MB".into());
    }
    let image = image::ImageReader::open(&source)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| format!("Cannot decode this image for editing: {e}"))?;
    let edited = apply_edits(image, &options)?;
    let preview = edited.thumbnail(max_edge, max_edge);
    let mut buffer = Cursor::new(Vec::new());
    preview
        .write_to(&mut buffer, ImageFormat::Png)
        .map_err(|e| format!("Cannot encode edit preview: {e}"))?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(buffer.into_inner())
    ))
}

#[tauri::command]
pub fn export_edited_copy(
    root: String,
    path: String,
    destination: String,
    options: EditOptions,
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
    let edited = apply_edits(image, &options)?;
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
        let too_bright = EditOptions {
            rotation: 0,
            flip_horizontal: false,
            flip_vertical: false,
            brightness: 101,
            contrast: 0.0,
            crop_left: 0.0,
            crop_top: 0.0,
            crop_right: 0.0,
            crop_bottom: 0.0,
            resize_width: None,
            resize_height: None,
        };
        let too_contrasty = EditOptions {
            brightness: 0,
            contrast: 101.0,
            ..too_bright
        };
        assert!(apply_edits(image.clone(), &too_bright).is_err());
        assert!(apply_edits(image, &too_contrasty).is_err());
    }

    #[test]
    fn quarter_turn_changes_dimensions() {
        let image = DynamicImage::new_rgb8(4, 3);
        let options = EditOptions {
            rotation: 90,
            flip_horizontal: false,
            flip_vertical: false,
            brightness: 0,
            contrast: 0.0,
            crop_left: 0.0,
            crop_top: 0.0,
            crop_right: 0.0,
            crop_bottom: 0.0,
            resize_width: None,
            resize_height: None,
        };
        let edited = apply_edits(image, &options).unwrap();
        assert_eq!((edited.width(), edited.height()), (3, 4));
    }    #[test]
    fn crop_and_resize_are_bounded() {
        let image = DynamicImage::new_rgb8(100, 80);
        let options = EditOptions {
            rotation: 0,
            flip_horizontal: false,
            flip_vertical: false,
            brightness: 0,
            contrast: 0.0,
            crop_left: 10.0,
            crop_top: 10.0,
            crop_right: 10.0,
            crop_bottom: 10.0,
            resize_width: Some(40),
            resize_height: None,
        };
        let edited = apply_edits(image, &options).unwrap();
        assert_eq!(edited.width(), 40);
        assert_eq!(edited.height(), 32);
    }

    #[test]
    fn excessive_crop_is_rejected() {
        let image = DynamicImage::new_rgb8(100, 80);
        let options = EditOptions {
            rotation: 0,
            flip_horizontal: false,
            flip_vertical: false,
            brightness: 0,
            contrast: 0.0,
            crop_left: 45.0,
            crop_top: 0.0,
            crop_right: 45.0,
            crop_bottom: 0.0,
            resize_width: None,
            resize_height: None,
        };
        assert!(apply_edits(image, &options).is_err());
    }

}
