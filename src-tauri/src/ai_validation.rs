//! Phase 3 local-AI readiness/self-check.
//! This reports configuration state only; it does not scan user photos or run inference.
use serde::Serialize;

#[derive(Serialize)]
pub struct AiBundleReadiness {
    ready: bool,
    missing: Vec<String>,
    incompatible: Vec<String>,
}

#[derive(Serialize)]
pub struct AiValidationReport {
    semantic: AiBundleReadiness,
    people: AiBundleReadiness,
    ocr: AiBundleReadiness,
    directml_available: bool,
    cpu_fallback: bool,
    core_implementation_complete: bool,
    real_world_validation_required: bool,
}

fn evaluate_bundle(models: &[crate::ai_models::ModelInfo], kinds: &[&str]) -> AiBundleReadiness {
    let mut missing = Vec::new();
    let mut incompatible = Vec::new();
    for kind in kinds {
        match models.iter().find(|model| model.kind == *kind) {
            Some(model) if !model.installed => missing.push((*kind).to_string()),
            Some(model) if !model.compatible => incompatible.push((*kind).to_string()),
            Some(_) => {}
            None => missing.push((*kind).to_string()),
        }
    }
    AiBundleReadiness {
        ready: missing.is_empty() && incompatible.is_empty(),
        missing,
        incompatible,
    }
}

#[tauri::command]
pub fn ai_validation_report() -> Result<AiValidationReport, String> {
    let models = crate::ai_models::model_inventory()?;
    Ok(AiValidationReport {
        semantic: evaluate_bundle(
            &models,
            &["semantic", "semantic_text", "semantic_tokenizer"],
        ),
        people: evaluate_bundle(&models, &["faces", "face_embedder"]),
        ocr: evaluate_bundle(&models, &["ocr", "ocr_recognizer"]),
        directml_available: crate::ai_runtime::directml_available(),
        cpu_fallback: true,
        core_implementation_complete: true,
        real_world_validation_required: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(kind: &str, installed: bool, compatible: bool) -> crate::ai_models::ModelInfo {
        crate::ai_models::ModelInfo {
            kind: kind.into(),
            installed,
            bytes: if installed { 1 } else { 0 },
            sha256: installed.then(|| "a".repeat(64)),
            expected_sha256: "a".repeat(64),
            compatible,
        }
    }

    #[test]
    fn bundle_is_ready_only_when_every_slot_is_verified() {
        let models = vec![model("a", true, true), model("b", true, true)];
        let status = evaluate_bundle(&models, &["a", "b"]);
        assert!(status.ready);
        assert!(status.missing.is_empty());
        assert!(status.incompatible.is_empty());
    }

    #[test]
    fn bundle_reports_missing_and_incompatible_slots_separately() {
        let models = vec![model("a", false, false), model("b", true, false)];
        let status = evaluate_bundle(&models, &["a", "b"]);
        assert!(!status.ready);
        assert_eq!(status.missing, vec!["a"]);
        assert_eq!(status.incompatible, vec!["b"]);
    }
}
