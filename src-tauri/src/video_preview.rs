//! Session-owned playback derivatives. Originals are never opened for writing.
use crate::{
    video_engine,
    video_probe::{self, VideoInfo},
};
use base64::Engine;
use serde::Serialize;
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::Manager;

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const MAX_PROXY_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Serialize)]
pub struct FilmFrame {
    pub seconds: f64,
    pub image: String,
}
#[derive(Clone, Serialize)]
pub struct PreviewAssets {
    pub source: String,
    pub proxy: bool,
    pub info: VideoInfo,
    pub filmstrip: Vec<FilmFrame>,
    pub warnings: Vec<String>,
}
#[derive(Clone, Serialize)]
pub struct PreviewStatus {
    state: &'static str,
    result: Option<PreviewAssets>,
    error: Option<String>,
}
struct Job {
    id: String,
    cancel: AtomicBool,
    status: Mutex<PreviewStatus>,
    directory: Mutex<Option<PathBuf>>,
}
impl Drop for Job {
    fn drop(&mut self) {
        if let Ok(directory) = self.directory.get_mut() {
            if let Some(directory) = directory.take() {
                let _ = fs::remove_dir_all(directory);
            }
        }
    }
}
pub struct PreviewJobs {
    current: Mutex<Option<Arc<Job>>>,
    gate: Arc<tokio::sync::Semaphore>,
}
impl Default for PreviewJobs {
    fn default() -> Self {
        Self {
            current: Mutex::new(None),
            gate: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }
}
impl PreviewJobs {
    fn replace(&self, job: Arc<Job>) -> Result<(), String> {
        if let Some(previous) = self.current.lock().map_err(|e| e.to_string())?.replace(job) {
            previous.cancel.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
    fn cancel(&self, id: &str) -> Result<(), String> {
        let mut current = self.current.lock().map_err(|e| e.to_string())?;
        if current.as_ref().is_some_and(|job| job.id == id) {
            if let Some(job) = current.take() {
                job.cancel.store(true, Ordering::SeqCst);
            }
        }
        Ok(())
    }
}
impl Drop for PreviewJobs {
    fn drop(&mut self) {
        if let Ok(Some(job)) = self.current.get_mut() {
            job.cancel.store(true, Ordering::SeqCst);
        }
    }
}
fn cancelled(job: &Job) -> bool {
    job.cancel.load(Ordering::SeqCst)
}
fn new_id() -> String {
    format!(
        "preview-{:x}-{:x}-{:x}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}
// Remove only our old derivative folders. Recent folders may belong to another app instance.
fn prune_abandoned(root: &Path) {
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("preview-")
                && name[8..].chars().all(|c| c.is_ascii_hexdigit() || c == '-')
                && entry
                    .file_type()
                    .is_ok_and(|t| t.is_dir() && !t.is_symlink())
                && entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|m| m.elapsed().ok())
                    .is_some_and(|age| age > Duration::from_secs(86400))
            {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }
}
fn call(
    engine: &Path,
    args: Vec<OsString>,
    timeout: Duration,
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    let borrowed: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    video_engine::run_until(
        &video_engine::tool(engine, "ffmpeg")?,
        &borrowed,
        timeout,
        cancel,
    )
    .map(|_| ())
}
fn input_args(source: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "-nostdin",
        "-v",
        "error",
        "-n",
        "-max_alloc",
        "268435456",
        "-threads",
        "2",
        "-filter_threads",
        "1",
        "-protocol_whitelist",
        "file,pipe",
        "-format_whitelist",
        "mov,matroska,webm,avi",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    args.push("-i".into());
    args.push(source.into());
    args
}
fn signature(path: &Path) -> Result<(u64, SystemTime), String> {
    let m = fs::metadata(path).map_err(|e| e.to_string())?;
    Ok((m.len(), m.modified().map_err(|e| e.to_string())?))
}

pub(crate) fn build_assets(
    engine: &Path,
    source: &Path,
    directory: &Path,
    proxy: bool,
    cancel: impl Fn() -> bool,
) -> Result<PreviewAssets, String> {
    let before = signature(source)?;
    let info = video_probe::probe(engine, source, &cancel)?;
    if info.hdr {
        return Err("HDR compatible previews and filmstrips require tested tone mapping. Use original playback or the Windows player for this clip.".into());
    }
    if info.duration_seconds > 7200.0
        || u64::from(info.width) * u64::from(info.height) > 100_000_000
    {
        return Err(
            "Preview generation supports videos up to two hours and 100 megapixels per frame."
                .into(),
        );
    }
    let destination = directory.join("preview.mp4");
    let mut warnings = vec![];
    if proxy {
        let mut args = input_args(source);
        // Preserve relative stream offsets and presentation timing, including VFR.
        args.splice(
            0..0,
            [OsString::from("-copyts"), OsString::from("-start_at_zero")],
        );
        args.extend(["-map".into(), format!("0:{}", info.video_stream).into()]);
        if let Some(audio) = info.audio.first() {
            args.extend(["-map".into(), format!("0:{}", audio.stream).into()]);
        }
        args.extend(["-vf", "scale=w='min(960,iw*sar)':h='min(540,ih)':force_original_aspect_ratio=decrease:force_divisible_by=2:reset_sar=1", "-fps_mode", "vfr", "-c:v", "libopenh264", "-b:v", "2000k", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "128k", "-ac", "2", "-map_metadata", "-1", "-movflags", "+faststart", "-fs", "536870912"].into_iter().map(Into::into));
        args.push(destination.clone().into());
        call(engine, args, Duration::from_secs(1800), &cancel)?;
        let output = video_probe::probe(engine, &destination, &cancel)?;
        if (output.duration_seconds - info.duration_seconds).abs() > 0.15
            || output.rotation_degrees.abs() > 0.01
            || output.width > 960
            || output.height > 540
            || fs::metadata(&destination).map_err(|e| e.to_string())?.len() >= MAX_PROXY_BYTES
        {
            return Err("Compatible preview failed timing, orientation, or size validation. Try original playback.".into());
        }
        if info.audio.len() > 1 {
            warnings.push(
                "Compatible preview plays the first audio track only; the original is unchanged."
                    .into(),
            );
        }
    }
    let playback = if proxy { &destination } else { source };
    let mut filmstrip = vec![];
    // Small, bounded JPEGs cross IPC; video bytes always use streaming.
    for index in 0..6 {
        if cancel() {
            return Err("Video preview cancelled".into());
        }
        let seconds = info.duration_seconds * (index as f64 + 0.5) / 6.0;
        let frame = directory.join(format!("frame-{index}.jpg"));
        let mut args = input_args(playback);
        let input_position = args.iter().position(|arg| arg == "-i").unwrap();
        args.splice(
            input_position..input_position,
            ["-ss".into(), format!("{seconds:.6}").into()],
        );
        args.extend([
            "-map".into(),
            if proxy {
                "0:v:0".into()
            } else {
                format!("0:{}", info.video_stream).into()
            },
        ]);
        args.extend(
            [
                "-an",
                "-frames:v",
                "1",
                "-vf",
                "scale=160:90:force_original_aspect_ratio=decrease:reset_sar=1",
                "-q:v",
                "5",
                "-update",
                "1",
            ]
            .into_iter()
            .map(Into::into),
        );
        args.push(frame.clone().into());
        if let Err(error) = call(engine, args, Duration::from_secs(20), &cancel) {
            if cancel() {
                return Err(error);
            }
            warnings.push(
                "Some filmstrip frames could not be generated. Playback remains available.".into(),
            );
            break;
        }
        if !fs::metadata(&frame).is_ok_and(|m| m.len() <= 64 * 1024) {
            warnings.push("A filmstrip frame was unavailable or exceeded its size limit. Playback remains available.".into());
            break;
        }
        let bytes = fs::read(&frame).map_err(|e| e.to_string())?;
        filmstrip.push(FilmFrame {
            seconds,
            image: format!(
                "data:image/jpeg;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            ),
        });
    }
    if before != signature(source)? {
        return Err(
            "The source changed while creating the preview. Reopen it and try again.".into(),
        );
    }
    if cancel() {
        return Err("Video preview cancelled".into());
    }
    Ok(PreviewAssets {
        source: playback.to_string_lossy().into_owned(),
        proxy,
        info,
        filmstrip,
        warnings,
    })
}

#[tauri::command]
pub fn start_video_preview(
    app: tauri::AppHandle,
    jobs: tauri::State<'_, PreviewJobs>,
    root: String,
    path: String,
    proxy: bool,
) -> Result<String, String> {
    let source = crate::media_stream::validated_video(&root, &path)?;
    start_job(app, jobs, source, proxy, None)
}

#[tauri::command]
pub fn start_video_project_preview(
    app: tauri::AppHandle,
    jobs: tauri::State<'_, PreviewJobs>,
    root: String,
    project: crate::video_project::Project,
) -> Result<String, String> {
    crate::video_project::validate(&project)?;
    start_job(app, jobs, PathBuf::new(), true, Some((root, project)))
}

fn start_job(
    app: tauri::AppHandle,
    jobs: tauri::State<'_, PreviewJobs>,
    source: PathBuf,
    proxy: bool,
    project: Option<(String, crate::video_project::Project)>,
) -> Result<String, String> {
    let engine = video_engine::directory(&app)?;
    let cache = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("video-preview-v1");
    let job = Arc::new(Job {
        id: new_id(),
        cancel: AtomicBool::new(false),
        status: Mutex::new(PreviewStatus {
            state: "queued",
            result: None,
            error: None,
        }),
        directory: Mutex::new(None),
    });
    let id = job.id.clone();
    jobs.replace(job.clone())?;
    let gate = jobs.gate.clone();
    tauri::async_runtime::spawn(async move {
        let permit = gate.acquire_owned().await;
        let Ok(permit) = permit else {
            return;
        };
        if cancelled(&job) {
            return;
        }
        let _ = tauri::async_runtime::spawn_blocking(move || {
            let _permit = permit;
            let result = (|| {
                fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
                prune_abandoned(&cache);
                let directory = cache.join(&job.id);
                fs::create_dir(&directory).map_err(|e| e.to_string())?;
                *job.directory.lock().map_err(|e| e.to_string())? = Some(directory.clone());
                job.status.lock().map_err(|e| e.to_string())?.state = "running";
                let result = if let Some((root, project)) = project {
                    crate::video_edit_preview::build(&engine, &root, &project, &directory, || {
                        cancelled(&job)
                    })?
                } else {
                    build_assets(&engine, &source, &directory, proxy, || cancelled(&job))?
                };
                if cancelled(&job) {
                    return Err("Video preview cancelled".into());
                }
                app.asset_protocol_scope()
                    .allow_file(&result.source)
                    .map_err(|e| e.to_string())?;
                Ok(result)
            })();
            if let Ok(mut status) = job.status.lock() {
                *status = match result {
                    Ok(result) => PreviewStatus {
                        state: "complete",
                        result: Some(result),
                        error: None,
                    },
                    Err(error) => PreviewStatus {
                        state: "failed",
                        result: None,
                        error: Some(error),
                    },
                };
            }
        })
        .await;
    });
    Ok(id)
}
#[tauri::command]
pub fn video_preview_status(
    jobs: tauri::State<'_, PreviewJobs>,
    id: String,
) -> Result<PreviewStatus, String> {
    let current = jobs.current.lock().map_err(|e| e.to_string())?;
    let job = current
        .as_ref()
        .filter(|job| job.id == id)
        .ok_or("Video preview no longer active")?;
    let result = job.status.lock().map_err(|e| e.to_string())?.clone();
    Ok(result)
}
#[tauri::command]
pub fn cancel_video_preview(jobs: tauri::State<'_, PreviewJobs>, id: String) -> Result<(), String> {
    jobs.cancel(&id)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn job() -> Arc<Job> {
        Arc::new(Job {
            id: new_id(),
            cancel: AtomicBool::new(false),
            status: Mutex::new(PreviewStatus {
                state: "queued",
                result: None,
                error: None,
            }),
            directory: Mutex::new(None),
        })
    }
    #[test]
    fn obsolete_cancel_does_not_stop_the_new_job_and_cleanup_waits_for_worker() {
        let jobs = PreviewJobs::default();
        let old = job();
        let new = job();
        let root = std::env::temp_dir().join(new_id());
        fs::create_dir(&root).unwrap();
        let owned = root.join("owned");
        fs::create_dir(&owned).unwrap();
        fs::write(owned.join("partial.mp4"), b"partial").unwrap();
        let unrelated = root.join("original.mp4");
        fs::write(&unrelated, b"original").unwrap();
        *old.directory.lock().unwrap() = Some(owned.clone());
        jobs.replace(old.clone()).unwrap();
        jobs.replace(new.clone()).unwrap();
        assert!(cancelled(&old));
        assert!(owned.exists());
        jobs.cancel(&old.id).unwrap();
        assert!(!cancelled(&new));
        drop(old);
        assert!(!owned.exists());
        assert_eq!(fs::read(&unrelated).unwrap(), b"original");
        jobs.cancel(&new.id).unwrap();
        assert!(cancelled(&new));
        fs::remove_dir_all(root).unwrap();
    }
}
