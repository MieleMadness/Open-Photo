//! CPU-first local ONNX inference for an installed semantic image model.
use serde::Serialize;

#[derive(Serialize)]
pub struct InferenceResult {
    pub(crate) output_shape: Vec<i64>,
    pub(crate) values: Vec<f32>,
}

/// Run one installed semantic ONNX model against one library image.\n/// The model must accept a single float32 tensor shaped [1, 3, 224, 224].\npub(crate) fn run_semantic_image(root: String, path: String) -> Result<InferenceResult, String> {
    let model = crate::ai_models::installed_model_path("semantic")?;
    let values = crate::ai_preprocess::prepare_tensor(&root, &path)?;
    let environment = ort::init()
        .with_name("Open-Photo")
        .with_telemetry(false)
        .build()
        .map_err(|e| e.to_string())?;
    let mut session = ort::session::Session::builder(&environment)
        .map_err(|e| e.to_string())?
        .commit_from_file(&model)
        .map_err(|e| format!("Cannot load semantic ONNX model: {e}"))?;
    if session.inputs().len() != 1 {
        return Err("Semantic model must expose exactly one image input".into());
    }
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
    if data.is_empty() {
        return Err("Semantic model returned an empty tensor".into());
    }
    if data.len() > 8192 {
        return Err("Semantic model output is larger than the current safety limit".into());
    }
    Ok(InferenceResult {
        output_shape: shape.iter().copied().collect(),
        values: data.to_vec(),
    })
}

#[tauri::command]
pub fn ai_run_semantic_image(root: String, path: String) -> Result<InferenceResult, String> {
    run_semantic_image(root, path)
}
