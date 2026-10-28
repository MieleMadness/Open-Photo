//! Local AI execution-provider policy.
//! Automatic mode prefers Windows DirectML and always permits CPU fallback.
use ort::ep::{DirectML, ExecutionProvider};
use ort::session::builder::SessionBuilder;
use serde::Serialize;
use std::sync::OnceLock;

pub(crate) fn environment() -> Result<ort::environment::Environment, String> {
    // ORT rc.15 permits one environment per process, including after a session
    // drops. Keep it alive and share it across images, text, People, and OCR.
    static ENVIRONMENT: OnceLock<Result<ort::environment::Environment, String>> = OnceLock::new();
    ENVIRONMENT
        .get_or_init(|| {
            ort::init()
                .with_name("Open-Photo")
                .with_telemetry(false)
                .build()
                .map_err(|e| e.to_string())
        })
        .clone()
}

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
    fn inference_environment_is_reusable_and_shared_across_threads() {
        let threads: Vec<_> = (0..4)
            .map(|_| std::thread::spawn(|| super::environment().map(|_| ())))
            .collect();
        for thread in threads {
            thread.join().unwrap().unwrap();
        }
        super::environment().unwrap();
    }

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
