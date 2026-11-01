use exif::{In, Reader, Tag, Value};
use nom_exif::{read_exif, MediaParser, MediaSource};
use serde::Serialize;
use std::{fs::File, io::BufReader, path::Path};

#[derive(Serialize)]
pub struct GeoPoint {
    path: String,
    lat: f64,
    lon: f64,
}
fn coordinate(value: &Value) -> Option<f64> {
    let Value::Rational(ref values) = *value else {
        return None;
    };
    if values.len() < 3 || values.iter().take(3).any(|v| v.denom == 0) {
        return None;
    }
    let x = |i: usize| values[i].num as f64 / values[i].denom as f64;
    let (degrees, minutes, seconds) = (x(0), x(1), x(2));
    if !(0.0..60.0).contains(&minutes) || !(0.0..60.0).contains(&seconds) {
        return None;
    }
    Some(degrees + minutes / 60.0 + seconds / 3600.0)
}
fn reference(value: &Value) -> Option<String> {
    match value {
        Value::Ascii(values) => Some(
            String::from_utf8_lossy(values.first()?)
                .trim_end_matches(['\0', ' ', '\t', '\r', '\n'])
                .to_ascii_uppercase(),
        ),
        _ => None,
    }
}
fn read_point(path: &Path) -> Option<(f64, f64)> {
    let file = File::open(path).ok()?;
    let metadata = Reader::new()
        .read_from_container(&mut BufReader::new(file))
        .ok()?;
    let latitude = coordinate(&metadata.get_field(Tag::GPSLatitude, In::PRIMARY)?.value)?;
    let longitude = coordinate(&metadata.get_field(Tag::GPSLongitude, In::PRIMARY)?.value)?;
    let lat_ref = reference(&metadata.get_field(Tag::GPSLatitudeRef, In::PRIMARY)?.value)?;
    let lon_ref = reference(&metadata.get_field(Tag::GPSLongitudeRef, In::PRIMARY)?.value)?;
    if !matches!(lat_ref.as_str(), "N" | "S") || !matches!(lon_ref.as_str(), "E" | "W") {
        return None;
    }
    let lat = if lat_ref == "S" { -latitude } else { latitude };
    let lon = if lon_ref == "W" {
        -longitude
    } else {
        longitude
    };
    (lat.is_finite() && lon.is_finite() && lat.abs() <= 90.0 && lon.abs() <= 180.0)
        .then_some((lat, lon))
}
// The existing nom-exif dependency supports HEIC EXIF and QuickTime track locations.
// Parse in memory and keep the source files read-only.
fn valid_point(lat: f64, lon: f64) -> Option<(f64, f64)> {
    (lat.is_finite() && lon.is_finite() && lat.abs() <= 90.0 && lon.abs() <= 180.0)
        .then_some((lat, lon))
}
fn read_iphone_point(path: &Path, extension: &str) -> Option<(f64, f64)> {
    let gps = if matches!(extension, "heic" | "heif") {
        read_exif(path).ok()?.gps_info()?.clone()
    } else {
        let mut parser = MediaParser::new();
        let source = MediaSource::open(path).ok()?;
        parser.parse_track(source).ok()?.gps_info()?.clone()
    };
    valid_point(gps.latitude_decimal()?, gps.longitude_decimal()?)
}
#[tauri::command]
pub fn gps_for_photos(root: String, paths: Vec<String>) -> Result<Vec<GeoPoint>, String> {
    if paths.len() > 5000 {
        return Err("Limit map scan to 5,000 photos per request".into());
    }
    let root = std::fs::canonicalize(root).map_err(|e| e.to_string())?;
    let mut points = Vec::new();
    for original in paths {
        let Ok(path) = std::fs::canonicalize(&original) else {
            continue;
        };
        if !path.starts_with(&root) || !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(ext.as_str(), "jpg" | "jpeg" | "tif" | "tiff" | "heic" | "heif" | "mov" | "mp4") {
            continue;
        }
        let location = if matches!(ext.as_str(), "jpg" | "jpeg" | "tif" | "tiff") {
            read_point(&path)
        } else {
            read_iphone_point(&path, &ext)
        };
        if let Some((lat, lon)) = location {
            points.push(GeoPoint {
                path: original,
                lat,
                lon,
            });
        }
    }
    Ok(points)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_exif_minutes_and_seconds() {
        let valid = Value::Rational(vec![
            exif::Rational { num: 27, denom: 1 },
            exif::Rational { num: 59, denom: 1 },
            exif::Rational {
                num: 599,
                denom: 10,
            },
        ]);
        let invalid_minutes = Value::Rational(vec![
            exif::Rational { num: 27, denom: 1 },
            exif::Rational { num: 60, denom: 1 },
            exif::Rational { num: 0, denom: 1 },
        ]);
        let invalid_seconds = Value::Rational(vec![
            exif::Rational { num: 27, denom: 1 },
            exif::Rational { num: 0, denom: 1 },
            exif::Rational { num: 60, denom: 1 },
        ]);
        assert!(coordinate(&valid).is_some());
        assert!(coordinate(&invalid_minutes).is_none());
        assert!(coordinate(&invalid_seconds).is_none());
    }
    #[test]
    fn rejects_zero_denominators_and_incomplete_gps() {
        assert!(coordinate(&Value::Rational(vec![
            exif::Rational { num: 1, denom: 1 },
            exif::Rational { num: 2, denom: 0 },
            exif::Rational { num: 3, denom: 1 },
        ]))
        .is_none());
        assert!(coordinate(&Value::Rational(vec![exif::Rational { num: 1, denom: 1 },])).is_none());
    }
    #[test]
    fn accepts_padded_exif_hemisphere_references() {
        assert_eq!(
            reference(&Value::Ascii(vec![b"S \0".to_vec()])),
            Some("S".into())
        );
    }
    #[test]
    fn validates_iphone_gps_coordinate_bounds() {
        assert_eq!(valid_point(27.95, -82.46), Some((27.95, -82.46)));
        assert!(valid_point(91.0, 0.0).is_none());
        assert!(valid_point(0.0, -181.0).is_none());
        assert!(valid_point(f64::NAN, 1.0).is_none());
    }
    #[test]
    fn rejects_excessive_map_batch() {
        assert!(gps_for_photos(".".into(), vec![String::new(); 5001]).is_err());
    }
    #[test]
    fn ignores_non_photo_files() {
        let result = gps_for_photos(".".into(), vec!["unknown.mov".into()]).unwrap();
        assert!(result.is_empty());
    }
}
