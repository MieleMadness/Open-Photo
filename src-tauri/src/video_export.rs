//! Validate and fully decode staged MP4s before publishing without overwrite.
use crate::{
    video_engine,
    video_project::{self, Project},
    video_render::{self, ExportSize},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Queued,
    VerifyingSource,
    Encoding,
    CheckingOutput,
    DecodingOutput,
    Publishing,
    Cancelling,
    Completed,
    Cancelled,
    Failed,
}
#[derive(Clone, Debug, Serialize)]
pub struct ExportReport {
    pub path: String,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
    pub duration_seconds: f64,
    pub warnings: Vec<String>,
}
#[derive(Clone, Serialize)]
pub struct ExportStatus {
    pub id: String,
    pub title: String,
    pub destination: String,
    pub active: bool,
    pub can_cancel: bool,
    pub stage: Stage,
    pub fraction: Option<f64>,
    pub bytes: u64,
    pub elapsed_seconds: f64,
    pub error: Option<String>,
    pub result: Option<ExportReport>,
}
pub struct Request<'a> {
    pub engine: &'a Path,
    pub root: &'a str,
    pub project: &'a Project,
    pub destination: &'a Path,
    pub size: ExportSize,
    pub acknowledge_first_audio: bool,
}
pub trait Control: Send + Sync {
    fn cancelled(&self) -> bool;
    fn update(&self, stage: Stage, fraction: Option<f64>, bytes: u64);
    fn begin_publish(&self) -> Result<(), String>;
    fn available_space(&self, parent: &Path) -> Result<Option<u64>, String> {
        available_space(parent)
    }
}
fn available_space(parent: &Path) -> Result<Option<u64>, String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let mut path: Vec<u16> = parent.as_os_str().encode_wide().collect();
        // GetDiskFreeSpaceExW requires a trailing separator for UNC share roots.
        if !matches!(path.last(), Some(92 | 47)) {
            path.push(92);
        }
        path.push(0);
        let mut free = 0u64;
        // SAFETY: path is NUL terminated and free is a live output buffer.
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
                path.as_ptr(),
                &mut free,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(format!(
                "Cannot check destination space: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(Some(free))
    }
    #[cfg(not(windows))]
    {
        let _ = parent;
        Ok(None)
    }
}
fn check_cancel(control: &dyn Control) -> Result<(), String> {
    if control.cancelled() {
        Err("Export cancelled".into())
    } else {
        Ok(())
    }
}

pub fn export_file(
    request: Request<'_>,
    control: Arc<dyn Control>,
) -> Result<ExportReport, String> {
    if !request
        .destination
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"))
    {
        return Err("Choose a new .mp4 filename".into());
    }
    video_project::validate(request.project)?;
    check_cancel(control.as_ref())?;
    control.update(Stage::VerifyingSource, None, 0);
    let parent = fs::canonicalize(
        request
            .destination
            .parent()
            .ok_or("Choose an existing output folder")?,
    )
    .map_err(|e| e.to_string())?;
    let duration = request.project.clips[0].source_out.seconds()?
        - request.project.clips[0].source_in.seconds()?;
    // A conservative allowance for the target bitrate plus muxing/faststart overhead.
    let bitrate = video_render::target_bitrate(request.project, request.size) + 192_000;
    let required = (duration * bitrate as f64 / 8.0 * 1.25).ceil() as u64 + 64 * 1024 * 1024;
    if control
        .available_space(&parent)?
        .is_some_and(|free| free < required)
    {
        return Err(format!(
            "Not enough free space in the output folder (allow at least {} MB)",
            required.div_ceil(1024 * 1024)
        ));
    }
    let staged = crate::safe_output::StagedOutput::new(request.destination)?;
    let temporary = staged.temporary_path().to_owned();
    let result = (|| {
        let plan = video_render::plan(
            request.engine,
            request.root,
            request.project,
            &temporary,
            video_render::RenderOptions {
                preview: false,
                size: request.size,
                acknowledge_first_audio: request.acknowledge_first_audio,
            },
            || control.cancelled(),
        )?;
        check_cancel(control.as_ref())?;
        control.update(Stage::Encoding, Some(0.0), 0);
        let progress = control.clone();
        let length = plan.duration;
        video_render::encode(
            request.engine,
            &plan,
            || control.cancelled(),
            move |p| {
                progress.update(
                    Stage::Encoding,
                    Some((p.seconds / length).clamp(0.0, 1.0) * 0.9),
                    p.bytes,
                )
            },
        )?;
        check_cancel(control.as_ref())?;
        let bytes = fs::metadata(&temporary).map_err(|e| e.to_string())?.len();
        control.update(Stage::CheckingOutput, Some(0.9), bytes);
        let info = video_render::validate_output(
            request.engine,
            request.project,
            &plan,
            &temporary,
            || control.cancelled(),
        )?;
        check_cancel(control.as_ref())?;
        control.update(Stage::DecodingOutput, Some(0.9), bytes);
        let progress = control.clone();
        video_render::decode_check(
            request.engine,
            &temporary,
            || control.cancelled(),
            move |p| {
                progress.update(
                    Stage::DecodingOutput,
                    Some(0.9 + (p.seconds / length).clamp(0.0, 1.0) * 0.09),
                    bytes,
                )
            },
        )?;
        control.update(Stage::VerifyingSource, Some(0.99), bytes);
        video_project::source(request.root, request.project, || control.cancelled())?;
        check_cancel(control.as_ref())?;
        control.begin_publish()?;
        let path = staged.commit()?;
        Ok(ExportReport {
            path: path.to_string_lossy().into_owned(),
            bytes,
            width: info.width,
            height: info.height,
            duration_seconds: info.duration_seconds,
            warnings: plan.warnings,
        })
    })();
    if result.is_err() && temporary.exists() {
        return Err(format!(
            "{}. Temporary file cleanup failed: {}",
            result.as_ref().err().unwrap(),
            temporary.display()
        ));
    }
    result
}

struct Job {
    cancel: AtomicBool,
    started: Instant,
    status: Mutex<ExportStatus>,
}
impl Control for Job {
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
    fn update(&self, stage: Stage, fraction: Option<f64>, bytes: u64) {
        if let Ok(mut status) = self.status.lock() {
            if !self.cancelled() {
                status.stage = stage;
                status.fraction = fraction;
                status.bytes = bytes;
            }
        }
    }
    fn begin_publish(&self) -> Result<(), String> {
        let mut status = self.status.lock().map_err(|e| e.to_string())?;
        check_cancel(self)?;
        status.stage = Stage::Publishing;
        status.can_cancel = false;
        status.fraction = Some(0.99);
        Ok(())
    }
}
#[derive(Default)]
pub struct ExportJobs {
    current: Mutex<Option<Arc<Job>>>,
}
impl Drop for ExportJobs {
    fn drop(&mut self) {
        if let Ok(Some(job)) = self.current.get_mut() {
            job.cancel.store(true, Ordering::SeqCst);
        }
    }
}
impl ExportJobs {
    fn status(&self) -> Result<Option<ExportStatus>, String> {
        let current = self.current.lock().map_err(|e| e.to_string())?;
        current
            .as_ref()
            .map(|job| {
                let mut status = job.status.lock().map_err(|e| e.to_string())?.clone();
                if status.active {
                    status.elapsed_seconds = job.started.elapsed().as_secs_f64();
                }
                Ok(status)
            })
            .transpose()
    }
    fn cancel(&self, id: &str) -> Result<(), String> {
        let current = self.current.lock().map_err(|e| e.to_string())?;
        let job = current.as_ref().ok_or("No export is active")?;
        let mut status = job.status.lock().map_err(|e| e.to_string())?;
        if status.id != id {
            return Err("This export is no longer active".into());
        }
        if !status.active {
            return Ok(());
        }
        if !status.can_cancel {
            return Err("Export is publishing the completed file; wait for it to finish".into());
        }
        job.cancel.store(true, Ordering::SeqCst);
        status.stage = Stage::Cancelling;
        status.can_cancel = false;
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub destination: String,
    pub size: ExportSize,
    pub acknowledge_first_audio: bool,
}
#[tauri::command]
pub fn start_video_export(
    app: tauri::AppHandle,
    jobs: tauri::State<'_, ExportJobs>,
    previews: tauri::State<'_, crate::video_preview::PreviewJobs>,
    root: String,
    project: Project,
    options: Options,
) -> Result<String, String> {
    let Options {
        destination,
        size,
        acknowledge_first_audio,
    } = options;
    video_project::validate(&project)?;
    let engine = video_engine::directory(&app)?;
    let mut current = jobs.current.lock().map_err(|e| e.to_string())?;
    if let Some(job) = current.as_ref() {
        if job.status.lock().map_err(|e| e.to_string())?.active {
            return Err("An export is already running. Wait or cancel it first.".into());
        }
    }
    let id = video_project::id();
    let job = Arc::new(Job {
        cancel: AtomicBool::new(false),
        started: Instant::now(),
        status: Mutex::new(ExportStatus {
            id: id.clone(),
            title: project.title.clone(),
            destination: destination.clone(),
            active: true,
            can_cancel: true,
            stage: Stage::Queued,
            fraction: None,
            bytes: 0,
            elapsed_seconds: 0.0,
            error: None,
            result: None,
        }),
    });
    *current = Some(job.clone());
    drop(current);
    let gate = previews.processing_gate();
    tauri::async_runtime::spawn_blocking(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _permit = loop {
                check_cancel(job.as_ref())?;
                match gate.clone().try_acquire_owned() {
                    Ok(permit) => break permit,
                    Err(tokio::sync::TryAcquireError::Closed) => {
                        return Err("Video engine queue is unavailable".into())
                    }
                    Err(tokio::sync::TryAcquireError::NoPermits) => {}
                }
                std::thread::sleep(Duration::from_millis(20));
            };
            export_file(
                Request {
                    engine: &engine,
                    root: &root,
                    project: &project,
                    destination: Path::new(&destination),
                    size,
                    acknowledge_first_audio,
                },
                job.clone(),
            )
        }))
        .unwrap_or_else(|_| Err("Video export stopped unexpectedly".into()));
        if let Ok(mut status) = job.status.lock() {
            status.elapsed_seconds = job.started.elapsed().as_secs_f64();
            status.active = false;
            status.can_cancel = false;
            match result {
                Ok(report) => {
                    status.stage = Stage::Completed;
                    status.fraction = Some(1.0);
                    status.bytes = report.bytes;
                    status.result = Some(report);
                }
                Err(error) => {
                    status.stage = if job.cancelled() && !error.contains("cleanup failed") {
                        Stage::Cancelled
                    } else {
                        Stage::Failed
                    };
                    status.error = Some(error);
                }
            }
        }
    });
    Ok(id)
}
#[tauri::command]
pub fn video_export_status(
    jobs: tauri::State<'_, ExportJobs>,
) -> Result<Option<ExportStatus>, String> {
    jobs.status()
}
#[tauri::command]
pub fn cancel_video_export(jobs: tauri::State<'_, ExportJobs>, id: String) -> Result<(), String> {
    jobs.cancel(&id)
}
#[tauri::command]
pub fn dismiss_video_export(jobs: tauri::State<'_, ExportJobs>, id: String) -> Result<(), String> {
    let mut current = jobs.current.lock().map_err(|e| e.to_string())?;
    if let Some(job) = current.as_ref() {
        let status = job.status.lock().map_err(|e| e.to_string())?;
        if status.id != id {
            return Err("Export changed".into());
        }
        if status.active {
            return Err("Cancel or wait for the export before dismissing".into());
        }
    }
    current.take();
    Ok(())
}
#[tauri::command]
pub fn open_video_export(jobs: tauri::State<'_, ExportJobs>, id: String) -> Result<(), String> {
    let status = jobs
        .status()?
        .filter(|s| s.id == id && s.stage == Stage::Completed)
        .ok_or("Completed export unavailable")?;
    opener::open(PathBuf::from(
        status.result.ok_or("Completed export unavailable")?.path,
    ))
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(windows)]
    fn queries_real_destination_space() {
        assert!(
            available_space(Path::new(env!("CARGO_MANIFEST_DIR")))
                .unwrap()
                .unwrap()
                > 0
        );
    }
    fn job() -> Arc<Job> {
        Arc::new(Job {
            cancel: AtomicBool::new(false),
            started: Instant::now(),
            status: Mutex::new(ExportStatus {
                id: video_project::id(),
                title: "test".into(),
                destination: "test.mp4".into(),
                active: true,
                can_cancel: true,
                stage: Stage::Encoding,
                fraction: Some(0.5),
                bytes: 0,
                elapsed_seconds: 0.0,
                error: None,
                result: None,
            }),
        })
    }
    #[test]
    fn cancellation_and_publication_have_a_single_commit_boundary() {
        let jobs = ExportJobs::default();
        let first = job();
        let id = first.status.lock().unwrap().id.clone();
        *jobs.current.lock().unwrap() = Some(first.clone());
        jobs.cancel(&id).unwrap();
        assert!(first.begin_publish().is_err());
        let second = job();
        let id = second.status.lock().unwrap().id.clone();
        *jobs.current.lock().unwrap() = Some(second.clone());
        second.begin_publish().unwrap();
        assert!(jobs.cancel(&id).is_err());
        assert!(!second.cancelled());
        assert!(!second.status.lock().unwrap().can_cancel);
    }
}
