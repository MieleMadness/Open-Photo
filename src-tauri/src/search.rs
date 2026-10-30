//! Read-only discovery providers. Ranking and current-library intersection live in unifiedSearch.ts.
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

pub(crate) const MAX_SCAN: usize = 50_000;
pub(crate) const MAX_MATCHES: usize = 4096;
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Ocr,
    Person,
    Semantic,
}
#[derive(Clone, Debug, Serialize)]
pub struct Evidence {
    pub root: String,
    pub path: String,
    pub modified_unix: i64,
    pub kind: Kind,
    pub text: String,
    pub matched_terms: Vec<String>,
    pub score: f32,
    pub timestamp_seconds: Option<f64>,
}
#[derive(Default)]
pub struct Batch {
    pub evidence: Vec<Evidence>,
    pub truncated: bool,
}
#[derive(Default, Serialize)]
pub struct Response {
    pub evidence: Vec<Evidence>,
    pub warnings: Vec<String>,
    pub cancelled: bool,
}
#[derive(Default)]
pub struct SearchState {
    requests: Arc<Mutex<Requests>>,
    gate: Arc<tokio::sync::Mutex<()>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    request_id: String,
    workspace_id: String,
    query: String,
    terms: Vec<String>,
    person_query: String,
    include_ai: bool,
    include_semantic: bool,
    device: Option<String>,
}
#[derive(Default)]
struct Requests {
    serial: u64,
    active: String,
    cancelled: VecDeque<String>,
}
impl Requests {
    fn begin(&mut self, id: &str) -> Option<u64> {
        if self.cancelled.iter().any(|old| old == id) {
            return None;
        }
        self.serial = self.serial.wrapping_add(1);
        self.active = id.into();
        Some(self.serial)
    }
    fn cancel(&mut self, id: &str) {
        self.cancelled.push_back(id.into());
        while self.cancelled.len() > 64 {
            self.cancelled.pop_front();
        }
        if self.active == id {
            self.serial = self.serial.wrapping_add(1);
            self.active.clear();
        }
    }
    fn current(&self, id: &str, serial: u64) -> bool {
        self.active == id && self.serial == serial
    }
}
pub(crate) fn terms(value: &str) -> Vec<String> {
    value.split_whitespace().map(str::to_lowercase).collect()
}
pub(crate) fn excerpt(value: &str, query: &[String]) -> String {
    let chars: Vec<char> = value.chars().take(16_384).collect();
    let lower: Vec<char> = chars.iter().flat_map(|c| c.to_lowercase()).collect();
    let needle: Vec<char> = query.first().map_or("", String::as_str).chars().collect();
    let start = if needle.is_empty() {
        0
    } else {
        lower
            .windows(needle.len())
            .position(|w| w == needle)
            .unwrap_or(0)
            .saturating_sub(40)
    };
    chars
        .iter()
        .skip(start)
        .take(400)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn append(response: &mut Response, name: &str, result: Result<Batch, String>) {
    match result {
        Ok(batch) => {
            response.evidence.extend(batch.evidence);
            if batch.truncated {
                response.warnings.push(format!(
                    "{name} reached its bounded search limit; some indexed matches may be omitted."
                ));
            }
        }
        Err(error) => response
            .warnings
            .push(format!("{name} is unavailable: {error}")),
    }
}
#[tauri::command]
pub fn cancel_workspace_search(state: tauri::State<'_, SearchState>, request_id: String) {
    if request_id.len() <= 120 {
        if let Ok(mut requests) = state.requests.lock() {
            requests.cancel(&request_id);
        }
    }
}
#[tauri::command]
pub async fn search_workspace_evidence(
    state: tauri::State<'_, SearchState>,
    request: Request,
) -> Result<Response, String> {
    let Request {
        request_id,
        workspace_id,
        query,
        terms,
        person_query,
        include_ai,
        include_semantic,
        device,
    } = request;
    if request_id.is_empty()
        || request_id.len() > 120
        || query.chars().count() > 500
        || person_query.chars().count() > 200
        || terms.len() > 50
        || terms.iter().any(|term| term.chars().count() > 500)
    {
        return Err("Search request is invalid or too long".into());
    }
    let requests = state.requests.clone();
    let Some(serial) = requests
        .lock()
        .map_err(|e| e.to_string())?
        .begin(&request_id)
    else {
        return Ok(Response {
            cancelled: true,
            ..Default::default()
        });
    };
    let gate = state.gate.clone();
    let permit = gate.lock_owned().await;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        let cancelled = || {
            requests
                .lock()
                .map_or(true, |state| !state.current(&request_id, serial))
        };
        if cancelled() {
            return Ok(Response {
                cancelled: true,
                ..Default::default()
            });
        }
        let roots = crate::user_data::workspace_roots(&workspace_id)?;
        let mut response = Response::default();
        if !include_ai || roots.is_empty() {
            return Ok(response);
        }
        if !query.trim().is_empty() {
            append(
                &mut response,
                "Indexed photo text",
                crate::search_providers::ocr(&roots, &terms, cancelled),
            );
        }
        if cancelled() {
            return Ok(Response {
                cancelled: true,
                ..Default::default()
            });
        }
        let name = if person_query.trim().is_empty() {
            &query
        } else {
            &person_query
        };
        if !name.trim().is_empty() {
            append(
                &mut response,
                "Named People",
                crate::search_providers::people(&roots, name, cancelled),
            );
        }
        if cancelled() {
            return Ok(Response {
                cancelled: true,
                ..Default::default()
            });
        }
        if include_semantic && !query.trim().is_empty() {
            // Inference validates the pinned bundle once, rather than repeating it for every root.
            let result = crate::ai_text::run_semantic_text(&query, device.as_deref())
                .and_then(|vector| crate::search_providers::semantic(&roots, &vector, cancelled));
            append(&mut response, "Image content search", result);
        }
        if cancelled() {
            Ok(Response {
                cancelled: true,
                ..Default::default()
            })
        } else {
            Ok(response)
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn request_contract_matches_the_frontend_and_rejects_unknown_fields() {
        let value = serde_json::json!({"requestId":"search-id","workspaceId":"workspace","query":"burger","terms":["burger"],"personQuery":"","includeAi":true,"includeSemantic":false,"device":"cpu"});
        let request: Request = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(request.workspace_id, "workspace");
        assert_eq!(request.terms, vec!["burger"]);
        let mut bad = value;
        bad["root"] = serde_json::json!("arbitrary-folder");
        assert!(serde_json::from_value::<Request>(bad).is_err());
    }
    #[test]
    fn an_old_cancel_does_not_stop_a_new_query_and_prestart_cancel_is_honored() {
        let mut requests = Requests::default();
        let old = requests.begin("old").unwrap();
        let new = requests.begin("new").unwrap();
        requests.cancel("old");
        assert!(requests.current("new", new));
        assert!(!requests.current("old", old));
        requests.cancel("not-started");
        assert!(requests.begin("not-started").is_none());
        requests.cancel("new");
        assert!(!requests.current("new", new));
    }
    #[test]
    fn excerpts_are_bounded_unicode_safe_and_cancel_history_is_bounded() {
        let text = "世界".repeat(10000) + " burger";
        assert!(excerpt(&text, &terms("burger")).chars().count() <= 400);
        let mut requests = Requests::default();
        for index in 0..100 {
            requests.cancel(&index.to_string());
        }
        assert_eq!(requests.cancelled.len(), 64);
    }
}
