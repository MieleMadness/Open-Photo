//! Independent stream spans; a longer audio track must not hide missing video.
use crate::{video_engine, video_probe::VideoInfo};
use std::{
    ffi::OsStr,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub start: f64,
    pub end: f64,
}
pub fn number(value: Option<&str>) -> Option<f64> {
    value?.parse().ok().filter(|v: &f64| v.is_finite())
}
pub fn frame_interval(info: &VideoInfo) -> f64 {
    info.frame_rate
        .as_deref()
        .and_then(|s| s.split_once('/'))
        .and_then(|(a, b)| Some(b.parse::<f64>().ok()? / a.parse::<f64>().ok()?))
        .filter(|n| n.is_finite() && *n > 0.0)
        .unwrap_or(0.05)
}
fn packet_span(
    engine: &Path,
    path: &Path,
    stream: usize,
    fallback_duration: f64,
    cancel: impl Fn() -> bool,
) -> Result<Option<Span>, String> {
    let state = Arc::new(Mutex::new(None::<Span>));
    let copy = state.clone();
    let stream = stream.to_string();
    let args: Vec<&OsStr> = vec![
        "-v".as_ref(),
        "error".as_ref(),
        "-max_alloc".as_ref(),
        "268435456".as_ref(),
        "-protocol_whitelist".as_ref(),
        "file,pipe".as_ref(),
        "-format_whitelist".as_ref(),
        "mov,matroska,webm,avi".as_ref(),
        "-select_streams".as_ref(),
        stream.as_ref(),
        "-show_packets".as_ref(),
        "-show_entries".as_ref(),
        "packet=pts_time,duration_time".as_ref(),
        "-of".as_ref(),
        "compact=p=0:nk=0".as_ref(),
        path.as_os_str(),
    ];
    video_engine::run_lines(
        &video_engine::tool(engine, "ffprobe")?,
        &args,
        Duration::from_secs(600),
        cancel,
        move |line| {
            let text = String::from_utf8_lossy(line);
            let mut pts = None;
            let mut duration = None;
            for field in text.trim().split('|') {
                if let Some((key, value)) = field.split_once('=') {
                    match key {
                        "pts_time" => pts = number(Some(value)),
                        "duration_time" => duration = number(Some(value)),
                        _ => {}
                    }
                }
            }
            if let Some(start) = pts {
                let end = start + duration.filter(|d| *d > 0.0).unwrap_or(fallback_duration);
                let mut state = copy.lock().map_err(|e| e.to_string())?;
                *state = Some(match *state {
                    Some(old) => Span {
                        start: old.start.min(start),
                        end: old.end.max(end),
                    },
                    None => Span { start, end },
                });
            }
            Ok(())
        },
    )?;
    let result = *state.lock().map_err(|e| e.to_string())?;
    Ok(result)
}
fn raw_span(
    engine: &Path,
    path: &Path,
    stream: usize,
    timing: (Option<&str>, Option<f64>, f64),
    origin: f64,
    cancel: impl Fn() -> bool,
) -> Result<Option<Span>, String> {
    let (start, duration, fallback) = timing;
    if let Some(duration) = duration.filter(|d| d.is_finite() && *d > 0.0) {
        let start = number(start).unwrap_or(origin);
        return Ok(Some(Span {
            start,
            end: start + duration,
        }));
    }
    Ok(
        packet_span(engine, path, stream, fallback, cancel)?.map(|span| Span {
            start: number(start).unwrap_or(span.start),
            end: span.end,
        }),
    )
}
fn intersect(span: Span, origin: f64, start: f64, duration: f64) -> Option<Span> {
    let range = Span {
        start: (span.start - origin - start).max(0.0),
        end: (span.end - origin - start).min(duration),
    };
    (range.end > range.start).then_some(range)
}
pub fn source_spans(
    engine: &Path,
    path: &Path,
    info: &VideoInfo,
    start: f64,
    duration: f64,
    cancel: impl Fn() -> bool,
) -> Result<(Span, Option<Span>), String> {
    let origin = number(info.format_start_time.as_deref()).unwrap_or(0.0);
    let video = raw_span(
        engine,
        path,
        info.video_stream,
        (
            info.start_time.as_deref(),
            info.video_duration_seconds,
            frame_interval(info),
        ),
        origin,
        &cancel,
    )?
    .and_then(|s| intersect(s, origin, start, duration))
    .ok_or("The kept range contains no video samples")?;
    let audio = if let Some(audio) = info.audio.first() {
        let interval = audio
            .sample_rate
            .as_deref()
            .and_then(|r| r.parse::<f64>().ok())
            .filter(|r| *r > 0.0)
            .map(|r| 1024.0 / r)
            .unwrap_or(0.022);
        let span = raw_span(
            engine,
            path,
            audio.stream,
            (
                audio.start_time.as_deref(),
                audio.duration_seconds,
                interval,
            ),
            origin,
            &cancel,
        )?
        .ok_or("Cannot establish source audio timing")?;
        intersect(span, origin, start, duration)
    } else {
        None
    };
    Ok((video, audio))
}
