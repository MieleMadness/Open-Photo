//! Read-only OpenCLIP image preprocessing for local semantic inference.
use base64::Engine;
use image::GenericImageView;
use serde::Serialize;

const CLIP_MEAN: [f32; 3] = [0.48145466, 0.4578275, 0.40821073];
const CLIP_STD: [f32; 3] = [0.26862954, 0.261_302_6, 0.275_777_1];

#[derive(Serialize)]
pub struct ImageTensorPreview {
    width: u32,
    height: u32,
    channels: u8,
    layout: &'static str,
    values_base64: String,
}

pub(crate) fn clip_tensor(input: image::DynamicImage) -> Result<Vec<f32>, String> {
    let (width, height) = input.dimensions();
    if width == 0 || height == 0 {
        return Err("Cannot preprocess an empty image".into());
    }
    let scale = 224.0_f32 / (width.min(height) as f32);
    let resized_width = ((width as f32 * scale).round() as u32).max(224);
    let resized_height = ((height as f32 * scale).round() as u32).max(224);
    if u64::from(resized_width) * u64::from(resized_height) > 16_777_216 {
        return Err(
            "Image aspect ratio exceeds bounded AI preprocessing; use a cropped copy".into(),
        );
    }
    let resized = input.resize_exact(
        resized_width,
        resized_height,
        image::imageops::FilterType::CatmullRom,
    );
    let left = (resized_width - 224) / 2;
    let top = (resized_height - 224) / 2;
    let rgb = resized.crop_imm(left, top, 224, 224).to_rgb8();

    let mut values = Vec::with_capacity(224 * 224 * 3);
    for channel in 0..3 {
        for pixel in rgb.pixels() {
            let value = f32::from(pixel[channel]) / 255.0;
            values.push((value - CLIP_MEAN[channel]) / CLIP_STD[channel]);
        }
    }
    Ok(values)
}

pub(crate) fn prepare_tensor(root: &str, path: &str) -> Result<Vec<f32>, String> {
    let source = crate::safe_media_path(root, path)?;
    let input = crate::image_decode::decode(&source)?;
    clip_tensor(input)
}

/// Convert a supported local photo to the supported OpenCLIP RGB 224x224 float32 CHW tensor.
/// This diagnostic command prepares input only and never modifies the original image.
#[tauri::command]
pub fn ai_prepare_image(root: String, path: String) -> Result<ImageTensorPreview, String> {
    let values = prepare_tensor(&root, &path)?;
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    Ok(ImageTensorPreview {
        width: 224,
        height: 224,
        channels: 3,
        layout: "OpenCLIP-CHW-f32-normalized-v1",
        values_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::{clip_tensor, CLIP_MEAN, CLIP_STD};

    #[test]
    fn extreme_aspect_ratios_are_rejected_before_resize_allocation() {
        let image = image::DynamicImage::new_rgb8(20_000, 1);
        assert!(clip_tensor(image)
            .unwrap_err()
            .contains("bounded AI preprocessing"));
    }

    #[test]
    fn clip_tensor_is_channel_first_and_normalized() {
        let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            224,
            224,
            image::Rgb([255, 0, 128]),
        ));
        let tensor = clip_tensor(image).unwrap();
        assert_eq!(tensor.len(), 224 * 224 * 3);
        let red = (1.0 - CLIP_MEAN[0]) / CLIP_STD[0];
        let green = (0.0 - CLIP_MEAN[1]) / CLIP_STD[1];
        assert!((tensor[0] - red).abs() < 0.0001);
        assert!((tensor[224 * 224] - green).abs() < 0.0001);
    }
}
