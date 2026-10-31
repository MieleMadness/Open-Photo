//! Typed timeline sources. A still image has geometry, never invented video metadata.
use crate::{
    video_probe::VideoInfo,
    video_project::{self, Clip, Output, VideoTime},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Media {
    Video { info: Box<VideoInfo> },
    Photo { width: u32, height: u32 },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub id: String,
    pub relative_path: String,
    pub sha256: String,
    pub bytes: u64,
    pub media: Media,
}
impl From<video_project::Asset> for Asset {
    fn from(a: video_project::Asset) -> Self {
        Self {
            id: a.id,
            relative_path: a.relative_path,
            sha256: a.sha256,
            bytes: a.bytes,
            media: Media::Video {
                info: Box::new(a.info),
            },
        }
    }
}
impl Asset {
    pub fn video(&self) -> Result<video_project::Asset, String> {
        match &self.media {
            Media::Video { info } => Ok(video_project::Asset {
                id: self.id.clone(),
                relative_path: self.relative_path.clone(),
                sha256: self.sha256.clone(),
                bytes: self.bytes,
                info: info.as_ref().clone(),
            }),
            Media::Photo { .. } => Err("This source is a photo".into()),
        }
    }
    pub fn time_base(&self) -> &str {
        match &self.media {
            Media::Video { info } => info.time_base.as_deref().unwrap_or("1/1000000"),
            Media::Photo { .. } => "1/1000000",
        }
    }
    pub fn default_duration(&self) -> f64 {
        match &self.media {
            Media::Video { info } => info.duration_seconds,
            Media::Photo { .. } => 3.0,
        }
    }
    pub fn is_photo(&self) -> bool {
        matches!(self.media, Media::Photo { .. })
    }
}
pub fn validate_photo(asset: &Asset, clip: &Clip, output: &Output) -> Result<(), String> {
    let Media::Photo { width, height } = asset.media else {
        return Err("Expected a photo source".into());
    };
    video_project::validate_identity(
        &asset.id,
        &asset.id,
        "Photo",
        &asset.relative_path,
        &asset.sha256,
        asset.bytes,
    )?;
    if width == 0
        || height == 0
        || width > 20_000
        || height > 20_000
        || u64::from(width) * u64::from(height) > 50_000_000
    {
        return Err("Unsupported photo dimensions".into());
    }
    if !video_project::valid_id(&clip.id)
        || clip.asset_id != asset.id
        || clip.source_in.time_base != "1/1000000"
        || clip.source_out.time_base != "1/1000000"
    {
        return Err("Invalid photo clip timing".into());
    }
    let start = clip.source_in.seconds()?;
    let end = clip.source_out.seconds()?;
    if end - start < 0.04 || end > 600.0 {
        return Err("Photo clips must last 0.04–600 seconds".into());
    }
    video_project::validate_edits_output(&clip.edits, output)
}
pub fn photo_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "jpg" | "jpeg" | "png" | "bmp" | "tif" | "tiff"
    )
}
pub fn decode_photo(path: &Path) -> Result<image::DynamicImage, String> {
    if !photo_extension(path) {
        return Err("Timeline photos currently support JPEG, PNG, BMP and TIFF".into());
    }
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 250 * 1024 * 1024 {
        return Err("Timeline photos are limited to 250 MB".into());
    }
    let format = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .format();
    if !matches!(
        format,
        Some(
            image::ImageFormat::Jpeg
                | image::ImageFormat::Png
                | image::ImageFormat::Bmp
                | image::ImageFormat::Tiff
        )
    ) {
        return Err("The file is not a supported still image".into());
    }
    // Animation needs an explicit timeline policy; never silently use an APNG's first frame.
    if format == Some(image::ImageFormat::Png) {
        let decoder = image::codecs::png::PngDecoder::new(std::io::BufReader::new(
            fs::File::open(path).map_err(|e| e.to_string())?,
        ))
        .map_err(|e| e.to_string())?;
        if decoder.is_apng().map_err(|e| e.to_string())? {
            return Err("Animated PNG is not supported as a still timeline clip".into());
        }
    }
    crate::image_decode::decode(path)
}
pub fn flatten_photo(image: image::DynamicImage) -> image::RgbImage {
    let rgba = image.into_rgba8();
    image::RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let pixel = rgba.get_pixel(x, y).0;
        image::Rgb(
            [0, 1, 2].map(|channel| {
                ((u16::from(pixel[channel]) * u16::from(pixel[3]) + 127) / 255) as u8
            }),
        )
    })
}
pub fn source(root: &str, asset: &Asset, cancel: impl Fn() -> bool) -> Result<PathBuf, String> {
    let candidate = Path::new(root).join(&asset.relative_path);
    let path = if asset.is_photo() {
        crate::safe_media_path(root, candidate.to_str().ok_or("Invalid media path")?)?
    } else {
        crate::media_stream::validated_video(root, candidate.to_str().ok_or("Invalid media path")?)?
    };
    if asset.is_photo() && !photo_extension(&path) {
        return Err("Unsupported timeline photo format".into());
    }
    let (bytes, hash) = video_project::fingerprint(&path, cancel)?;
    if bytes != asset.bytes || hash != asset.sha256 {
        return Err(
            "Source differs from this project's fingerprint. Relink the original file.".into(),
        );
    }
    Ok(path)
}
pub fn new_photo(root: &Path, file: &Path) -> Result<Asset, String> {
    let image = decode_photo(file)?;
    let (bytes, sha256) = video_project::fingerprint(file, || false)?;
    Ok(Asset {
        id: video_project::id(),
        relative_path: file
            .strip_prefix(root)
            .map_err(|_| "Photo is outside this workspace")?
            .to_str()
            .ok_or("Invalid photo path")?
            .replace('\\', "/"),
        sha256,
        bytes,
        media: Media::Photo {
            width: image.width(),
            height: image.height(),
        },
    })
}
pub fn default_clip(asset: &Asset) -> Result<Clip, String> {
    Ok(Clip {
        id: video_project::id(),
        asset_id: asset.id.clone(),
        source_in: VideoTime::from_seconds(0.0, asset.time_base())?,
        source_out: VideoTime::from_seconds(asset.default_duration(), asset.time_base())?,
        edits: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transparent_photos_composite_on_black_and_disguised_animation_is_rejected() {
        let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(2, 1, |x, _| {
            image::Rgba([255, 128, 64, if x == 0 { 0 } else { 128 }])
        }));
        let flattened = flatten_photo(image);
        assert_eq!(flattened.get_pixel(0, 0).0, [0, 0, 0]);
        assert_eq!(flattened.get_pixel(1, 0).0, [128, 64, 32]);
        let path = std::env::temp_dir().join(format!("disguised-{}.jpg", video_project::id()));
        fs::write(&path, b"GIF89a\x01\0\x01\0\x80\0\0\0\0\0\xff\xff\xff").unwrap();
        assert!(decode_photo(&path).is_err());
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn animated_png_is_rejected_before_still_normalization() {
        let path = std::env::temp_dir().join(format!("animated-{}.png", video_project::id()));
        image::RgbImage::from_pixel(2, 2, image::Rgb([255, 0, 0]))
            .save(&path)
            .unwrap();
        let mut png = fs::read(&path).unwrap();
        let mut control = b"acTL".to_vec();
        control.extend_from_slice(&1u32.to_be_bytes());
        control.extend_from_slice(&0u32.to_be_bytes());
        let mut crc = !0u32;
        for byte in &control {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ if crc & 1 != 0 { 0xedb88320 } else { 0 };
            }
        }
        control.extend_from_slice(&(!crc).to_be_bytes());
        let mut chunk = 8u32.to_be_bytes().to_vec();
        chunk.extend(control);
        png.splice(33..33, chunk);
        fs::write(&path, png).unwrap();
        assert!(decode_photo(&path).unwrap_err().contains("Animated PNG"));
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn photo_import_uses_oriented_dimensions_without_modifying_jpeg() {
        let root = std::env::temp_dir().join(format!("photo-orientation-{}", video_project::id()));
        fs::create_dir(&root).unwrap();
        let file = root.join("phone.jpg");
        let image = image::RgbImage::from_pixel(16, 24, image::Rgb([255, 0, 0]));
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
            .encode_image(&image)
            .unwrap();
        let exif = [
            b'E', b'x', b'i', b'f', 0, 0, b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 1, 3, 0, 1, 0,
            0, 0, 6, 0, 0, 0, 0, 0, 0, 0,
        ];
        let mut oriented = vec![0xff, 0xd8, 0xff, 0xe1];
        oriented.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        oriented.extend_from_slice(&exif);
        oriented.extend_from_slice(&jpeg[2..]);
        fs::write(&file, &oriented).unwrap();
        let asset = new_photo(&root, &file).unwrap();
        assert_eq!(
            asset.media,
            Media::Photo {
                width: 24,
                height: 16
            }
        );
        assert_eq!(fs::read(file).unwrap(), oriented);
        fs::remove_dir_all(root).unwrap();
    }
}
