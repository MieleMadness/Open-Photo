use exif::{In, Reader, Tag, Value};
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
    Some(x(0) + x(1) / 60.0 + x(2) / 3600.0)
}
fn reference(value: &Value) -> Option<String> {
    match value {
        Value::Ascii(values) => Some(
            String::from_utf8_lossy(values.first()?)
                .trim_end_matches('\0')
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
        if !matches!(ext.as_str(), "jpg" | "jpeg" | "tif" | "tiff") {
            continue;
        }
        if let Some((lat, lon)) = read_point(&path) {
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
    fn rejects_excessive_map_batch() {
        assert!(gps_for_photos(".".into(), vec![String::new(); 5001]).is_err());
    }
    #[test]
    fn ignores_non_photo_files() {
        let result = gps_for_photos(".".into(), vec!["unknown.mov".into()]).unwrap();
        assert!(result.is_empty());
    }
}
