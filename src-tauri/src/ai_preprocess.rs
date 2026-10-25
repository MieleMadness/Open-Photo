//! Read-only AI image preprocessing for future local ONNX inference.
//! No image embeddings or classifications are produced by this module.
use base64::Engine;
use serde::Serialize;

#[derive(Serialize)]
pub struct ImageTensorPreview {
    width: u32,
    height: u32,
    channels: u8,
    layout: &'static str,
    values_base64: String,
}

/// Convert a supported local photo to a deterministic RGB 224x224 float32 CHW tensor.
/// This command prepares input only. It does not invoke an inference model.
#[tauri::command]
pub fn ai_prepare_image(root: String, path: String) -> Result<ImageTensorPreview, String> {
    let source = crate::safe_media_path(&root, &path)?;
    let input = image::ImageReader::open(source)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| format!("Cannot decode this photo for AI: {e}"))?;
    let rgb = input.resize_exact(224, 224, image::imageops::FilterType::Triangle).to_rgb8();
    let mut bytes = Vec::with_capacity(224 * 224 * 3 * 4);
    for channel in 0..3 {
        for pixel in rgb.pixels() {
            let value = f32::from(pixel[channel]) / 255.0;
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    Ok(ImageTensorPreview {
        width: 224,
        height: 224,
        channels: 3,
        layout: "CHW-f32-le-0-to-1",
        values_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn channel_first_tensor_is_consistent() {
        let image = image::RgbImage::from_pixel(224, 224, image::Rgb([255, 0, 128]));
        let mut tensor = Vec::new();
        for channel in 0..3 {
            for pixel in image.pixels() {
                tensor.extend_from_slice(&(f32::from(pixel[channel]) / 255.0).to_le_bytes());
            }
        }
        assert_eq!(tensor.len(), 224 * 224 * 3 * 4);
        assert_eq!(&tensor[0..4], &1_f32.to_le_bytes());
        assert_eq!(&tensor[(224 * 224 * 4)..(224 * 224 * 4 + 4)], &0_f32.to_le_bytes());
    }
}
