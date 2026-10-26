//! Offline AI model storage. This module never sends data to a network service.
//! An installed model is not an active inference engine; execution is a separate milestone.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_MODEL_BYTES: u64 = 2 * 1024 * 1024 * 1024;

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
        "faces" => Ok("faces.onnx"),
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

#[tauri::command]
pub fn ai_model_status() -> Result<Vec<ModelInfo>, String> {
    let dir = model_directory()?;
    ["semantic", "faces", "ocr"]
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
    if !source.is_file()
        || source
            .extension()
            .and_then(|v| v.to_str())
            .is_none_or(|v| !v.eq_ignore_ascii_case("onnx"))
    {
        return Err("Choose a valid local ONNX model file".into());
    }
    let (size, sha) = fingerprint(&source)?;
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
