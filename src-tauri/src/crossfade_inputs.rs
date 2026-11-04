//! Test-only normalized video/audio stream contract for future crossfade execution.
//! Actual source fingerprint and scratch ownership gates are separate.
use crate::video_probe::VideoInfo;

/// All parts must be normalized to one project canvas and exact clock before
/// FFmpeg receives them. Audio-less originals must already carry PCM silence.
pub(crate) fn validate_normalized_parts(
    parts: &[VideoInfo],
    expected_frames: &[u64],
    fps: u32,
) -> Result<(), String> {
    if !matches!(fps, 30 | 60)
        || parts.len() < 2
        || parts.len() > 200
        || parts.len() != expected_frames.len()
    {
        return Err("Crossfade normalized parts require 2–200 matching clips.".into());
    }
    let first = &parts[0];
    if first.width == 0 || first.height == 0 {
        return Err("Crossfade normalized canvas is invalid.".into());
    }
    for (info, frames) in parts.iter().zip(expected_frames.iter()) {
        let duration = *frames as f64 / f64::from(fps);
        if *frames < 2
            || info.frame_count != Some(*frames)
            || info.width != first.width
            || info.height != first.height
            || info.codec != "h264"
            || info.pixel_format.as_deref() != Some("yuv420p")
            || info.rotation_degrees.abs() > 0.01
            || info.hdr
            || info.frame_rate_may_vary
            || info.frame_rate.as_deref().and_then(|value| {
                let (a, b) = value.split_once('/')?;
                Some(a.parse::<u64>().ok()? == u64::from(fps).checked_mul(b.parse::<u64>().ok()?)?)
            }) != Some(true)
            || (info.duration_seconds - duration).abs() > 1.0 / f64::from(fps)
            || info.audio.len() != 1
        {
            return Err("Crossfade part does not match normalized video clock or canvas.".into());
        }
        let audio = &info.audio[0];
        if audio.codec != "pcm_f32le"
            || audio.sample_rate.as_deref() != Some("48000")
            || audio.channels != Some(2)
            || audio
                .duration_seconds
                .is_some_and(|seconds| (seconds - duration).abs() > 0.05)
        {
            return Err(
                "Crossfade part must contain 48kHz stereo PCM with matching duration.".into(),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video_probe;

    fn part(frames: u64) -> VideoInfo {
        let json = serde_json::json!({
            "streams": [
                {
                    "index": 0,
                    "codec_type": "video",
                    "codec_name": "h264",
                    "width": 96,
                    "height": 64,
                    "pix_fmt": "yuv420p",
                    "avg_frame_rate": "30/1",
                    "r_frame_rate": "30/1",
                    "nb_frames": frames.to_string(),
                    "duration": (frames as f64 / 30.0).to_string()
                },
                {
                    "index": 1,
                    "codec_type": "audio",
                    "codec_name": "pcm_f32le",
                    "sample_rate": "48000",
                    "channels": 2,
                    "channel_layout": "stereo",
                    "duration": (frames as f64 / 30.0).to_string()
                }
            ],
            "format": {
                "duration": (frames as f64 / 30.0).to_string()
            }
        });
        video_probe::parse_probe(json.to_string().as_bytes()).unwrap()
    }

    #[test]
    fn accepts_exact_two_part_normalization_including_silent_originals() {
        let parts = [part(60), part(90)];
        assert!(validate_normalized_parts(&parts, &[60, 90], 30).is_ok());
    }

    #[test]
    fn rejects_wrong_video_clock_geometry_or_audio_normalization() {
        let first = part(60);
        let second = part(90);
        assert!(
            validate_normalized_parts(&[first.clone(), second.clone()], &[60, 91], 30).is_err()
        );
        let mut wrong_canvas = second.clone();
        wrong_canvas.width = 128;
        assert!(validate_normalized_parts(&[first.clone(), wrong_canvas], &[60, 90], 30).is_err());
        let mut missing_audio = second.clone();
        missing_audio.audio.clear();
        assert!(validate_normalized_parts(&[first.clone(), missing_audio], &[60, 90], 30).is_err());
        let mut wrong_rate = second.clone();
        wrong_rate.audio[0].sample_rate = Some("44100".into());
        assert!(validate_normalized_parts(&[first.clone(), wrong_rate], &[60, 90], 30).is_err());
        let mut varying_rate = second.clone();
        varying_rate.frame_rate_may_vary = true;
        assert!(validate_normalized_parts(&[first.clone(), varying_rate], &[60, 90], 30).is_err());
        let mut hdr = second;
        hdr.hdr = true;
        assert!(validate_normalized_parts(&[first, hdr], &[60, 90], 30).is_err());
    }
}
