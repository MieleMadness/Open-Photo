//! Render saved edits with the same validated graph that export will consume.
use crate::{
    video_engine,
    video_preview::PreviewAssets,
    video_probe,
    video_project::{self, Project},
};
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::Path,
    time::Duration,
};
pub(crate) fn build(
    engine: &Path,
    root: &str,
    p: &Project,
    directory: &Path,
    cancel: impl Fn() -> bool,
) -> Result<PreviewAssets, String> {
    let source = video_project::source(root, p, &cancel)?;
    let info = video_probe::probe(engine, &source, &cancel)?;
    if info.hdr {
        return Err(
            "HDR edit previews require tested tone mapping. Original playback remains available."
                .into(),
        );
    }
    if info.width != p.asset.info.width
        || info.height != p.asset.info.height
        || (info.duration_seconds - p.asset.info.duration_seconds).abs() > 0.001
        || info.time_base != p.asset.info.time_base
    {
        return Err("Project source metadata does not match the verified video".into());
    }
    let c = &p.clips[0];
    let start = c.source_in.seconds()?;
    let length = c.source_out.seconds()? - start;
    let (filter, width, height) = video_project::filters(p, true)?;
    let output = directory.join("edited-preview.mp4");
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
        format!("{length:.9}").into(),
        "-map".into(),
        format!("0:{}", info.video_stream).into(),
        "-vf".into(),
        filter.into(),
    ]);
    if !c.edits.muted {
        if let Some(audio) = info.audio.first() {
            args.extend([
                "-map".into(),
                format!("0:{}", audio.stream).into(),
                "-af".into(),
                format!("volume={:.4}", f64::from(c.edits.gain_percent) / 100.0).into(),
            ]);
        }
    }
    args.extend(
        [
            "-fps_mode",
            "vfr",
            "-c:v",
            "libopenh264",
            "-b:v",
            "2000k",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            "-ac",
            "2",
            "-map_metadata",
            "-1",
            "-movflags",
            "+faststart",
            "-fs",
            "536870912",
        ]
        .into_iter()
        .map(Into::into),
    );
    args.push(output.clone().into());
    let args: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    video_engine::run_until(
        &video_engine::tool(engine, "ffmpeg")?,
        &args,
        Duration::from_secs(1800),
        &cancel,
    )?;
    let result = video_probe::probe(engine, &output, &cancel)?;
    let tolerance = info
        .frame_rate
        .as_deref()
        .and_then(|s| s.split_once('/'))
        .and_then(|(a, b)| Some(b.parse::<f64>().ok()? / a.parse::<f64>().ok()?))
        .unwrap_or(0.05)
        .max(0.05)
        * 2.0;
    if (result.duration_seconds - length).abs() > tolerance
        || result.width != width
        || result.height != height
        || result.rotation_degrees.abs() > 0.01
        || fs::metadata(&output).map_err(|e| e.to_string())?.len() >= 512 * 1024 * 1024
    {
        return Err("Edited preview failed duration, geometry, or size validation".into());
    }
    // Verify again so a source replaced during decoding cannot silently become a valid preview.
    video_project::source(root, p, &cancel)?;
    let mut warnings = vec!["Edited preview only. Video export is not implemented yet.".into()];
    if info.audio.len() > 1 {
        warnings.push("Preview uses the first audio track, converted to stereo.".into());
    }
    Ok(PreviewAssets {
        source: output.to_string_lossy().into_owned(),
        proxy: true,
        info: result,
        filmstrip: vec![],
        warnings,
    })
}
