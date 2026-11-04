//! Standalone Windows test: real audible tones must survive crossfade mixing.
//! Kept independent of the working hard-cut/dip preview and export pathways.
#![cfg(all(test, windows))]

use crate::{
    crossfade_clock::{self, Boundary, ClipClock},
    crossfade_graph, video_engine, video_probe, video_project,
};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn ffmpeg(engine: &Path, args: &[OsString]) {
    video_engine::run(
        &video_engine::tool(engine, "ffmpeg").unwrap(),
        &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
        Duration::from_secs(90),
    )
    .unwrap();
}

/// Calculate RMS over a centered 200-ms window of mono 48 kHz PCM audio.
fn rms_at(samples: &[f32], seconds: f64) -> f64 {
    let center = (seconds * 48000.0).round() as usize;
    let radius = 4800;
    let span = &samples[center - radius..center + radius];
    (span
        .iter()
        .map(|sample| f64::from(*sample).powi(2))
        .sum::<f64>()
        / span.len() as f64)
        .sqrt()
}

#[test]
fn bundled_ffmpeg_preserves_audible_audio_before_during_and_after_crossfade() {
    let engine = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/video-engine");
    let dir = std::env::temp_dir().join(format!("crossfade-audio-{}", video_project::id()));
    fs::create_dir(&dir).unwrap();
    let _fixture = Fixture(dir.clone());
    for (index, (color, frequency)) in [("red", 440), ("blue", 880)].iter().enumerate() {
        let args: Vec<OsString> = vec![
            "-nostdin".into(),
            "-v".into(),
            "error".into(),
            "-f".into(),
            "lavfi".into(),
            "-i".into(),
            format!("color=c={color}:s=96x64:r=30:d=2").into(),
            "-f".into(),
            "lavfi".into(),
            "-i".into(),
            format!("sine=frequency={frequency}:sample_rate=48000:duration=2").into(),
            "-t".into(),
            "2".into(),
            "-frames:v".into(),
            "60".into(),
            "-c:v".into(),
            "libopenh264".into(),
            "-b:v".into(),
            "150000".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
            "-c:a".into(),
            "pcm_f32le".into(),
            "-ac".into(),
            "2".into(),
            dir.join(format!("tone-{index}.mkv")).into(),
        ];
        ffmpeg(&engine, &args);
    }

    let clips = [
        ClipClock {
            id: "red",
            frames: 60,
        },
        ClipClock {
            id: "blue",
            frames: 60,
        },
    ];
    let edges = [Boundary {
        from_clip_id: "red",
        to_clip_id: "blue",
        frames: 15,
    }];
    let clock = crossfade_clock::compile(&clips, &edges).unwrap();
    let graph = crossfade_graph::plan(&clock, 30).unwrap();
    assert_eq!(graph.expected_frames, 105);
    let output = dir.join("mixed-tones.mp4");
    let args: Vec<OsString> = vec![
        "-nostdin".into(),
        "-v".into(),
        "error".into(),
        "-i".into(),
        dir.join("tone-0.mkv").into(),
        "-i".into(),
        dir.join("tone-1.mkv").into(),
        "-filter_complex".into(),
        graph.filter_complex.into(),
        "-map".into(),
        format!("[{}]", graph.video_output).into(),
        "-map".into(),
        format!("[{}]", graph.audio_output).into(),
        "-c:v".into(),
        "libopenh264".into(),
        "-b:v".into(),
        "200000".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        "aac".into(),
        "-ar".into(),
        "48000".into(),
        "-ac".into(),
        "2".into(),
        output.clone().into(),
    ];
    ffmpeg(&engine, &args);
    let info = video_probe::probe(&engine, &output, || false).unwrap();
    assert_eq!(info.frame_count, Some(105));
    assert!((info.duration_seconds - 3.5).abs() < 0.05);
    assert_eq!(info.audio.len(), 1);
    assert_eq!(info.audio[0].sample_rate.as_deref(), Some("48000"));
    let encoded_audio_seconds = info.audio[0].duration_seconds.unwrap_or(info.duration_seconds);
    assert!(
        (encoded_audio_seconds - 3.5).abs() < 0.05,
        "Encoded audio duration was {encoded_audio_seconds}s, expected 3.5s"
    );

    let decoded = dir.join("mixed-mono.f32le");
    let args: Vec<OsString> = vec![
        "-nostdin".into(),
        "-v".into(),
        "error".into(),
        "-i".into(),
        output.into(),
        "-map".into(),
        "0:a:0".into(),
        "-vn".into(),
        // Trim AAC packet padding from the *decoded fixture*, not the exported file.
        // This still fails if fewer than 168000 samples are actually available.
        "-af".into(),
        "atrim=end_sample=168000,asetpts=N/SR/TB".into(),
        "-ar".into(),
        "48000".into(),
        "-ac".into(),
        "1".into(),
        "-c:a".into(),
        "pcm_f32le".into(),
        "-f".into(),
        "f32le".into(),
        decoded.clone().into(),
    ];
    ffmpeg(&engine, &args);
    let bytes = fs::read(decoded).unwrap();
    assert_eq!(bytes.len() % 4, 0);
    let samples = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect::<Vec<_>>();
    assert_eq!(
        samples.len(),
        168_000,
        "Decoded and bounded fixture audio must cover the full 3.5-second timeline"
    );
    assert!(samples.iter().all(|sample| sample.is_finite()));
    for seconds in [0.5, 1.75, 3.0] {
        let rms = rms_at(&samples, seconds);
        assert!(rms > 0.02, "Audio disappeared at {seconds}s (RMS={rms})");
        assert!(rms < 0.5, "Unexpected audio peak at {seconds}s (RMS={rms})");
    }
}
