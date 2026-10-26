//! Local OpenCLIP text tokenization and ONNX inference.
use serde::Serialize;

const CLIP_SEQUENCE_LENGTH: usize = 77;
const CLIP_PAD_EOS_ID: i64 = 49407;

#[derive(Serialize)]
pub struct TextInferenceResult {
    tokens: usize,
    values: Vec<f32>,
}

pub(crate) fn run_semantic_text(query: &str, device: Option<&str>) -> Result<Vec<f32>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("Enter a semantic search phrase".into());
    }
    if query.chars().count() > 500 {
        return Err("Semantic search text is limited to 500 characters".into());
    }

    crate::ai_models::validate_supported_clip_bundle()?;
    let tokenizer_path = crate::ai_models::installed_model_path("semantic_tokenizer")?;
    let tokenizer = tokenizers::Tokenizer::from_file(tokenizer_path).map_err(|e| e.to_string())?;
    let encoding = tokenizer
        .encode(query, true)
        .map_err(|e| format!("Cannot tokenize semantic query: {e}"))?;

    let mut ids: Vec<i64> = encoding.get_ids().iter().map(|id| i64::from(*id)).collect();
    if ids.len() > CLIP_SEQUENCE_LENGTH {
        ids.truncate(CLIP_SEQUENCE_LENGTH);
        ids[CLIP_SEQUENCE_LENGTH - 1] = CLIP_PAD_EOS_ID;
    }
    let mut attention_mask = vec![1_i64; ids.len()];
    ids.resize(CLIP_SEQUENCE_LENGTH, CLIP_PAD_EOS_ID);
    attention_mask.resize(CLIP_SEQUENCE_LENGTH, 0);

    let environment = ort::init()
        .with_name("Open-Photo")
        .with_telemetry(false)
        .build()
        .map_err(|e| e.to_string())?;
    let model = crate::ai_models::installed_model_path("semantic_text")?;
    let builder = ort::session::Session::builder(&environment).map_err(|e| e.to_string())?;
    let builder = crate::ai_runtime::configure_session(builder, device)?;
    let mut session = builder
        .commit_from_file(&model)
        .map_err(|e| format!("Cannot load semantic text ONNX model: {e}"))?;
    let has_input_ids = session
        .inputs()
        .iter()
        .any(|input| input.name() == "input_ids");
    let has_attention_mask = session
        .inputs()
        .iter()
        .any(|input| input.name() == "attention_mask");
    if !has_input_ids || !has_attention_mask {
        return Err("Supported semantic text model inputs are missing".into());
    }

    let ids_tensor =
        ort::value::Tensor::from_array(([1usize, CLIP_SEQUENCE_LENGTH], ids.into_boxed_slice()))
            .map_err(|e| e.to_string())?;
    let mask_tensor = ort::value::Tensor::from_array((
        [1usize, CLIP_SEQUENCE_LENGTH],
        attention_mask.into_boxed_slice(),
    ))
    .map_err(|e| e.to_string())?;
    let outputs = session
        .run(ort::inputs![
            "input_ids" => ids_tensor,
            "attention_mask" => mask_tensor
        ])
        .map_err(|e| format!("Semantic text inference failed: {e}"))?;
    if outputs.len() == 0 {
        return Err("Semantic text model returned no outputs".into());
    }
    let (_, data) = outputs[0]
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("Semantic text output is not a float tensor: {e}"))?;
    if data.len() != 512 {
        return Err("Supported OpenCLIP text model must return a 512-value embedding".into());
    }

    let mut values = data.to_vec();
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm == 0.0 {
        return Err("Semantic text model returned a zero embedding".into());
    }
    for value in &mut values {
        *value /= norm;
    }
    Ok(values)
}

#[tauri::command]
pub fn ai_run_semantic_text(query: String, device: Option<String>) -> Result<TextInferenceResult, String> {
    let token_count = tokenizers::Tokenizer::from_file(crate::ai_models::installed_model_path(
        "semantic_tokenizer",
    )?)
    .map_err(|e| e.to_string())?
    .encode(query.trim(), true)
    .map_err(|e| e.to_string())?
    .get_ids()
    .len()
    .min(CLIP_SEQUENCE_LENGTH);
    let values = run_semantic_text(&query, device.as_deref())?;
    Ok(TextInferenceResult {
        tokens: token_count,
        values,
    })
}

#[cfg(test)]
mod tests {
    use super::{CLIP_PAD_EOS_ID, CLIP_SEQUENCE_LENGTH};

    #[test]
    fn clip_text_contract_is_fixed() {
        assert_eq!(CLIP_SEQUENCE_LENGTH, 77);
        assert_eq!(CLIP_PAD_EOS_ID, 49407);
    }
}
