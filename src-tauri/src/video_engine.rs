//! Pinned, bundled tools only: never execute a tool discovered on PATH.
use serde::Serialize;
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::Manager;

pub(crate) const VERSION: &str = "n8.1.3-24-g92611900bb";
pub(crate) const ARCHIVE_SHA256: &str =
    "6c6167da57decb941a0466b60d95f7f51e4edd6b2d79c24dc4860c1c15701292";

// Closing this handle (including on app exit) kills native work rather than leaving an orphan.
#[cfg(windows)]
pub(crate) struct ProcessJob(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl ProcessJob {
    pub(crate) fn attach(child: &std::process::Child) -> Result<Self, String> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::*;
        // SAFETY: null security/name arguments create a private, non-inheritable job.
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let job = Self(handle);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: the buffer matches the requested Windows structure and both handles are live.
        let success = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            ) != 0
                && AssignProcessToJobObject(handle, child.as_raw_handle()) != 0
        };
        if !success {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(job)
    }
}
#[cfg(windows)]
impl Drop for ProcessJob {
    fn drop(&mut self) {
        // SAFETY: this guard owns the job handle and closes it exactly once.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

pub(crate) fn directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    if cfg!(debug_assertions) {
        Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/video-engine"))
    } else {
        Ok(app
            .path()
            .resource_dir()
            .map_err(|e| e.to_string())?
            .join("video-engine"))
    }
}

pub(crate) fn tool(directory: &Path, name: &str) -> Result<PathBuf, String> {
    if !matches!(name, "ffprobe" | "ffmpeg") {
        return Err("Unknown video tool".into());
    }
    let manifest:serde_json::Value=serde_json::from_slice(&std::fs::read(directory.join("engine.json")).map_err(|_|"Bundled video engine is missing; reinstall Open-Photo or prepare the development engine".to_string())?).map_err(|e|e.to_string())?;
    if manifest["version"] != VERSION || manifest["archive_sha256"] != ARCHIVE_SHA256 {
        return Err("Video engine does not match this Open-Photo build".into());
    }
    let executable = directory.join(format!("{name}.exe"));
    if !executable.is_file() {
        return Err("Bundled video executable is missing".into());
    }
    Ok(executable)
}

fn collect(
    mut reader: impl Read,
    limit: usize,
    overflow: Arc<AtomicBool>,
) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let count = reader.read(&mut chunk).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        let remaining = limit.saturating_sub(result.len());
        result.extend_from_slice(&chunk[..count.min(remaining)]);
        if count > remaining {
            overflow.store(true, Ordering::Release);
        }
    }
    Ok(result)
}

/// Drain both pipes concurrently, cap allocations, and reap the child on all failure paths.
pub(crate) fn run(
    executable: &Path,
    args: &[&std::ffi::OsStr],
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    run_until(executable, args, timeout, || false)
}

pub(crate) fn run_until(
    executable: &Path,
    args: &[&std::ffi::OsStr],
    timeout: Duration,
    cancel: impl Fn() -> bool,
) -> Result<Vec<u8>, String> {
    run_reader(executable, args, timeout, cancel, |reader, overflow| {
        collect(reader, 4 * 1024 * 1024, overflow)
    })
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Progress {
    pub seconds: f64,
    pub bytes: u64,
}

fn read_lines(
    reader: impl Read,
    overflow: Arc<AtomicBool>,
    mut line_handler: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<Vec<u8>, String> {
    use std::io::BufRead;
    let mut reader = std::io::BufReader::new(reader);
    let mut line = Vec::with_capacity(256);
    loop {
        line.clear();
        let n = Read::by_ref(&mut reader)
            .take(4097)
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if n > 4096 {
            overflow.store(true, Ordering::Release);
            return Err("Video tool output line exceeded its safety limit".into());
        }
        if let Err(error) = line_handler(&line) {
            overflow.store(true, Ordering::Release);
            return Err(error);
        }
    }
    Ok(vec![])
}
fn progress_reader(
    reader: impl Read,
    overflow: Arc<AtomicBool>,
    progress: impl Fn(Progress),
) -> Result<Vec<u8>, String> {
    let mut state = Progress::default();
    read_lines(reader, overflow, |line| {
        let text = String::from_utf8_lossy(line);
        let Some((key, value)) = text.trim().split_once('=') else {
            return Ok(());
        };
        match key {
            "out_time_us" => {
                if let Ok(micros) = value.parse::<i64>() {
                    state.seconds = (micros as f64 / 1_000_000.0).max(0.0);
                }
            }
            "total_size" => {
                if let Ok(bytes) = value.parse() {
                    state.bytes = bytes;
                }
            }
            "progress" => progress(state),
            _ => {}
        }
        Ok(())
    })
}
pub(crate) fn run_lines(
    executable: &Path,
    args: &[&std::ffi::OsStr],
    timeout: Duration,
    cancel: impl Fn() -> bool,
    line_handler: impl FnMut(&[u8]) -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    run_reader(
        executable,
        args,
        timeout,
        cancel,
        move |reader, overflow| read_lines(reader, overflow, line_handler),
    )
    .map(|_| ())
}
pub(crate) fn run_progress(
    executable: &Path,
    args: &[&std::ffi::OsStr],
    timeout: Duration,
    cancel: impl Fn() -> bool,
    progress: impl Fn(Progress) + Send + 'static,
) -> Result<(), String> {
    run_reader(
        executable,
        args,
        timeout,
        cancel,
        move |reader, overflow| progress_reader(reader, overflow, progress),
    )
    .map(|_| ())
}
fn run_reader(
    executable: &Path,
    args: &[&std::ffi::OsStr],
    timeout: Duration,
    cancel: impl Fn() -> bool,
    reader: impl FnOnce(std::process::ChildStdout, Arc<AtomicBool>) -> Result<Vec<u8>, String>
        + Send
        + 'static,
) -> Result<Vec<u8>, String> {
    let mut command = Command::new(executable);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Cannot start the bundled video tool: {e}"))?;
    #[cfg(windows)]
    let _process_job = match ProcessJob::attach(&child) {
        Ok(job) => job,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Cannot contain the video process: {error}"));
        }
    };
    let overflow = Arc::new(AtomicBool::new(false));
    let out = child.stdout.take().ok_or("Video tool stdout unavailable")?;
    let err = child.stderr.take().ok_or("Video tool stderr unavailable")?;
    let flag = overflow.clone();
    let stdout = thread::spawn(move || reader(out, flag));
    let flag = overflow.clone();
    let stderr = thread::spawn(move || collect(err, 64 * 1024, flag));
    let started = Instant::now();
    let mut failure = None;
    let status = loop {
        if cancel() || overflow.load(Ordering::Acquire) || started.elapsed() > timeout {
            failure = Some(
                if cancel() {
                    "Video processing cancelled or superseded"
                } else if overflow.load(Ordering::Acquire) {
                    "Video tool output exceeded its safety limit"
                } else {
                    "Video processing timed out"
                }
                .to_string(),
            );
            let _ = child.kill();
            break child.wait();
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(error);
            }
        }
    };
    let output = stdout.join().map_err(|_| "Video output reader failed")??;
    let diagnostics = stderr
        .join()
        .map_err(|_| "Video diagnostic reader failed")??;
    if let Some(failure) = failure {
        return Err(failure);
    }
    if overflow.load(Ordering::Acquire) {
        return Err("Video tool output exceeded its safety limit".into());
    }
    if !status.map_err(|e| e.to_string())?.success() {
        return Err(format!(
            "Video processing failed: {}",
            String::from_utf8_lossy(&diagnostics).trim()
        ));
    }
    Ok(output)
}

#[derive(Serialize)]
pub struct EngineStatus {
    version: &'static str,
    available: bool,
    message: String,
    video_decoders: Vec<String>,
    video_encoders: Vec<String>,
}

fn video_codecs(output: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(output)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let flags = parts.next()?;
            if flags.len() == 6 && flags.starts_with('V') {
                parts.next().filter(|name| *name != "=").map(str::to_owned)
            } else {
                None
            }
        })
        .collect()
}

#[tauri::command]
pub async fn video_engine_status(app: tauri::AppHandle) -> Result<EngineStatus, String> {
    let directory = directory(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let result = tool(&directory, "ffprobe")
            .and_then(|path| run(&path, &["-version".as_ref()], Duration::from_secs(10)));
        let ffmpeg = tool(&directory, "ffmpeg")?;
        let decoders = run(
            &ffmpeg,
            &["-hide_banner".as_ref(), "-decoders".as_ref()],
            Duration::from_secs(10),
        )?;
        let encoders = run(
            &ffmpeg,
            &["-hide_banner".as_ref(), "-encoders".as_ref()],
            Duration::from_secs(10),
        )?;
        Ok(EngineStatus {
            version: VERSION,
            available: result.is_ok(),
            message: result
                .err()
                .unwrap_or_else(|| "Local video inspection available".into()),
            video_decoders: video_codecs(&decoders),
            video_encoders: video_codecs(&encoders),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn codec_inventory_excludes_the_flags_legend() {
        assert_eq!(
            video_codecs(b" V..... = Video\n VFS..D h264 H.264 decoder\n A....D aac AAC decoder\n"),
            vec!["h264"]
        );
    }
    #[test]
    fn reader_stays_bounded_and_reports_overflow() {
        let flag = Arc::new(AtomicBool::new(false));
        let result = collect(&b"0123456789"[..], 4, flag.clone()).unwrap();
        assert_eq!(result, b"0123");
        assert!(flag.load(Ordering::Acquire));
    }
    #[test]
    fn progress_stream_reports_time_without_retaining_logs_and_bounds_each_line() {
        let values = Arc::new(std::sync::Mutex::new(vec![]));
        let copy = values.clone();
        let flag = Arc::new(AtomicBool::new(false));
        let output=progress_reader(&b"out_time_us=1500000\ntotal_size=2048\nprogress=continue\nout_time_us=2500000\nprogress=end\n"[..],flag.clone(),move|p|copy.lock().unwrap().push((p.seconds,p.bytes))).unwrap();
        assert!(output.is_empty());
        assert_eq!(*values.lock().unwrap(), vec![(1.5, 2048), (2.5, 2048)]);
        assert!(!flag.load(Ordering::Acquire));
        assert!(progress_reader(&vec![b'x'; 100_000][..], flag.clone(), |_| {}).is_err());
        assert!(flag.load(Ordering::Acquire));
    }
}
