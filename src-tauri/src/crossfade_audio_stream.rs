//! Test-only streaming AAC packet inspection. Unlike JSON packet collection,
//! this holds constant memory even at the two-hour crossfade project limit.
use std::{
    ffi::OsStr,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Default)]
struct PacketClock {
    total: u64,
    skipped: u64,
    discarded: u64,
    packets: u64,
}

impl PacketClock {
    fn accept(&mut self, raw: &[u8], expected: u64) -> Result<(), String> {
        let text = std::str::from_utf8(raw)
            .map_err(|_| "Crossfade AAC packet line is not UTF-8.")?
            .trim();
        let mut duration = None;
        let mut side_type = None;
        let mut skip = 0_u64;
        let mut discard = 0_u64;
        for field in text.split('|') {
            if let Some(value) = field.strip_prefix("duration=") {
                if duration.is_some() {
                    return Err("Crossfade AAC packet duration is duplicated.".into());
                }
                duration = value.parse::<u64>().ok();
            } else if let Some(value) = field.strip_prefix("side_datum/skip_samples:side_data_type=") {
                side_type = Some(value);
            } else if let Some(value) = field.strip_prefix("side_datum/skip_samples:skip_samples=") {
                skip = value
                    .parse()
                    .map_err(|_| "Crossfade AAC priming field is invalid.")?;
            } else if let Some(value) =
                field.strip_prefix("side_datum/skip_samples:discard_padding=")
            {
                discard = value
                    .parse()
                    .map_err(|_| "Crossfade AAC discard field is invalid.")?;
            }
        }
        let duration = duration.ok_or("Crossfade AAC packet has no valid duration.")?;
        if duration == 0 || duration > 8192 {
            return Err("Crossfade AAC packet duration exceeds its allowed range.".into());
        }
        self.total = self
            .total
            .checked_add(duration)
            .ok_or("Crossfade AAC packet count overflow.")?;
        let ceiling = expected
            .checked_add(16_384)
            .ok_or("Crossfade AAC maximum presentation overflow.")?;
        if self.total > ceiling {
            return Err("Crossfade AAC packet clock exceeds the maximum expected samples.".into());
        }
        if side_type == Some("Skip Samples") {
            self.skipped = self
                .skipped
                .checked_add(skip)
                .ok_or("Crossfade AAC priming overflow.")?;
            self.discarded = self
                .discarded
                .checked_add(discard)
                .ok_or("Crossfade AAC discard padding overflow.")?;
        }
        self.packets += 1;
        Ok(())
    }

    fn finish(self, expected: u64) -> Result<(), String> {
        if self.packets == 0 || self.skipped > 8192 || self.discarded > 8192 {
            return Err("Crossfade AAC packets or padding are invalid.".into());
        }
        let actual = self
            .total
            .checked_sub(self.skipped)
            .and_then(|value| value.checked_sub(self.discarded))
            .ok_or("Crossfade AAC padding exceeds decoded samples.")?;
        if actual != expected {
            return Err("Crossfade AAC presentation samples disagree with the frame clock.".into());
        }
        Ok(())
    }
}

/// Runs the existing cancellable bounded native process runner and streams
/// one compact FFprobe packet record at a time (maximum 4096 bytes per line).
pub(crate) fn verify_streaming(
    engine: &Path,
    output: &Path,
    expected: u64,
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    if expected == 0 || expected > 48_000 * 7200 || cancel() {
        return Err("Crossfade AAC presentation request is invalid or cancelled.".into());
    }
    let ffprobe = crate::video_engine::tool(engine, "ffprobe")?;
    let stream_args: &[&OsStr] = &[
        "-v".as_ref(),
        "error".as_ref(),
        "-protocol_whitelist".as_ref(),
        "file,pipe".as_ref(),
        "-format_whitelist".as_ref(),
        "mov".as_ref(),
        "-select_streams".as_ref(),
        "a:0".as_ref(),
        "-show_streams".as_ref(),
        "-show_entries".as_ref(),
        "stream=time_base".as_ref(),
        "-of".as_ref(),
        "compact=p=0:nk=0".as_ref(),
        output.as_os_str(),
    ];
    let stream_info =
        crate::video_engine::run_until(&ffprobe, stream_args, Duration::from_secs(120), &cancel)?;
    let text = std::str::from_utf8(&stream_info)
        .map_err(|_| "Crossfade AAC stream timebase is not UTF-8.")?;
    if text.lines().filter(|line| line.contains("time_base=")).count() != 1
        || !text
            .lines()
            .any(|line| line.split('|').any(|field| field == "time_base=1/48000"))
    {
        return Err("Crossfade AAC stream must have exactly one 48kHz timebase.".into());
    }
    let args: &[&OsStr] = &[
        "-v".as_ref(),
        "error".as_ref(),
        "-protocol_whitelist".as_ref(),
        "file,pipe".as_ref(),
        "-format_whitelist".as_ref(),
        "mov".as_ref(),
        "-select_streams".as_ref(),
        "a:0".as_ref(),
        "-show_packets".as_ref(),
        "-show_entries".as_ref(),
        "packet=duration,side_data_list".as_ref(),
        "-of".as_ref(),
        "compact=p=0:nk=0".as_ref(),
        output.as_os_str(),
    ];
    let state = Arc::new(Mutex::new(PacketClock::default()));
    let writer = Arc::clone(&state);
    crate::video_engine::run_lines(
        &ffprobe,
        args,
        Duration::from_secs(600),
        &cancel,
        move |line| {
            writer
                .lock()
                .map_err(|_| "Crossfade AAC packet state unavailable.")?
                .accept(line, expected)
        },
    )?;
    if cancel() {
        return Err("Crossfade AAC presentation verification cancelled.".into());
    }
    let state = Arc::try_unwrap(state)
        .map_err(|_| "Crossfade AAC packet state is still shared.")?
        .into_inner()
        .map_err(|_| "Crossfade AAC packet state unavailable.")?;
    state.finish(expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streams_packet_samples_and_rejects_priming_or_tampering() {
        let mut state = PacketClock::default();
        state
            .accept(b"duration=1024|side_datum/skip_samples:side_data_type=Skip Samples|side_datum/skip_samples:skip_samples=1024|side_datum/skip_samples:discard_padding=0\n", 1600)
            .unwrap();
        state.accept(b"duration=1024\n", 1600).unwrap();
        state.accept(b"duration=576\n", 1600).unwrap();
        state.finish(1600).unwrap();
        let mut broken = PacketClock::default();
        assert!(broken.accept(b"duration=bogus\n", 48000).is_err());
        assert!(broken.accept(b"duration=8193\n", 48000).is_err());
        broken.accept(b"duration=1024\n", 48000).unwrap();
        assert!(broken.finish(48000).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn verifies_bundled_two_second_aac_in_streaming_mode() {
        let engine = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources/video-engine");
        let dir = std::env::temp_dir().join(format!(
            "openphoto-streaming-aac-{}",
            crate::video_project::id()
        ));
        std::fs::create_dir(&dir).unwrap();
        let output = dir.join("aac.mp4");
        let args: &[&OsStr] = &[
            "-nostdin".as_ref(),
            "-v".as_ref(),
            "error".as_ref(),
            "-f".as_ref(),
            "lavfi".as_ref(),
            "-i".as_ref(),
            "anullsrc=r=48000:cl=stereo".as_ref(),
            "-t".as_ref(),
            "2".as_ref(),
            "-c:a".as_ref(),
            "aac".as_ref(),
            "-ar".as_ref(),
            "48000".as_ref(),
            "-ac".as_ref(),
            "2".as_ref(),
            "-f".as_ref(),
            "mp4".as_ref(),
            output.as_os_str(),
        ];
        let ffmpeg = crate::video_engine::tool(&engine, "ffmpeg").unwrap();
        crate::video_engine::run_until(&ffmpeg, args, Duration::from_secs(90), || false)
            .unwrap();
        verify_streaming(&engine, &output, 96_000, || false).unwrap();
        assert!(verify_streaming(&engine, &output, 96_001, || false).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
