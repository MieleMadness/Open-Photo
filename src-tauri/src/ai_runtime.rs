//! Local AI execution-provider policy.
//! Automatic mode prefers Windows DirectML and always permits CPU fallback.
use ort::ep::{DirectML, ExecutionProvider};
use ort::session::builder::SessionBuilder;
use serde::Serialize;

#[derive(Serialize)]
pub struct AiRuntimeStatus {
    directml_compiled: bool,
    directml_available: bool,
    automatic_policy: &'static str,
    cpu_fallback: bool,
}

pub(crate) fn normalized_device(device: Option<&str>) -> Result<&'static str, String> {
    match device.unwrap_or("auto") {
        "auto" => Ok("auto"),
        "cpu" => Ok("cpu"),
        _ => Err("AI processing preference must be 'auto' or 'cpu'".into()),
    }
}

pub(crate) fn configure_session(
    builder: SessionBuilder,
    device: Option<&str>,
) -> Result<SessionBuilder, String> {
    match normalized_device(device)? {
        "cpu" => Ok(builder),
        "auto" => builder
            .with_execution_providers([DirectML::default().build().fail_silently()])
            .map_err(|e| format!("Unable to configure DirectML execution provider: {e}")),
        _ => unreachable!(),
    }
}

pub(crate) fn directml_available() -> bool {
    DirectML::is_available().unwrap_or(false)
}

#[tauri::command]
pub fn ai_runtime_status() -> AiRuntimeStatus {
    let directml_available = directml_available();
    AiRuntimeStatus {
        directml_compiled: true,
        directml_available,
        automatic_policy: "DirectML first, CPU fallback",
        cpu_fallback: true,
    }
}

#[cfg(test)]
mod tests {
    use super::normalized_device;

    #[test]
    fn accepts_known_device_preferences() {
        assert_eq!(normalized_device(None).unwrap(), "auto");
        assert_eq!(normalized_device(Some("auto")).unwrap(), "auto");
        assert_eq!(normalized_device(Some("cpu")).unwrap(), "cpu");
    }

    #[test]
    fn rejects_unknown_device_preferences() {
        assert!(normalized_device(Some("cuda")).is_err());
    }
}
