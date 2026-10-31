//! Bounded still previews for videos; video bytes never cross IPC.
use crate::{cache_disk, media_stream, video_engine, video_probe};
use base64::Engine;
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::Path,
    sync::Arc,
    time::{Duration, UNIX_EPOCH},
};

pub struct PosterJobs {
    library: Arc<tokio::sync::Semaphore>,
    player: Arc<tokio::sync::Semaphore>,
}
impl Default for PosterJobs {
    fn default() -> Self {
        Self {
            library: Arc::new(tokio::sync::Semaphore::new(1)),
            player: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }
}
fn signature(source: &Path) -> Result<(u64, u128), String> {
    let m = fs::metadata(source).map_err(|e| e.to_string())?;
    Ok((
        m.len(),
        m.modified()
            .map_err(|e| e.to_string())?
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos(),
    ))
}
pub(crate) fn poster(
    engine: &Path,
    root: &str,
    path: &str,
    max_edge: u32,
) -> Result<String, String> {
    frame_at(engine, root, path, max_edge, None)
}

fn frame_at(
    engine: &Path,
    root: &str,
    path: &str,
    max_edge: u32,
    requested: Option<f64>,
) -> Result<String, String> {
    let source = media_stream::validated_video(root, path)?;
    let before = signature(&source)?;
    let edge = max_edge.clamp(64, 640);
    if requested.is_some_and(|s| !s.is_finite() || s < 0.0 || s > 7200.0) {
        return Err("Invalid filmstrip timestamp".into());
    }
    let time_key = requested
        .map(|s| format!("{s:.3}"))
        .unwrap_or_else(|| "poster".into());
    let key = format!(
        "video-frame-v2:{}:{}:{}:{}:{edge}:{time_key}",
        video_engine::VERSION,
        source.display(),
        before.0,
        before.1
    );
    let bytes = if let Some(bytes) = cache_disk::read(&key) {
        bytes
    } else {
        let info = video_probe::probe(engine, &source, || false)?;
        if info.hdr {
            return Err("HDR video posters require tested tone mapping".into());
        }
        if info.duration_seconds > 7200.0
            || u64::from(info.width) * u64::from(info.height) > 100_000_000
        {
            return Err("Video poster exceeds duration or frame-size limits".into());
        }
        let seconds = requested
            .unwrap_or((info.duration_seconds * 0.1).min(5.0))
            .min((info.duration_seconds - 0.001).max(0.0));
        let args: Vec<OsString> = [
            "-nostdin",
            "-v",
            "error",
            "-xerror",
            "-threads",
            "2",
            "-filter_threads",
            "1",
            "-max_alloc",
            "268435456",
            "-protocol_whitelist",
            "file,pipe",
            "-format_whitelist",
            "mov,matroska,webm,avi",
            "-ss",
        ]
        .into_iter()
        .map(Into::into)
        .chain([
            format!("{seconds:.6}").into(),
            "-i".into(),
            source.clone().into(),
            "-map".into(),
            format!("0:{}", info.video_stream).into(),
            "-an".into(),
            "-sn".into(),
            "-dn".into(),
            "-frames:v".into(),
            "1".into(),
            "-vf".into(),
            format!("scale={edge}:{edge}:force_original_aspect_ratio=decrease:force_divisible_by=2:reset_sar=1").into(),
            "-pix_fmt".into(),
            "rgb24".into(),
            "-c:v".into(),
            "png".into(),
            "-threads:v".into(),
            "1".into(),
            "-f".into(),
            "image2pipe".into(),
            "pipe:1".into(),
        ])
        .collect();
        let borrowed: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
        let bytes = video_engine::run(
            &video_engine::tool(engine, "ffmpeg")?,
            &borrowed,
            Duration::from_secs(30),
        )?;
        if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.len() > 2 * 1024 * 1024 {
            return Err("Video poster did not produce a bounded PNG".into());
        }
        if signature(&source)? != before {
            return Err("Video changed while its poster was generated".into());
        }
        cache_disk::write(&key, &bytes);
        bytes
    };
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}
#[tauri::command]
pub async fn video_poster(
    app: tauri::AppHandle,
    jobs: tauri::State<'_, PosterJobs>,
    root: String,
    path: String,
    max_edge: u32,
    for_player: Option<bool>,
) -> Result<String, String> {
    let engine = video_engine::directory(&app)?;
    // A large library's queued thumbnails must not delay the selected player's poster.
    let gate = if for_player.unwrap_or(false) {
        &jobs.player
    } else {
        &jobs.library
    };
    let permit = gate
        .clone()
        .acquire_owned()
        .await
        .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        poster(&engine, &root, &path, max_edge)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Timeline-only frame lookup: path scoped, bounded, cached, and read-only.
#[tauri::command]
pub async fn video_timeline_frame(
    app: tauri::AppHandle,
    jobs: tauri::State<'_, PosterJobs>,
    root: String,
    path: String,
    seconds: f64,
) -> Result<String, String> {
    if !seconds.is_finite() || !(0.0..=7200.0).contains(&seconds) {
        return Err("Invalid filmstrip timestamp".into());
    }
    let engine = video_engine::directory(&app)?;
    let permit = jobs
        .library
        .clone()
        .acquire_owned()
        .await
        .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        frame_at(&engine, &root, &path, 160, Some(seconds))
    })
    .await
    .map_err(|e| e.to_string())?
}
