//! One edit/stream mapping pipeline for previews and exports.
use crate::{
    video_engine,
    video_probe::{self, VideoInfo},
    video_project::{self, Project},
    video_timing::{self, Span},
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::Path,
    time::Duration,
};
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExportSize {
    #[default]
    Project,
    Hd1080,
}
pub const MAX_EXPORT_BYTES: u64 = 16 * 1024 * 1024 * 1024;
fn output_canvas(p: &Project, size: ExportSize) -> Project {
    let mut canvas = p.clone();
    if size == ExportSize::Hd1080 {
        let (w, h) = if canvas.output.height > canvas.output.width {
            (1080.0, 1920.0)
        } else {
            (1920.0, 1080.0)
        };
        let factor = (w / f64::from(canvas.output.width))
            .min(h / f64::from(canvas.output.height))
            .min(1.0);
        canvas.output.width =
            ((f64::from(canvas.output.width) * factor / 2.0).floor() as u32 * 2).max(16);
        canvas.output.height =
            ((f64::from(canvas.output.height) * factor / 2.0).floor() as u32 * 2).max(16);
    }
    canvas
}
pub fn target_bitrate(p: &Project, size: ExportSize) -> u64 {
    let canvas = output_canvas(p, size);
    (12_000_000 * u64::from(canvas.output.width) * u64::from(canvas.output.height) / (1920 * 1080))
        .clamp(12_000_000, 48_000_000)
}
pub struct Plan {
    pub info: VideoInfo,
    pub width: u32,
    pub height: u32,
    pub video_span: Span,
    pub audio_span: Option<Span>,
    pub duration: f64,
    pub max_bytes: u64,
    pub timeout: Duration,
    pub args: Vec<OsString>,
    pub warnings: Vec<String>,
}
fn seconds(value: Option<&str>) -> Option<f64> {
    value?.parse::<f64>().ok().filter(|v| v.is_finite())
}
#[derive(Clone, Copy)]
pub struct RenderOptions {
    pub preview: bool,
    pub size: ExportSize,
    pub acknowledge_first_audio: bool,
}
pub fn plan(
    engine: &Path,
    root: &str,
    p: &Project,
    output: &Path,
    options: RenderOptions,
    cancel: impl Fn() -> bool,
) -> Result<Plan, String> {
    let RenderOptions {
        preview,
        size,
        acknowledge_first_audio,
    } = options;
    let source = video_project::source(root, p, &cancel)?;
    let info = video_probe::probe(engine, &source, &cancel)?;
    if info.hdr {
        return Err("HDR rendering/export requires tested tone mapping. Use original playback for this clip.".into());
    }
    if info.width != p.asset.info.width
        || info.height != p.asset.info.height
        || (info.duration_seconds - p.asset.info.duration_seconds).abs() > 0.001
        || info.time_base != p.asset.info.time_base
    {
        return Err("Project source metadata does not match the verified video".into());
    }
    let clip = &p.clips[0];
    if !preview && !clip.edits.muted && info.audio.len() > 1 && !acknowledge_first_audio {
        return Err(
            "Confirm that export will use only the first audio track, converted to stereo".into(),
        );
    }
    let start = clip.source_in.seconds()?;
    let duration = clip.source_out.seconds()? - start;
    let (video_span, audio_span) =
        video_timing::source_spans(engine, &source, &info, start, duration, &cancel)?;
    let canvas = output_canvas(p, if preview { ExportSize::Project } else { size });
    let (filter, width, height) = video_project::filters(&canvas, preview)?;
    let max_bytes = if preview {
        512 * 1024 * 1024
    } else {
        MAX_EXPORT_BYTES
    };
    // -y applies only to a job-owned cache file or StagedOutput's private staging path.
    let mut args: Vec<OsString> = [
        "-nostdin",
        "-v",
        "error",
        "-xerror",
        "-y",
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
        "-ss",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    args.extend([
        format!("{start:.9}").into(),
        "-i".into(),
        source.clone().into(),
        "-t".into(),
        format!("{duration:.9}").into(),
        "-map".into(),
        format!("0:{}", info.video_stream).into(),
        "-vf".into(),
        filter.into(),
    ]);
    if !clip.edits.muted && audio_span.is_some() {
        if let Some(audio) = info.audio.first() {
            args.extend([
                "-map".into(),
                format!("0:{}", audio.stream).into(),
                "-af".into(),
                format!("volume={:.4}", f64::from(clip.edits.gain_percent) / 100.0).into(),
            ]);
        }
    }
    args.extend([
        "-b:v".into(),
        if preview {
            "2000000".into()
        } else {
            target_bitrate(p, size).to_string().into()
        },
    ]);
    args.extend(
        [
            "-fps_mode",
            "vfr",
            // Carry fine source timestamps instead of quantizing trims onto nominal frame ticks.
            "-enc_time_base:v",
            info.time_base.as_deref().unwrap_or("1/1000000"),
            "-c:v",
            "libopenh264",
            "-force_key_frames:v",
            video_engine::H264_KEYFRAME_EXPRESSION,
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "-ar",
            "48000",
            "-ac",
            "2",
            "-map_metadata",
            "-1",
            "-map_chapters",
            "-1",
            "-movflags",
            "+faststart",
            "-movie_timescale",
            "90000",
            "-fs",
        ]
        .into_iter()
        .map(Into::into),
    );
    args.extend([
        max_bytes.to_string().into(),
        "-progress".into(),
        "pipe:1".into(),
        "-nostats".into(),
        "-f".into(),
        "mp4".into(),
        output.into(),
    ]);
    let mut warnings=vec!["MP4 contains SDR H.264 video and optional AAC stereo audio. Subtitles, chapters, and source metadata are not copied.".into()];
    if !clip.edits.muted && info.audio.len() > 1 {
        warnings
            .push("Only the first source audio track was included, converted to stereo.".into());
    }
    Ok(Plan {
        info,
        width,
        height,
        video_span,
        audio_span,
        duration,
        max_bytes,
        timeout: Duration::from_secs(if preview { 1800 } else { 21600 }),
        args,
        warnings,
    })
}
pub fn encode(
    engine: &Path,
    plan: &Plan,
    cancel: impl Fn() -> bool,
    progress: impl Fn(video_engine::Progress) + Send + 'static,
) -> Result<(), String> {
    let args: Vec<&OsStr> = plan.args.iter().map(OsString::as_os_str).collect();
    video_engine::run_progress(
        &video_engine::tool(engine, "ffmpeg")?,
        &args,
        plan.timeout,
        cancel,
        progress,
    )
}
pub fn validate_output(
    engine: &Path,
    p: &Project,
    plan: &Plan,
    output: &Path,
    cancel: impl Fn() -> bool,
) -> Result<VideoInfo, String> {
    let result = video_probe::probe(engine, output, cancel)?;
    let tolerance = (video_timing::frame_interval(&plan.info) * 2.0).clamp(0.005, 0.1);
    let video_start =
        seconds(result.start_time.as_deref()).ok_or("Rendered video has no start timestamp")?;
    let video_duration = result
        .video_duration_seconds
        .ok_or("Rendered video has no stream duration")?;
    if (video_start - plan.video_span.start).abs() > tolerance
        || (video_start + video_duration - plan.video_span.end).abs() > tolerance
    {
        return Err("Rendered video track failed independent timing validation".into());
    }
    if (result.duration_seconds
        - if p.clips[0].edits.muted {
            plan.video_span.end
        } else {
            plan.audio_span
                .map_or(plan.video_span.end, |a| a.end.max(plan.video_span.end))
        })
    .abs()
        > tolerance
        || result.width != plan.width
        || result.height != plan.height
        || result.rotation_degrees.abs() > 0.01
        || result.codec != "h264"
        || result.pixel_format.as_deref() != Some("yuv420p")
        || result.hdr
        || fs::metadata(output).map_err(|e| e.to_string())?.len() >= plan.max_bytes
    {
        return Err("Rendered video failed duration, geometry, format, or size validation".into());
    }
    let expect_audio = !p.clips[0].edits.muted && plan.audio_span.is_some();
    if result.audio.len() != usize::from(expect_audio) {
        return Err("Rendered video did not preserve the selected audio policy".into());
    }
    if let Some(audio) = result.audio.first() {
        if audio.codec != "aac"
            || audio.channels != Some(2)
            || audio.sample_rate.as_deref() != Some("48000")
        {
            return Err("Rendered audio does not match the AAC stereo preset".into());
        }
        if let Some(expected) = plan.audio_span {
            let start = seconds(audio.start_time.as_deref())
                .ok_or("Rendered audio has no start timestamp")?;
            let duration = audio
                .duration_seconds
                .ok_or("Rendered audio has no stream duration")?;
            if (start - expected.start).abs() > 0.05
                || (start + duration - expected.end).abs() > 0.1
            {
                return Err("Rendered audio timing is out of sync".into());
            }
        }
    }
    Ok(result)
}
pub fn decode_check(
    engine: &Path,
    output: &Path,
    cancel: impl Fn() -> bool,
    progress: impl Fn(video_engine::Progress) + Send + 'static,
) -> Result<(), String> {
    let args: Vec<&OsStr> = [
        "-nostdin",
        "-v",
        "error",
        "-xerror",
        "-max_alloc",
        "268435456",
        "-threads",
        "2",
        "-protocol_whitelist",
        "file,pipe",
        "-format_whitelist",
        "mov",
        "-i",
    ]
    .into_iter()
    .map(OsStr::new)
    .chain([output.as_os_str()])
    .chain(
        [
            "-map",
            "0:v:0",
            "-map",
            "0:a?",
            "-progress",
            "pipe:1",
            "-nostats",
            "-f",
            "null",
            "-",
        ]
        .into_iter()
        .map(OsStr::new),
    )
    .collect();
    video_engine::run_progress(
        &video_engine::tool(engine, "ffmpeg")?,
        &args,
        Duration::from_secs(21600),
        cancel,
        progress,
    )
}
