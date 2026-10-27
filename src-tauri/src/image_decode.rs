//! Shared decode limits and EXIF orientation, before any crop or AI coordinates.
use image::{DynamicImage, ImageDecoder, ImageReader, Limits};
use std::{fs, path::Path};

pub(crate) const POLICY: &str = "oriented-v2";
const MAX_PIXELS: u64 = 50_000_000;

pub(crate) fn decode(path: &Path) -> Result<DynamicImage, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 250 * 1024 * 1024 {
        return Err("Image decoding is limited to files up to 250 MB".into());
    }
    let mut reader = ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(20_000);
    limits.max_image_height = Some(20_000);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err("Image decoding is limited to 50 megapixels".into());
    }
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    image.apply_orientation(orientation);
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{metadata::Orientation, Rgb, RgbImage};

    #[test]
    fn all_exif_orientations_transform_distinct_pixels() {
        let pixels = RgbImage::from_fn(3, 2, |x, y| Rgb([(y * 3 + x + 1) as u8, 0, 0]));
        let expected = [
            vec![1, 2, 3, 4, 5, 6],
            vec![3, 2, 1, 6, 5, 4],
            vec![6, 5, 4, 3, 2, 1],
            vec![4, 5, 6, 1, 2, 3],
            vec![1, 4, 2, 5, 3, 6],
            vec![4, 1, 5, 2, 6, 3],
            vec![6, 3, 5, 2, 4, 1],
            vec![3, 6, 2, 5, 1, 4],
        ];
        for (index, expected) in expected.iter().enumerate() {
            let mut image = DynamicImage::ImageRgb8(pixels.clone());
            image.apply_orientation(Orientation::from_exif(index as u8 + 1).unwrap());
            assert_eq!(
                image.to_rgb8().pixels().map(|p| p[0]).collect::<Vec<_>>(),
                *expected
            );
        }
    }
}
