//! Shared decode limits and EXIF orientation, before any crop or AI coordinates.
use image::{DynamicImage, ImageDecoder, ImageReader, Limits};
use std::{
    fs,
    io::{BufRead, Cursor, Seek},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::SystemTime,
};

pub(crate) const POLICY: &str = "oriented-v2";
const MAX_PIXELS: u64 = 50_000_000;

struct CachedSource {
    path: PathBuf,
    bytes: u64,
    modified: SystemTime,
    image: DynamicImage,
}
static EDIT_SOURCE: OnceLock<Mutex<Option<CachedSource>>> = OnceLock::new();

/// At most one 64 MB normalized editor source is retained between slider changes.
pub(crate) fn editor_source(path: &Path) -> Result<DynamicImage, String> {
    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    let modified = metadata.modified().map_err(|e| e.to_string())?;
    let mut cache = EDIT_SOURCE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|e| e.to_string())?;
    if let Some(source) = cache.as_ref().filter(|source| {
        source.path == path && source.bytes == metadata.len() && source.modified == modified
    }) {
        return Ok(source.image.clone());
    }
    *cache = None;
    let image = decode(path)?;
    if image.as_bytes().len() <= 64 * 1024 * 1024 {
        *cache = Some(CachedSource {
            path: path.into(),
            bytes: metadata.len(),
            modified,
            image: image.clone(),
        });
    }
    Ok(image)
}

pub(crate) fn decode(path: &Path) -> Result<DynamicImage, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 250 * 1024 * 1024 {
        return Err("Image decoding is limited to files up to 250 MB".into());
    }
    let reader = ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    decode_reader(reader, true)
}

/// RAW metadata supplies orientation; do not apply embedded EXIF a second time.
pub(crate) fn decode_embedded(bytes: &[u8]) -> Result<DynamicImage, String> {
    if bytes.len() as u64 > MAX_EMBEDDED_BYTES {
        return Err("Embedded RAW previews are limited to 75 MB".into());
    }
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    decode_reader(reader, false)
}

pub(crate) const MAX_EMBEDDED_BYTES: u64 = 75 * 1024 * 1024;

pub(crate) fn validate_dimensions(width: u32, height: u32) -> Result<(), String> {
    if width > 20_000 || height > 20_000 {
        return Err("Image decoding is limited to 20,000 pixels per edge".into());
    }
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err("Image decoding is limited to 50 megapixels".into());
    }
    Ok(())
}

fn decode_reader<R: BufRead + Seek>(
    mut reader: ImageReader<R>,
    normalize_orientation: bool,
) -> Result<DynamicImage, String> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(20_000);
    limits.max_image_height = Some(20_000);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let (width, height) = decoder.dimensions();
    validate_dimensions(width, height)?;
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    if normalize_orientation {
        image.apply_orientation(orientation);
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{metadata::Orientation, GenericImageView, Rgb, RgbImage};

    #[test]
    fn jpeg_exif_is_normalized_before_edit_coordinates_without_changing_source() {
        let directory =
            std::env::temp_dir().join(format!("openphoto-orientation-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let source = directory.join("phone.jpg");
        let image = RgbImage::from_fn(3, 2, |x, y| Rgb([(x * 70) as u8, (y * 120) as u8, 40]));
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 100)
            .encode_image(&image)
            .unwrap();
        let baseline = image::load_from_memory(&jpeg).unwrap();
        for orientation in 1..=8_u8 {
            // One little-endian TIFF Orientation entry in an EXIF APP1 segment.
            let exif = [
                b'E',
                b'x',
                b'i',
                b'f',
                0,
                0,
                b'I',
                b'I',
                42,
                0,
                8,
                0,
                0,
                0,
                1,
                0,
                0x12,
                1,
                3,
                0,
                1,
                0,
                0,
                0,
                orientation,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ];
            let mut bytes = jpeg[..2].to_vec();
            bytes.extend([0xff, 0xe1]);
            bytes.extend(((exif.len() + 2) as u16).to_be_bytes());
            bytes.extend(exif);
            bytes.extend(&jpeg[2..]);
            std::fs::write(&source, &bytes).unwrap();
            let mut expected = baseline.clone();
            expected.apply_orientation(Orientation::from_exif(orientation).unwrap());
            let embedded = decode_embedded(&bytes).unwrap();
            assert_eq!(embedded.as_bytes(), baseline.as_bytes());
            assert_eq!(embedded.dimensions(), baseline.dimensions());
            let actual = decode(&source).unwrap();
            assert_eq!(actual.dimensions(), expected.dimensions());
            assert_eq!(actual.as_bytes(), expected.as_bytes());
            assert_eq!(std::fs::read(&source).unwrap(), bytes);
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn embedded_preview_limits_reject_oversized_headers() {
        let mut bmp = vec![0_u8; 58];
        bmp[..2].copy_from_slice(b"BM");
        bmp[2..6].copy_from_slice(&58_u32.to_le_bytes());
        bmp[10..14].copy_from_slice(&54_u32.to_le_bytes());
        bmp[14..18].copy_from_slice(&40_u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&20_001_u32.to_le_bytes());
        bmp[22..26].copy_from_slice(&1_u32.to_le_bytes());
        bmp[26..28].copy_from_slice(&1_u16.to_le_bytes());
        bmp[28..30].copy_from_slice(&24_u16.to_le_bytes());
        assert!(decode_embedded(&bmp).is_err());
        assert!(validate_dimensions(10_000, 5_001).is_err());
        assert!(validate_dimensions(20_001, 1).is_err());
        assert!(validate_dimensions(10_000, 5_000).is_ok());
    }

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
