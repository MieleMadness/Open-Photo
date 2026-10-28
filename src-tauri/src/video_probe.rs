//! Read-only media inspection, preserving exact frame-rate and timestamp ratios.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{path::Path, time::Duration};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VideoInfo {
    pub duration_seconds: f64,
    pub video_stream: usize,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub sample_aspect_ratio: Option<String>,
    pub frame_rate: Option<String>,
    pub nominal_frame_rate: Option<String>,
    pub time_base: Option<String>,
    pub start_time: Option<String>,
    pub format_start_time: Option<String>,
    pub rotation_degrees: f64,
    pub pixel_format: Option<String>,
    pub color_space: Option<String>,
    pub color_primaries: Option<String>,
    pub color_transfer: Option<String>,
    pub hdr: bool,
    pub frame_rate_may_vary: bool,
    pub audio: Vec<AudioInfo>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AudioInfo {
    pub stream: usize,
    pub codec: String,
    pub sample_rate: Option<String>,
    pub channels: Option<u32>,
    pub channel_layout: Option<String>,
    pub start_time: Option<String>,
    pub duration_seconds: Option<f64>,
}

fn string(value: &Value, key: &str) -> Option<String> {
    value[key]
        .as_str()
        .filter(|s| !s.is_empty() && *s != "unknown" && *s != "N/A")
        .map(str::to_owned)
}
fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|n| n.is_finite())
}
fn ratio(value: Option<String>) -> Option<String> {
    value.filter(|s| {
        let Some((a, b)) = s.split_once('/') else {
            return false;
        };
        matches!((a.parse::<u64>(),b.parse::<u64>()),(Ok(a),Ok(b))if a>0&&b>0)
    })
}
fn ratio_value(value: &str) -> Option<f64> {
    let (a, b) = value.split_once('/')?;
    Some(a.parse::<f64>().ok()? / b.parse::<f64>().ok()?)
}

pub(crate) fn parse_probe(bytes: &[u8]) -> Result<VideoInfo, String> {
    let doc: Value = serde_json::from_slice(bytes)
        .map_err(|e| format!("Invalid video inspection response: {e}"))?;
    let streams = doc["streams"]
        .as_array()
        .ok_or("Video inspection did not report streams")?;
    let video = streams
        .iter()
        .find(|s| {
            s["codec_type"] == "video"
                && s["disposition"]["attached_pic"].as_u64().unwrap_or(0) == 0
        })
        .ok_or("No playable video stream found")?;
    let width = video["width"]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v > 0)
        .ok_or("Video width unavailable")?;
    let height = video["height"]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v > 0)
        .ok_or("Video height unavailable")?;
    let duration = number(&doc["format"]["duration"])
        .or_else(|| number(&video["duration"]))
        .filter(|v| *v > 0.0)
        .ok_or("Video duration unavailable")?;
    let frame_rate = ratio(string(video, "avg_frame_rate"));
    let nominal_frame_rate = ratio(string(video, "r_frame_rate"));
    let frame_rate_may_vary = frame_rate
        .as_deref()
        .zip(nominal_frame_rate.as_deref())
        .is_some_and(|(a, b)| {
            ratio_value(a)
                .zip(ratio_value(b))
                .is_some_and(|(a, b)| (a - b).abs() > 0.01)
        });
    let rotation = video["side_data_list"]
        .as_array()
        .and_then(|data| data.iter().find_map(|s| number(&s["rotation"])))
        .or_else(|| number(&video["tags"]["rotate"]))
        .unwrap_or(0.0);
    let transfer = string(video, "color_transfer");
    let primaries = string(video, "color_primaries");
    let hdr = matches!(transfer.as_deref(), Some("smpte2084" | "arib-std-b67"))
        || video["side_data_list"].as_array().is_some_and(|data| {
            data.iter().any(|s| {
                s["side_data_type"].as_str().is_some_and(|kind| {
                    kind.contains("DOVI")
                        || kind.contains("Mastering display")
                        || kind.contains("HDR")
                })
            })
        });
    let audio = streams
        .iter()
        .filter(|s| s["codec_type"] == "audio")
        .map(|s| AudioInfo {
            stream: s["index"].as_u64().unwrap_or(0) as usize,
            codec: string(s, "codec_name").unwrap_or_else(|| "unknown".into()),
            sample_rate: string(s, "sample_rate"),
            channels: s["channels"].as_u64().and_then(|v| u32::try_from(v).ok()),
            channel_layout: string(s, "channel_layout"),
            start_time: string(s, "start_time"),
            duration_seconds: number(&s["duration"]),
        })
        .collect::<Vec<_>>();
    let mut warnings = Vec::new();
    if hdr {
        warnings.push("HDR detected. SDR editing requires a tested tone-mapping workflow.".into());
    }
    if frame_rate_may_vary {
        warnings.push("Frame timing may vary; nominal and average rates differ.".into());
    }
    if audio.len() > 1 {
        warnings
            .push("Multiple audio tracks detected; export must choose a track explicitly.".into());
    }
    if audio.is_empty() {
        warnings.push("This video has no audio track.".into());
    }
    Ok(VideoInfo {
        duration_seconds: duration,
        video_stream: video["index"].as_u64().unwrap_or(0) as usize,
        codec: string(video, "codec_name").unwrap_or_else(|| "unknown".into()),
        width,
        height,
        sample_aspect_ratio: string(video, "sample_aspect_ratio"),
        frame_rate,
        nominal_frame_rate,
        time_base: ratio(string(video, "time_base")),
        start_time: string(video, "start_time"),
        format_start_time: string(&doc["format"], "start_time"),
        rotation_degrees: rotation,
        pixel_format: string(video, "pix_fmt"),
        color_space: string(video, "color_space"),
        color_primaries: primaries,
        color_transfer: transfer,
        hdr,
        frame_rate_may_vary,
        audio,
        warnings,
    })
}

pub(crate) fn probe(
    directory: &Path,
    source: &Path,
    cancel: impl Fn() -> bool,
) -> Result<VideoInfo, String> {
    let tool = crate::video_engine::tool(directory, "ffprobe")?;
    let args: Vec<&std::ffi::OsStr> = vec![
        "-v".as_ref(),
        "error".as_ref(),
        "-max_alloc".as_ref(),
        "268435456".as_ref(),
        "-protocol_whitelist".as_ref(),
        "file,pipe".as_ref(),
        "-format_whitelist".as_ref(),
        "mov,matroska,webm,avi".as_ref(),
        "-probesize".as_ref(),
        "10000000".as_ref(),
        "-analyzeduration".as_ref(),
        "10000000".as_ref(),
        "-show_streams".as_ref(),
        "-show_format".as_ref(),
        "-of".as_ref(),
        "json".as_ref(),
        source.as_os_str(),
    ];
    parse_probe(&crate::video_engine::run_until(
        &tool,
        &args,
        Duration::from_secs(30),
        cancel,
    )?)
}

pub struct InspectionJobs {
    gate: std::sync::Arc<tokio::sync::Semaphore>,
    generation: std::sync::Arc<std::sync::atomic::AtomicU64>,
}
impl Default for InspectionJobs {
    fn default() -> Self {
        Self {
            gate: std::sync::Arc::new(tokio::sync::Semaphore::new(1)),
            generation: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }
}

#[tauri::command]
pub async fn inspect_video(
    app: tauri::AppHandle,
    jobs: tauri::State<'_, InspectionJobs>,
    root: String,
    path: String,
) -> Result<VideoInfo, String> {
    let source = crate::safe_media_path(&root, &path)?;
    if !matches!(
        source
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "mov" | "mp4" | "m4v" | "avi" | "mkv" | "webm"
    ) {
        return Err("Select a supported video file".into());
    }
    let directory = crate::video_engine::directory(&app)?;
    use std::sync::atomic::Ordering;
    let generation = jobs.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let latest = jobs.generation.clone();
    let permit = jobs
        .gate
        .clone()
        .acquire_owned()
        .await
        .map_err(|e| e.to_string())?;
    if latest.load(Ordering::SeqCst) != generation {
        return Err("Video inspection superseded".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        probe(&directory, &source, || {
            latest.load(Ordering::SeqCst) != generation
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Value {
        serde_json::json!({"streams":[{"index":0,"codec_type":"video","codec_name":"h264","width":1920,"height":1080,"avg_frame_rate":"30000/1001","r_frame_rate":"30000/1001","time_base":"1/90000","side_data_list":[{"rotation":-90}]}],"format":{"duration":"12.5"}})
    }
    #[test]
    fn preserves_rational_rates_rotation_and_silent_video() {
        let result = parse_probe(&serde_json::to_vec(&fixture()).unwrap()).unwrap();
        assert_eq!(result.frame_rate.as_deref(), Some("30000/1001"));
        assert_eq!(result.rotation_degrees, -90.0);
        assert_eq!(result.duration_seconds, 12.5);
        assert!(result.audio.is_empty());
        assert!(!result.frame_rate_may_vary);
    }
    #[test]
    fn detects_hdr_variable_timing_and_multiple_audio_tracks() {
        let mut f = fixture();
        f["streams"][0]["color_transfer"] = "smpte2084".into();
        f["streams"][0]["r_frame_rate"] = "60/1".into();
        f["streams"].as_array_mut().unwrap().extend([
            serde_json::json!({"index":1,"codec_type":"audio","codec_name":"aac","channels":2}),
            serde_json::json!({"index":2,"codec_type":"audio","codec_name":"aac"}),
        ]);
        let result = parse_probe(&serde_json::to_vec(&f).unwrap()).unwrap();
        assert!(result.hdr);
        assert!(result.frame_rate_may_vary);
        assert_eq!(result.audio.len(), 2);
    }
    #[test]
    fn rejects_broken_duration_and_ignores_attached_cover_art() {
        let mut f = fixture();
        f["format"]["duration"] = "NaN".into();
        assert!(parse_probe(&serde_json::to_vec(&f).unwrap()).is_err());
        let mut f = fixture();
        f["streams"][0]["disposition"] = serde_json::json!({"attached_pic":1});
        assert!(parse_probe(&serde_json::to_vec(&f).unwrap()).is_err());
        assert!(parse_probe(b"broken").is_err());
    }
}
