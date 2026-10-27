//! Phase 4 non-destructive photo edits.
//! The source media is always read-only. Edits are written only to a new, explicitly chosen file.
use base64::Engine;
use image::{
    codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, GenericImageView, ImageFormat,
};
use serde::{Deserialize, Serialize};
use std::fs::{self, FileTimes};
use std::{io::Cursor, path::PathBuf};

const MAX_EDIT_SOURCE_BYTES: u64 = 250 * 1024 * 1024;
const MAX_EDIT_OUTPUT_PIXELS: u64 = 100_000_000;

fn default_jpeg_quality() -> u8 {
    92
}

fn default_preserve_modified_time() -> bool {
    true
}

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
    #[serde(default)]
    grayscale: bool,
    #[serde(default)]
    hue_rotate: i32,
    #[serde(default)]
    blur_sigma: f32,
    #[serde(default)]
    sharpen_sigma: f32,
    #[serde(default)]
    sharpen_threshold: i32,
    #[serde(default = "default_jpeg_quality")]
    jpeg_quality: u8,
    #[serde(default = "default_preserve_modified_time")]
    preserve_modified_time: bool,
}

#[derive(Serialize)]
pub struct EditedPreviewResult {
    data_uri: String,
    width: u32,
    height: u32,
    source_width: u32,
    source_height: u32,
}

#[derive(Serialize)]
pub struct EditedExportResult {
    path: String,
    width: u32,
    height: u32,
    bytes: u64,
    preserved_modified_time: bool,
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

fn validate_output_dimensions(width: u32, height: u32) -> Result<(u32, u32), String> {
    let pixels = u64::from(width) * u64::from(height);
    if pixels == 0 || pixels > MAX_EDIT_OUTPUT_PIXELS {
        return Err("Edited output is limited to 100 megapixels".into());
    }
    Ok((width, height))
}

fn crop_rect(
    width: u32,
    height: u32,
    options: &EditOptions,
) -> Result<(u32, u32, u32, u32), String> {
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

    let left =
        ((width as f32 * options.crop_left / 100.0).round() as u32).min(width.saturating_sub(1));
    let top =
        ((height as f32 * options.crop_top / 100.0).round() as u32).min(height.saturating_sub(1));
    let right = ((width as f32 * options.crop_right / 100.0).round() as u32)
        .min(width.saturating_sub(left + 1));
    let bottom = ((height as f32 * options.crop_bottom / 100.0).round() as u32)
        .min(height.saturating_sub(top + 1));
    let crop_width = width.saturating_sub(left + right).max(1);
    let crop_height = height.saturating_sub(top + bottom).max(1);
    Ok((left, top, crop_width, crop_height))
}

fn target_dimensions(width: u32, height: u32, options: &EditOptions) -> Result<(u32, u32), String> {
    if let Some(resize_width) = options.resize_width {
        if resize_width == 0 || resize_width > 20_000 {
            return Err("Resize width must be between 1 and 20,000 pixels".into());
        }
    }
    if let Some(resize_height) = options.resize_height {
        if resize_height == 0 || resize_height > 20_000 {
            return Err("Resize height must be between 1 and 20,000 pixels".into());
        }
    }

    let dimensions = match (options.resize_width, options.resize_height) {
        (Some(resize_width), Some(resize_height)) => (resize_width, resize_height),
        (Some(resize_width), None) => {
            let resize_height =
                ((height as f64 * resize_width as f64 / width as f64).round() as u32).max(1);
            (resize_width, resize_height)
        }
        (None, Some(resize_height)) => {
            let resize_width =
                ((width as f64 * resize_height as f64 / height as f64).round() as u32).max(1);
            (resize_width, resize_height)
        }
        (None, None) => (width, height),
    };
    validate_output_dimensions(dimensions.0, dimensions.1)
}

fn edited_output_dimensions(
    source_width: u32,
    source_height: u32,
    options: &EditOptions,
) -> Result<(u32, u32), String> {
    let rotation = validate_rotation(options.rotation)?;
    let (rotated_width, rotated_height) = match rotation {
        90 | 270 => (source_height, source_width),
        _ => (source_width, source_height),
    };
    let (_, _, crop_width, crop_height) = crop_rect(rotated_width, rotated_height, options)?;
    target_dimensions(crop_width, crop_height, options)
}

fn preview_resize_dimensions(width: u32, height: u32, max_edge: u32) -> (u32, u32) {
    if width <= max_edge && height <= max_edge {
        return (width, height);
    }
    if width >= height {
        let preview_height =
            ((height as f64 * max_edge as f64 / width as f64).round() as u32).max(1);
        (max_edge, preview_height)
    } else {
        let preview_width =
            ((width as f64 * max_edge as f64 / height as f64).round() as u32).max(1);
        (preview_width, max_edge)
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

    let (left, top, crop_width, crop_height) = crop_rect(image.width(), image.height(), options)?;
    if (crop_width, crop_height) != image.dimensions() {
        image = image.crop_imm(left, top, crop_width, crop_height);
    }

    let (target_width, target_height) = target_dimensions(image.width(), image.height(), options)?;
    if (target_width, target_height) != image.dimensions() {
        image = image.resize_exact(target_width, target_height, FilterType::Lanczos3);
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

    if !(-180..=180).contains(&options.hue_rotate) {
        return Err("Hue rotation must be between -180 and 180 degrees".into());
    }
    if options.hue_rotate != 0 {
        image = image.huerotate(options.hue_rotate);
    }

    if !options.blur_sigma.is_finite() || !(0.0..=10.0).contains(&options.blur_sigma) {
        return Err("Blur must be between 0 and 10".into());
    }
    if options.blur_sigma > f32::EPSILON {
        image = image.blur(options.blur_sigma);
    }

    if !options.sharpen_sigma.is_finite() || !(0.0..=10.0).contains(&options.sharpen_sigma) {
        return Err("Sharpen strength must be between 0 and 10".into());
    }
    if !(0..=100).contains(&options.sharpen_threshold) {
        return Err("Sharpen threshold must be between 0 and 100".into());
    }
    if options.sharpen_sigma > f32::EPSILON {
        image = image.unsharpen(options.sharpen_sigma, options.sharpen_threshold);
    }

    if options.grayscale {
        image = image.grayscale();
    }
    Ok(image)
}

fn editable_source_extension(extension: &str) -> bool {
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "jpg" | "jpeg" | "png" | "webp" | "tif" | "tiff" | "bmp" | "avif"
    )
}

fn validate_edit_source(source: &std::path::Path) -> Result<(), String> {
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if !editable_source_extension(extension) {
        return Err(
            "This media type is not supported by the Phase 4 image editor. Animated GIF, HEIC/HEIF, RAW and video editing are not enabled yet."
                .into(),
        );
    }
    Ok(())
}

fn validate_jpeg_quality(quality: u8) -> Result<u8, String> {
    if !(40..=100).contains(&quality) {
        return Err("JPEG quality must be between 40 and 100".into());
    }
    Ok(quality)
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

fn preview_edited_photo_blocking(
    root: String,
    path: String,
    options: EditOptions,
    max_edge: u32,
) -> Result<EditedPreviewResult, String> {
    let source = crate::safe_media_path(&root, &path)?;
    validate_edit_source(&source)?;
    if !(128..=2_048).contains(&max_edge) {
        return Err("Preview size must be between 128 and 2048 pixels".into());
    }
    let source_bytes = fs::metadata(&source).map_err(|e| e.to_string())?.len();
    if source_bytes > MAX_EDIT_SOURCE_BYTES {
        return Err("Editing is limited to source images up to 250 MB".into());
    }
    let image = crate::image_decode::editor_source(&source)?;
    let source_width = image.width();
    let source_height = image.height();
    let (width, height) = edited_output_dimensions(source_width, source_height, &options)?;
    // Render the same operations at source/output scale, then downsample.
    // Applying blur/sharpen after downsampling exaggerates their visible strength.
    let edited = apply_edits(image, &options)?;
    let (preview_width, preview_height) = preview_resize_dimensions(width, height, max_edge);
    let preview = edited.resize_exact(preview_width, preview_height, FilterType::Lanczos3);
    let mut buffer = Cursor::new(Vec::new());
    preview
        .write_to(&mut buffer, ImageFormat::Png)
        .map_err(|e| format!("Cannot encode edit preview: {e}"))?;
    Ok(EditedPreviewResult {
        data_uri: format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(buffer.into_inner())
        ),
        width,
        height,
        source_width,
        source_height,
    })
}

fn export_edited_copy_blocking(
    root: String,
    path: String,
    destination: String,
    options: EditOptions,
) -> Result<EditedExportResult, String> {
    let source = crate::safe_media_path(&root, &path)?;
    validate_edit_source(&source)?;
    let source_metadata = fs::metadata(&source).map_err(|e| e.to_string())?;
    let source_bytes = source_metadata.len();
    if source_bytes > MAX_EDIT_SOURCE_BYTES {
        return Err("Editing is limited to source images up to 250 MB".into());
    }
    let source_modified = if options.preserve_modified_time {
        Some(
            source_metadata
                .modified()
                .map_err(|e| format!("Cannot read source modified time: {e}"))?,
        )
    } else {
        None
    };

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

    let image = crate::image_decode::editor_source(&source)?;
    let edited = apply_edits(image, &options)?;
    let width = edited.width();
    let height = edited.height();

    let mut staged = crate::safe_output::StagedOutput::new(&destination)?;
    let output = staged.file();
    if format == ImageFormat::Jpeg {
        let quality = validate_jpeg_quality(options.jpeg_quality)?;
        JpegEncoder::new_with_quality(&mut *output, quality).encode_image(&edited)
    } else {
        edited.write_to(&mut *output, format)
    }
    .map_err(|error| format!("Cannot encode edited export: {error}"))?;
    if let Some(modified) = source_modified {
        output
            .set_times(FileTimes::new().set_modified(modified))
            .map_err(|error| format!("Cannot preserve source modified time: {error}"))?;
    }
    staged.commit()?;
    let bytes = fs::metadata(&destination).map_err(|e| e.to_string())?.len();

    Ok(EditedExportResult {
        path: destination.to_string_lossy().into_owned(),
        width,
        height,
        bytes,
        preserved_modified_time: source_modified.is_some(),
    })
}

#[tauri::command]
pub async fn preview_edited_photo(
    jobs: tauri::State<'_, crate::media_jobs::ImageJobs>,
    root: String,
    path: String,
    options: EditOptions,
    max_edge: u32,
) -> Result<EditedPreviewResult, String> {
    use std::sync::atomic::Ordering;
    let generation = jobs.preview_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let latest = jobs.preview_generation.clone();
    let permit = jobs
        .gate
        .clone()
        .acquire_owned()
        .await
        .map_err(|e| e.to_string())?;
    if latest.load(Ordering::SeqCst) != generation {
        return Err("Preview superseded".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        if latest.load(Ordering::SeqCst) != generation {
            return Err("Preview superseded".into());
        }
        preview_edited_photo_blocking(root, path, options, max_edge)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn export_edited_copy(
    jobs: tauri::State<'_, crate::media_jobs::ImageJobs>,
    root: String,
    path: String,
    destination: String,
    options: EditOptions,
) -> Result<EditedExportResult, String> {
    let permit = jobs
        .gate
        .clone()
        .acquire_owned()
        .await
        .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        export_edited_copy_blocking(root, path, destination, options)
    })
    .await
    .map_err(|e| e.to_string())?
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
            grayscale: false,
            hue_rotate: 0,
            blur_sigma: 0.0,
            sharpen_sigma: 0.0,
            sharpen_threshold: 0,
            jpeg_quality: 92,
            preserve_modified_time: true,
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
            grayscale: false,
            hue_rotate: 0,
            blur_sigma: 0.0,
            sharpen_sigma: 0.0,
            sharpen_threshold: 0,
            jpeg_quality: 92,
            preserve_modified_time: true,
        };
        let edited = apply_edits(image, &options).unwrap();
        assert_eq!((edited.width(), edited.height()), (3, 4));
    }

    #[test]
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
            grayscale: false,
            hue_rotate: 0,
            blur_sigma: 0.0,
            sharpen_sigma: 0.0,
            sharpen_threshold: 0,
            jpeg_quality: 92,
            preserve_modified_time: true,
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
            grayscale: false,
            hue_rotate: 0,
            blur_sigma: 0.0,
            sharpen_sigma: 0.0,
            sharpen_threshold: 0,
            jpeg_quality: 92,
            preserve_modified_time: true,
        };
        assert!(apply_edits(image, &options).is_err());
    }

    #[test]
    fn adjustment_limits_are_enforced() {
        let image = DynamicImage::new_rgb8(8, 8);
        let base = EditOptions {
            rotation: 0,
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
            grayscale: false,
            hue_rotate: 0,
            blur_sigma: 0.0,
            sharpen_sigma: 0.0,
            sharpen_threshold: 0,
            jpeg_quality: 92,
            preserve_modified_time: true,
        };
        assert!(apply_edits(
            image.clone(),
            &EditOptions {
                hue_rotate: 181,
                ..base
            }
        )
        .is_err());
        assert!(apply_edits(
            image.clone(),
            &EditOptions {
                blur_sigma: 10.1,
                ..base
            }
        )
        .is_err());
        assert!(apply_edits(
            image,
            &EditOptions {
                sharpen_threshold: 101,
                ..base
            }
        )
        .is_err());
    }

    #[test]
    fn jpeg_quality_is_bounded() {
        assert_eq!(validate_jpeg_quality(92).unwrap(), 92);
        assert!(validate_jpeg_quality(39).is_err());
        assert!(validate_jpeg_quality(101).is_err());
    }

    #[test]
    fn preserve_modified_time_defaults_on() {
        assert!(default_preserve_modified_time());
    }

    #[test]
    fn output_pixel_limit_is_enforced() {
        assert_eq!(
            validate_output_dimensions(10_000, 10_000).unwrap(),
            (10_000, 10_000)
        );
        assert!(validate_output_dimensions(20_000, 20_000).is_err());
    }

    #[test]
    fn preview_resize_is_bounded_by_max_edge() {
        assert_eq!(
            preview_resize_dimensions(20_000, 10_000, 1_400),
            (1_400, 700)
        );
        assert_eq!(preview_resize_dimensions(800, 600, 1_400), (800, 600));
    }

    #[test]
    fn output_dimensions_follow_rotation_crop_and_resize() {
        let options = EditOptions {
            rotation: 90,
            flip_horizontal: false,
            flip_vertical: false,
            brightness: 0,
            contrast: 0.0,
            crop_left: 10.0,
            crop_top: 0.0,
            crop_right: 10.0,
            crop_bottom: 0.0,
            resize_width: Some(1_000),
            resize_height: None,
            grayscale: false,
            hue_rotate: 0,
            blur_sigma: 0.0,
            sharpen_sigma: 0.0,
            sharpen_threshold: 0,
            jpeg_quality: 92,
            preserve_modified_time: true,
        };
        assert_eq!(
            edited_output_dimensions(4_000, 3_000, &options).unwrap(),
            (1_000, 1_667)
        );
    }

    #[test]
    fn editor_source_formats_exclude_animated_gif() {
        assert!(editable_source_extension("jpg"));
        assert!(editable_source_extension("AVIF"));
        assert!(!editable_source_extension("gif"));
        assert!(!editable_source_extension("heic"));
    }
}
