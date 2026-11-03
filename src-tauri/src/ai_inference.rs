//! CPU-first local ONNX inference for the supported semantic image model.
use ort::session::Session;
use serde::Serialize;

#[derive(Serialize)]
pub struct InferenceResult {
    pub(crate) output_shape: Vec<i64>,
    pub(crate) values: Vec<f32>,
}

pub(crate) fn semantic_image_session(device: Option<&str>) -> Result<Session, String> {
    crate::ai_models::validate_supported_clip_vision()?;
    let model = crate::ai_models::installed_model_path("semantic")?;
    let environment = crate::ai_runtime::environment()?;
    let builder = Session::builder(&environment).map_err(|e| e.to_string())?;
    let mut builder = crate::ai_runtime::configure_session(builder, device)?;
    let session = builder
        .commit_from_file(&model)
        .map_err(|e| format!("Cannot load semantic ONNX model: {e}"))?;
    if session.inputs().len() != 1 {
        return Err("Semantic model must expose exactly one image input".into());
    }
    Ok(session)
}

pub(crate) fn run_semantic_image_with_session(
    session: &mut Session,
    root: &str,
    path: &str,
) -> Result<InferenceResult, String> {
    let values = crate::ai_preprocess::prepare_tensor(root, path)?;
    run_semantic_tensor(session, values)
}
pub(crate) fn run_semantic_tensor(
    session: &mut Session,
    values: Vec<f32>,
) -> Result<InferenceResult, String> {
    let tensor = ort::value::Tensor::from_array(([1usize, 3, 224, 224], values.into_boxed_slice()))
        .map_err(|e| e.to_string())?;
    let outputs = session
        .run(ort::inputs![tensor])
        .map_err(|e| format!("Semantic inference failed: {e}"))?;
    if outputs.len() == 0 {
        return Err("Semantic model returned no outputs".into());
    }
    let (shape, data) = outputs[0]
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("Semantic model output is not a float tensor: {e}"))?;
    if data.len() != 512 {
        return Err("Supported OpenCLIP vision model must return a 512-value embedding".into());
    }
    let mut values = data.to_vec();
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    if !norm.is_finite() || norm == 0.0 {
        return Err("Semantic model returned an invalid embedding".into());
    }
    for value in &mut values {
        *value /= norm;
    }
    Ok(InferenceResult {
        output_shape: shape.iter().copied().collect(),
        values,
    })
}

/// Run one installed semantic ONNX model against one library image.
pub(crate) fn run_semantic_image(
    root: String,
    path: String,
    device: Option<String>,
) -> Result<InferenceResult, String> {
    let mut session = semantic_image_session(device.as_deref())?;
    run_semantic_image_with_session(&mut session, &root, &path)
}

#[tauri::command]
pub fn ai_run_semantic_image(
    root: String,
    path: String,
    device: Option<String>,
) -> Result<InferenceResult, String> {
    run_semantic_image(root, path, device)
}
