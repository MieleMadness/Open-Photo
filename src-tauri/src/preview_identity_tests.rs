use super::*;
use image::{Rgb, RgbImage};
use std::fs::FileTimes;

#[test]
fn preview_content_identity_survives_warm_and_cold_caches_and_preserved_timestamps() {
    let root = tests::temp_root();
    let file = root.join("identity.bmp");
    let render = |color| RgbImage::from_pixel(4, 4, Rgb(color)).save(&file).unwrap();
    render([255, 0, 0]);
    let root_text = root.to_string_lossy().into_owned();
    let path = file.to_string_lossy().into_owned();
    let before = fs::read(&file).unwrap();
    let modified = file.metadata().unwrap().modified().unwrap();
    let version = source_version::read(&file).unwrap();
    let red =
        preview_image_sync(root_text.clone(), path.clone(), 320, Some(version.clone())).unwrap();
    assert_eq!(
        red,
        preview_image_sync(root_text.clone(), path.clone(), 320, Some(version.clone())).unwrap()
    );
    let key = format!(
        "oriented-v3:{}:{version}:320",
        fs::canonicalize(&file).unwrap().display()
    );
    assert!(cache_disk::read(&key).is_some());
    cache().lock().unwrap().0.remove(&key);
    assert_eq!(
        red,
        preview_image_sync(root_text.clone(), path.clone(), 320, Some(version.clone())).unwrap()
    );
    assert_eq!(fs::read(&file).unwrap(), before);
    render([0, 0, 255]);
    fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_times(FileTimes::new().set_modified(modified))
        .unwrap();
    assert_eq!(file.metadata().unwrap().len(), before.len() as u64);
    let blue_source = fs::read(&file).unwrap();
    assert!(
        preview_image_sync(root_text.clone(), path.clone(), 320, Some(version))
            .unwrap_err()
            .contains("Source changed")
    );
    let blue = preview_image_sync(root_text, path, 320, None).unwrap();
    assert_ne!(red, blue);
    let png = base64::engine::general_purpose::STANDARD
        .decode(blue.split_once(',').unwrap().1)
        .unwrap();
    assert_eq!(
        image::load_from_memory(&png)
            .unwrap()
            .to_rgb8()
            .get_pixel(0, 0)
            .0,
        [0, 0, 255]
    );
    assert_eq!(fs::read(&file).unwrap(), blue_source);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn heic_source_binding_rejects_preserved_metadata_replacement_without_modifying_bytes() {
    let root = tests::temp_root();
    let path = root.join("fixture.heic");
    fs::write(&path, b"old-heic-bytes").unwrap();
    let modified = path.metadata().unwrap().modified().unwrap();
    let version = source_version::read(&path).unwrap();
    fs::write(&path, b"new-heic-bytes").unwrap();
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(modified))
        .unwrap();
    assert!(read_iphone_media_sync(
        root.to_str().unwrap().into(),
        path.to_str().unwrap().into(),
        Some(version)
    )
    .is_err());
    assert_eq!(fs::read(&path).unwrap(), b"new-heic-bytes");
    fs::remove_dir_all(root).unwrap();
}
