//! Offline AI model storage. This module never sends data to a network service.
//! Semantic model slots are validated against one supported OpenCLIP bundle before execution.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_MODEL_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub(crate) const SUPPORTED_CLIP_VISION_SHA256: &str =
    "7e14f76233d0c840c0621b1ef68f5877efe9357850782b1bbaf0c01693f73b43";
pub(crate) const SUPPORTED_CLIP_TEXT_SHA256: &str =
    "252b86e0ef1fc95b22cfd52fbf647142727fdbecc152556ffe0fba0b10a80370";
pub(crate) const SUPPORTED_CLIP_TOKENIZER_SHA256: &str =
    "b556ac8c99757ffb677208af34bc8c6721572114111a6e0aaf5fa69ff0b8d842";
pub(crate) const SUPPORTED_YUNET_SHA256: &str =
    "ebafce4e3c118d6554634be5c27ab333b4c047a9a8c3faf1d7cf93101c22f0f0";
pub(crate) const SUPPORTED_SFACE_SHA256: &str =
    "0ba9fbfa01b5270c96627c4ef784da859931e02f04419c829e83484087c34e79";

pub(crate) fn model_directory() -> Result<PathBuf, String> {
    let dir = dirs::data_local_dir()
        .ok_or("Local application data folder unavailable")?
        .join("Open-Photo")
        .join("ai-models");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}
pub(crate) fn model_name(kind: &str) -> Result<&'static str, String> {
    match kind {
        "semantic" => Ok("semantic.onnx"),
        "semantic_text" => Ok("semantic-text.onnx"),
        "semantic_tokenizer" => Ok("semantic-tokenizer.json"),
        "faces" => Ok("faces-yunet.onnx"),
        "face_embedder" => Ok("faces-sface.onnx"),
        "ocr" => Ok("ocr.onnx"),
        _ => Err("Unknown model type".into()),
    }
}
#[derive(Serialize)]
pub struct ModelInfo {
    kind: String,
    installed: bool,
    bytes: u64,
    sha256: Option<String>,
}

fn fingerprint(path: &Path) -> Result<(u64, String), String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut bytes = 0_u64;
    let mut block = [0_u8; 65536];
    loop {
        let read = file.read(&mut block).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        bytes += read as u64;
        if bytes > MAX_MODEL_BYTES {
            return Err("Model exceeds the 2 GB size limit".into());
        }
        hash.update(&block[..read]);
    }
    if bytes == 0 {
        return Err("Empty model file".into());
    }
    Ok((bytes, format!("{:x}", hash.finalize())))
}
pub(crate) fn installed_model_path(kind: &str) -> Result<PathBuf, String> {
    let path = model_directory()?.join(model_name(kind)?);
    if !path.is_file() {
        return Err(format!("{kind} model is not installed"));
    }
    Ok(path)
}

pub(crate) fn installed_model_sha256(kind: &str) -> Result<String, String> {
    let path = installed_model_path(kind)?;
    fingerprint(&path).map(|(_, sha)| sha)
}

pub(crate) fn validate_supported_clip_bundle() -> Result<String, String> {
    let vision = installed_model_sha256("semantic")?;
    let text = installed_model_sha256("semantic_text")?;
    let tokenizer = installed_model_sha256("semantic_tokenizer")?;
    if vision != SUPPORTED_CLIP_VISION_SHA256
        || text != SUPPORTED_CLIP_TEXT_SHA256
        || tokenizer != SUPPORTED_CLIP_TOKENIZER_SHA256
    {
        return Err(
            "Semantic search requires the supported OpenCLIP ViT-B/32 vision model, text model, and tokenizer bundle"
                .into(),
        );
    }
    Ok(format!("openclip-vit-b32-v1:{vision}"))
}

pub(crate) fn validate_supported_clip_vision() -> Result<String, String> {
    let vision = installed_model_sha256("semantic")?;
    if vision != SUPPORTED_CLIP_VISION_SHA256 {
        return Err(
            "Semantic image indexing requires the supported OpenCLIP ViT-B/32 vision model".into(),
        );
    }
    Ok(format!("openclip-vit-b32-v1:{vision}"))
}

pub(crate) fn validate_supported_face_bundle() -> Result<(String, String), String> {
    let detector = installed_model_sha256("faces")?;
    let embedder = installed_model_sha256("face_embedder")?;
    if detector != SUPPORTED_YUNET_SHA256 || embedder != SUPPORTED_SFACE_SHA256 {
        return Err(
            "People indexing requires the supported YuNet 2026may detector and SFace 2021dec embedding model"
                .into(),
        );
    }
    Ok((
        format!("yunet-2026may:{detector}"),
        format!("sface-2021dec:{embedder}"),
    ))
}

#[tauri::command]
pub fn ai_model_status() -> Result<Vec<ModelInfo>, String> {
    let dir = model_directory()?;
    [
        "semantic",
        "semantic_text",
        "semantic_tokenizer",
        "faces",
        "face_embedder",
        "ocr",
    ]
    .iter()
    .map(|kind| {
        let file = dir.join(model_name(kind)?);
        if !file.exists() {
            return Ok(ModelInfo {
                kind: (*kind).into(),
                installed: false,
                bytes: 0,
                sha256: None,
            });
        }
        let (bytes, sha) = fingerprint(&file)?;
        Ok(ModelInfo {
            kind: (*kind).into(),
            installed: true,
            bytes,
            sha256: Some(sha),
        })
    })
    .collect()
}
#[tauri::command]
pub fn ai_import_model(
    kind: String,
    path: String,
    expected_sha256: String,
) -> Result<ModelInfo, String> {
    let name = model_name(&kind)?;
    if expected_sha256.len() != 64 || !expected_sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("A 64-character SHA-256 checksum is required to verify a model".into());
    }
    let source = fs::canonicalize(&path).map_err(|e| e.to_string())?;
    let expected_extension = if kind == "semantic_tokenizer" {
        "json"
    } else {
        "onnx"
    };
    if !source.is_file()
        || source
            .extension()
            .and_then(|v| v.to_str())
            .is_none_or(|v| !v.eq_ignore_ascii_case(expected_extension))
    {
        return Err(format!(
            "Choose a valid local .{expected_extension} file for this AI slot"
        ));
    }
    let (size, sha) = fingerprint(&source)?;
    let supported_sha = match kind.as_str() {
        "semantic" => Some(SUPPORTED_CLIP_VISION_SHA256),
        "semantic_text" => Some(SUPPORTED_CLIP_TEXT_SHA256),
        "semantic_tokenizer" => Some(SUPPORTED_CLIP_TOKENIZER_SHA256),
        "faces" => Some(SUPPORTED_YUNET_SHA256),
        "face_embedder" => Some(SUPPORTED_SFACE_SHA256),
        _ => None,
    };
    if supported_sha.is_some_and(|required| sha != required) {
        return Err(match kind.as_str() {
            "faces" => "The People detector slot only accepts YuNet 2026may from OpenCV Zoo".into(),
            "face_embedder" => {
                "The People embedding slot only accepts SFace 2021dec from OpenCV Zoo".into()
            }
            _ => "This semantic slot only accepts the supported OpenCLIP ViT-B/32 bundle asset"
                .into(),
        });
    }
    if !sha.eq_ignore_ascii_case(&expected_sha256) {
        return Err("Model checksum mismatch: nothing was installed".into());
    }
    let dir = model_directory()?;
    let destination = dir.join(name);
    if destination.exists() {
        return Err(
            "This model slot is already installed; existing models are never overwritten".into(),
        );
    }
    let mut source_file = fs::File::open(source).map_err(|e| e.to_string())?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .map_err(|e| e.to_string())?;
    let copied = std::io::copy(&mut source_file, &mut output).map_err(|e| e.to_string());
    let synced = output.flush().and_then(|_| output.sync_all());
    drop(output);
    if copied.as_ref().is_err() || synced.is_err() {
        let _ = fs::remove_file(destination);
        return Err("Unable to install model".into());
    }
    let (written, digest) = fingerprint(&destination)?;
    if written != size || digest != sha {
        let _ = fs::remove_file(destination);
        return Err("Installed model verification failed".into());
    }
    Ok(ModelInfo {
        kind,
        installed: true,
        bytes: written,
        sha256: Some(digest),
    })
}
/// Remove only a known model slot under the private application-data folder.
/// Never touches user photos or arbitrary input paths.
#[tauri::command]
pub fn ai_remove_model(kind: String) -> Result<bool, String> {
    let name = model_name(&kind)?;
    let destination = model_directory()?.join(name);
    match fs::remove_file(&destination) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_model_type_is_rejected() {
        assert!(model_name("../escape").is_err());
    }
    #[test]
    fn empty_model_is_rejected() {
        let path = std::env::temp_dir().join(format!(
            "openphoto-test-empty-model-{}.onnx",
            std::process::id()
        ));
        fs::write(&path, []).unwrap();
        assert!(fingerprint(&path).is_err());
        fs::remove_file(path).unwrap();
    }
}
