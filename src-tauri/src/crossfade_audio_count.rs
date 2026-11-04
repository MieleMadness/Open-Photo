//! Experimental, test-only exact decoded AAC sample count inspection.
//! The public video editor still uses its existing verified export pipeline.
use std::{ffi::OsStr, path::Path, time::Duration};

/// Parse one decoded audio frame's sample count per line, rejecting overflow,
/// malformed metadata and any final clock mismatch.
pub(crate) fn sum_decoded_samples(lines: &[u8], expected: u64) -> Result<u64, String> {
    if expected == 0 || expected > 48_000 * 7200 {
        return Err("Crossfade expected audio sample clock is out of bounds.".into());
    }
    let decoded = std::str::from_utf8(lines)
        .map_err(|_| "Crossfade audio frame metadata is not UTF-8.")?;
    let mut sum = 0_u64;
    let mut frames = 0_u64;
    for line in decoded.lines() {
        let value = line
            .trim()
            .parse::<u64>()
            .map_err(|_| "Crossfade audio frame sample count is invalid.")?;
        if value == 0 || value > 8192 {
            return Err("Crossfade decoded audio frame size is invalid.".into());
        }
        sum = sum
            .checked_add(value)
            .ok_or("Crossfade decoded audio sample count overflow.")?;
        frames += 1;
        if sum > expected {
            return Err("Crossfade output has too many decoded audio samples.".into());
        }
    }
    if frames == 0 || sum != expected {
        return Err("Crossfade audio sample count differs from its frame clock.".into());
    }
    Ok(sum)
}

/// FFprobe enumerates decoded audio frames (nb_samples), not AAC packet count
/// or a rounded container duration. The process runner bounds stdout at 4 MiB.
pub(crate) fn verify_decoded_samples(
    engine: &Path,
    output: &Path,
    expected: u64,
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    if cancel() {
        return Err("Crossfade decoded audio verification cancelled.".into());
    }
    let tool = crate::video_engine::tool(engine, "ffprobe")?;
    let args: &[&OsStr] = &[
        OsStr::new("-v"),
        OsStr::new("error"),
        OsStr::new("-max_alloc"),
        OsStr::new("268435456"),
        OsStr::new("-protocol_whitelist"),
        OsStr::new("file,pipe"),
        OsStr::new("-format_whitelist"),
        OsStr::new("mov"),
        OsStr::new("-select_streams"),
        OsStr::new("a:0"),
        OsStr::new("-show_frames"),
        OsStr::new("-show_entries"),
        OsStr::new("frame=nb_samples"),
        OsStr::new("-of"),
        OsStr::new("csv=p=0"),
        output.as_os_str(),
    ];
    let result = crate::video_engine::run_until(&tool, args, Duration::from_secs(120), &cancel)?;
    if cancel() {
        return Err("Crossfade decoded audio verification cancelled.".into());
    }
    sum_decoded_samples(&result, expected)?;
    Ok(())
}

/// AAC can decode padded samples at its end. Compute exact *presentation*
/// samples from packet durations minus priming and discard padding, rather
/// than treating the raw decoded frame tally as playback duration.
pub(crate) fn presentation_samples_from_probe(
    data: &[u8],
    expected: u64,
) -> Result<u64, String> {
    if expected == 0 || expected > 48_000 * 7200 {
        return Err("Crossfade expected presentation clock is out of bounds.".into());
    }
    let doc: serde_json::Value = serde_json::from_slice(data)
        .map_err(|_| "Crossfade audio packet response is invalid JSON.")?;
    let streams = doc["streams"]
        .as_array()
        .ok_or("Crossfade packet response has no audio stream metadata.")?;
    if streams.len() != 1 || streams[0]["time_base"].as_str() != Some("1/48000") {
        return Err("Crossfade AAC packet clock must be 48kHz.".into());
    }
    let packets = doc["packets"]
        .as_array()
        .ok_or("Crossfade audio packet list is missing.")?;
    if packets.is_empty() {
        return Err("Crossfade output has no audio packets.".into());
    }
    let mut total = 0_u64;
    let mut skipped = 0_u64;
    let mut discarded = 0_u64;
    for packet in packets {
        let duration = packet["duration"]
            .as_u64()
            .or_else(|| packet["duration"].as_str()?.parse::<u64>().ok())
            .ok_or("Crossfade AAC packet duration is invalid.")?;
        if duration == 0 || duration > 8192 {
            return Err("Crossfade AAC packet duration exceeds its allowed range.".into());
        }
        total = total
            .checked_add(duration)
            .ok_or("Crossfade AAC presentation sample overflow.")?;
        if let Some(side_data) = packet["side_data_list"].as_array() {
            for side in side_data {
                if side["side_data_type"].as_str() == Some("Skip Samples") {
                    skipped = skipped
                        .checked_add(side["skip_samples"].as_u64().unwrap_or(0))
                        .ok_or("Crossfade AAC priming count overflow.")?;
                    discarded = discarded
                        .checked_add(side["discard_padding"].as_u64().unwrap_or(0))
                        .ok_or("Crossfade AAC discard count overflow.")?;
                }
            }
        }
    }
    if skipped > 8192 || discarded > 8192 {
        return Err("Crossfade AAC priming or discard padding is excessive.".into());
    }
    let presentation = total
        .checked_sub(skipped)
        .and_then(|value| value.checked_sub(discarded))
        .ok_or("Crossfade AAC packet corrections exceed its sample count.")?;
    if presentation != expected {
        return Err("Crossfade AAC presentation samples disagree with video frame clock.".into());
    }
    Ok(presentation)
}

/// Inspect the packet presentation clock using ffprobe's bounded output. This
/// is a conservative test-only guard: very long files can exceed the runner's
/// 4 MiB diagnostic cap and are rejected until streaming inspection exists.
pub(crate) fn verify_presentation_samples(
    engine: &Path,
    output: &Path,
    expected: u64,
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    if cancel() {
        return Err("Crossfade AAC presentation verification cancelled.".into());
    }
    let tool = crate::video_engine::tool(engine, "ffprobe")?;
    let args: &[&OsStr] = &[
        OsStr::new("-v"),
        OsStr::new("error"),
        OsStr::new("-max_alloc"),
        OsStr::new("268435456"),
        OsStr::new("-protocol_whitelist"),
        OsStr::new("file,pipe"),
        OsStr::new("-format_whitelist"),
        OsStr::new("mov"),
        OsStr::new("-select_streams"),
        OsStr::new("a:0"),
        OsStr::new("-show_packets"),
        OsStr::new("-show_streams"),
        OsStr::new("-show_entries"),
        OsStr::new("stream=time_base:packet=duration,side_data_list"),
        OsStr::new("-of"),
        OsStr::new("json"),
        output.as_os_str(),
    ];
    let result = crate::video_engine::run_until(&tool, args, Duration::from_secs(120), &cancel)?;
    if cancel() {
        return Err("Crossfade AAC presentation verification cancelled.".into());
    }
    presentation_samples_from_probe(&result, expected)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_clock_corrects_encoder_priming_and_partial_final_packet() {
        let doc = serde_json::json!({
            "streams": [{"time_base": "1/48000"}],
            "packets": [
                {"duration": 1024, "side_data_list": [
                    {"side_data_type": "Skip Samples", "skip_samples": 1024, "discard_padding": 0}
                ]},
                {"duration": 1024},
                {"duration": 768}
            ]
        });
        let encoded = serde_json::to_vec(&doc).unwrap();
        assert_eq!(presentation_samples_from_probe(&encoded, 1792).unwrap(), 1792);
        assert!(presentation_samples_from_probe(&encoded, 2048).is_err());
        let mut invalid = doc;
        invalid["streams"][0]["time_base"] = "1/1000".into();
        assert!(presentation_samples_from_probe(&serde_json::to_vec(&invalid).unwrap(), 1792).is_err());
    }

    #[test]
    fn validates_exact_sample_count_and_rejects_padding_or_truncation() {
        assert_eq!(sum_decoded_samples(b"1024\n1024\n512\n", 2560).unwrap(), 2560);
        assert!(sum_decoded_samples(b"1024\n1024\n512\n", 2048).is_err());
        assert!(sum_decoded_samples(b"1024\n1024\n512\n", 3072).is_err());
        assert!(sum_decoded_samples(b"1024\n1024\n512\n", 0).is_err());
        assert!(sum_decoded_samples(b"1024\n0\n512\n", 1536).is_err());
        assert!(sum_decoded_samples(b"1024\ninvalid\n", 1024).is_err());
        assert!(sum_decoded_samples(b"", 1024).is_err());
        assert!(sum_decoded_samples(b"8193\n", 8193).is_err());
        assert!(sum_decoded_samples(b"\xff\n", 1024).is_err());
    }
}
