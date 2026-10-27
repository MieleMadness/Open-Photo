//! Synthetic media only. These tests exercise the exact engine bundled by the installer.
use crate::{video_engine, video_probe};
use sha2::{Digest, Sha256};
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "openphoto-video-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn engine() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/video-engine")
}
fn ffmpeg(args: &[&str], destination: &Path) {
    let mut owned: Vec<OsString> = args.iter().map(OsString::from).collect();
    owned.push(destination.into());
    let borrowed: Vec<&OsStr> = owned.iter().map(OsString::as_os_str).collect();
    video_engine::run(
        &video_engine::tool(&engine(), "ffmpeg").unwrap(),
        &borrowed,
        Duration::from_secs(30),
    )
    .unwrap();
}
fn fingerprint(path: &Path) -> Vec<u8> {
    Sha256::digest(fs::read(path).unwrap()).to_vec()
}

#[test]
fn inspects_real_unicode_phone_video_with_and_without_audio_without_modification() {
    let fixture = Fixture::new();
    for audio in [false, true] {
        let path = fixture.0.join(if audio {
            "旅行 clip with audio.mp4"
        } else {
            "旅行 silent clip.mp4"
        });
        let mut args = vec![
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=64x48:rate=30:duration=1.5",
        ];
        if audio {
            args.extend([
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=1.5",
            ]);
        }
        args.extend([
            "-c:v",
            "libopenh264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-t",
            "1.5",
        ]);
        ffmpeg(&args, &path);
        let before = fingerprint(&path);
        let info = video_probe::probe(&engine(), &path, || false).unwrap();
        assert_eq!((info.width, info.height), (64, 48));
        assert_eq!(info.codec, "h264");
        assert!((info.duration_seconds - 1.5).abs() < 0.2);
        assert_eq!(info.audio.len(), usize::from(audio));
        assert_eq!(fingerprint(&path), before);
        if audio {
            let rotated = fixture.0.join("rotated.mov");
            ffmpeg(
                &[
                    "-nostdin",
                    "-v",
                    "error",
                    "-display_rotation",
                    "90",
                    "-i",
                    path.to_str().unwrap(),
                    "-c",
                    "copy",
                ],
                &rotated,
            );
            let rotated_info = video_probe::probe(&engine(), &rotated, || false).unwrap();
            assert_eq!(rotated_info.rotation_degrees.abs(), 90.0);
        }
    }
}

#[test]
fn recognizes_hdr_and_variable_frame_timing_in_real_container_metadata() {
    let fixture = Fixture::new();
    let source = fixture.0.join("source.mp4");
    ffmpeg(
        &[
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=64x48:rate=30:duration=1.5",
            "-c:v",
            "libopenh264",
        ],
        &source,
    );
    let hdr = fixture.0.join("hdr.mp4");
    // OpenH264 does not write the requested color-transfer VUI; set it explicitly.
    ffmpeg(
        &[
            "-nostdin",
            "-v",
            "error",
            "-i",
            source.to_str().unwrap(),
            "-c",
            "copy",
            "-bsf:v",
            "h264_metadata=colour_primaries=9:transfer_characteristics=16:matrix_coefficients=9",
        ],
        &hdr,
    );
    assert!(video_probe::probe(&engine(), &hdr, || false).unwrap().hdr);
    let vfr = fixture.0.join("variable.mp4");
    ffmpeg(
        &[
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=64x48:rate=30:duration=2",
            "-vf",
            "select='not(mod(n,3))+not(mod(n,7))'",
            "-fps_mode",
            "vfr",
            "-c:v",
            "libopenh264",
        ],
        &vfr,
    );
    assert!(
        video_probe::probe(&engine(), &vfr, || false)
            .unwrap()
            .frame_rate_may_vary
    );
}

#[test]
fn rejects_corrupt_or_playlist_inputs_and_reaps_timed_out_or_cancelled_tools() {
    let fixture = Fixture::new();
    let bad = fixture.0.join("broken.mp4");
    fs::write(&bad, b"not a video").unwrap();
    assert!(video_probe::probe(&engine(), &bad, || false).is_err());
    let playlist = fixture.0.join("playlist.mp4");
    fs::write(
        &playlist,
        b"#EXTM3U\n#EXTINF:1,\nhttps://example.invalid/not-allowed.ts\n",
    )
    .unwrap();
    assert!(video_probe::probe(&engine(), &playlist, || false).is_err());
    let ffmpeg = video_engine::tool(&engine(), "ffmpeg").unwrap();
    let args: Vec<&OsStr> = vec![
        "-nostdin",
        "-v",
        "error",
        "-re",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=64x48:rate=30",
        "-f",
        "null",
        "-",
    ]
    .into_iter()
    .map(OsStr::new)
    .collect();
    assert!(
        video_engine::run(&ffmpeg, &args, Duration::from_millis(100))
            .unwrap_err()
            .contains("timed out")
    );
    assert!(
        video_engine::run_until(&ffmpeg, &args, Duration::from_secs(5), || true)
            .unwrap_err()
            .contains("superseded")
    );
}
