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

#[cfg(test)]
mod tests {
    use super::*;

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
