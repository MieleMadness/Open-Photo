use image::ImageReader;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct SimilarGroup {
    pub paths: Vec<String>,
    pub max_distance: u32,
}

/// Difference hash: a fast CPU-only visual similarity estimate.
/// Similarity is advisory; it never modifies or deletes original media.
fn difference_hash(path: &Path) -> Result<u64, String> {
    let reader = ImageReader::open(path).map_err(|e| e.to_string())?;
    let image = reader
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())?
        .resize_exact(9, 8, image::imageops::FilterType::Triangle)
        .to_luma8();
    let mut hash = 0_u64;
    for row in 0..8 {
        for col in 0..8 {
            hash <<= 1;
            if image.get_pixel(col, row)[0] > image.get_pixel(col + 1, row)[0] {
                hash |= 1;
            }
        }
    }
    Ok(hash)
}

#[tauri::command]
pub fn find_similar_photos(
    roots: Vec<String>,
    paths: Vec<String>,
) -> Result<Vec<SimilarGroup>, String> {
    if roots.is_empty() || roots.len() > 64 || paths.len() > 5000 {
        return Err("Select 1–64 folders and up to 5,000 images".into());
    }
    let allowed: Vec<_> = roots
        .iter()
        .map(std::fs::canonicalize)
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let mut hashes = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for input in paths {
        let Ok(path) = std::fs::canonicalize(input) else {
            continue;
        };
        if !path.is_file()
            || !allowed.iter().any(|root| path.starts_with(root))
            || !seen.insert(path.clone())
        {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "webp" | "bmp" | "tiff" | "tif"
        ) {
            continue;
        }
        if path
            .metadata()
            .is_ok_and(|metadata| metadata.len() > 75 * 1024 * 1024)
        {
            continue;
        }
        if let Ok(hash) = difference_hash(&path) {
            hashes.push((path.to_string_lossy().into_owned(), hash));
        }
    }
    // Compare only against representatives to keep groups bounded.
    let mut groups: Vec<(u64, Vec<String>, u32)> = Vec::new();
    for (path, hash) in hashes {
        if let Some((representative, paths, max_distance)) = groups
            .iter_mut()
            .find(|(reference, _, _)| (hash ^ *reference).count_ones() <= 5)
        {
            *max_distance = (*max_distance).max((hash ^ *representative).count_ones());
            paths.push(path);
        } else {
            groups.push((hash, vec![path], 0));
        }
    }
    Ok(groups
        .into_iter()
        .filter(|(_, paths, _)| paths.len() > 1)
        .map(|(_, paths, max_distance)| SimilarGroup {
            paths,
            max_distance,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::find_similar_photos;
    #[test]
    fn groups_matching_images_but_not_opposite_gradients() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("openphoto-similar-{stamp}"));
        std::fs::create_dir_all(&root).unwrap();
        let first = root.join("one.png");
        let second = root.join("two.png");
        let opposite = root.join("opposite.png");
        let light_to_dark = image::GrayImage::from_fn(9, 8, |x, _| image::Luma([(x * 20) as u8]));
        let dark_to_light =
            image::GrayImage::from_fn(9, 8, |x, _| image::Luma([((8 - x) * 20) as u8]));
        light_to_dark.save(&first).unwrap();
        light_to_dark.save(&second).unwrap();
        dark_to_light.save(&opposite).unwrap();
        let paths = vec![
            first.to_string_lossy().into_owned(),
            second.to_string_lossy().into_owned(),
            opposite.to_string_lossy().into_owned(),
        ];
        let results =
            find_similar_photos(vec![root.to_string_lossy().into_owned()], paths).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].paths.len(), 2);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn differing_bit_positions_are_counted() {
        let left = 9_u64;
        let right = 14_u64;
        assert_eq!((left ^ right).count_ones(), 3);
    }
}
