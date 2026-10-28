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

fn frame_times(path: &Path) -> Vec<f64> {
    let bytes = video_engine::run(
        &video_engine::tool(&engine(), "ffprobe").unwrap(),
        &[
            "-v".as_ref(),
            "error".as_ref(),
            "-select_streams".as_ref(),
            "v:0".as_ref(),
            "-show_entries".as_ref(),
            "frame=best_effort_timestamp_time".as_ref(),
            "-of".as_ref(),
            "json".as_ref(),
            path.as_os_str(),
        ],
        Duration::from_secs(20),
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["frames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            f["best_effort_timestamp_time"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap()
        })
        .collect()
}

#[test]
fn compatible_preview_preserves_vfr_frame_boundaries_and_audio_offsets() {
    let fixture = Fixture::new();
    let source = fixture.0.join("旅行 variable source.mp4");
    ffmpeg(
        &[
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=96x64:rate=30:duration=2",
            "-itsoffset",
            "0.2",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=1.8",
            "-vf",
            "select='not(mod(n,3))+not(mod(n,7))'",
            "-fps_mode",
            "vfr",
            "-c:v",
            "libopenh264",
            "-c:a",
            "aac",
        ],
        &source,
    );
    let before = fingerprint(&source);
    let output = fixture.0.join("derivative");
    fs::create_dir(&output).unwrap();
    let result =
        crate::video_preview::build_assets(&engine(), &source, &output, true, || false).unwrap();
    let proxy = Path::new(&result.source);
    let source_times = frame_times(&source);
    let proxy_times = frame_times(proxy);
    assert_eq!(source_times.len(), proxy_times.len());
    for (a, b) in source_times.iter().zip(&proxy_times) {
        assert!((a - b).abs() < 0.001, "{a} vs {b}");
    }
    assert_eq!(result.filmstrip.len(), 6);
    assert_eq!(result.info.audio.len(), 1);
    assert_eq!(fingerprint(&source), before);
    // Decode the result, not just its headers.
    ffmpeg(
        &[
            "-nostdin",
            "-v",
            "error",
            "-i",
            proxy.to_str().unwrap(),
            "-f",
            "null",
        ],
        &fixture.0.join("decoded.null"),
    );
    let probe_audio = |path: &Path| {
        let bytes = video_engine::run(
            &video_engine::tool(&engine(), "ffprobe").unwrap(),
            &[
                "-v".as_ref(),
                "error".as_ref(),
                "-show_streams".as_ref(),
                "-of".as_ref(),
                "json".as_ref(),
                path.as_os_str(),
            ],
            Duration::from_secs(10),
        )
        .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["streams"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["codec_type"] == "audio")
            .unwrap()["start_time"]
            .as_str()
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    assert!((probe_audio(&source) - probe_audio(proxy)).abs() < 0.03);
}

#[test]
fn portrait_silent_preview_applies_rotation_and_preserves_source() {
    let fixture = Fixture::new();
    let base = fixture.0.join("base.mp4");
    let source = fixture.0.join("portrait.mov");
    ffmpeg(
        &[
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=96x64:rate=24:duration=1",
            "-c:v",
            "libopenh264",
        ],
        &base,
    );
    ffmpeg(
        &[
            "-nostdin",
            "-v",
            "error",
            "-display_rotation",
            "90",
            "-i",
            base.to_str().unwrap(),
            "-c",
            "copy",
        ],
        &source,
    );
    let before = fingerprint(&source);
    let directory = fixture.0.join("preview");
    fs::create_dir(&directory).unwrap();
    let result =
        crate::video_preview::build_assets(&engine(), &source, &directory, true, || false).unwrap();
    let info = video_probe::probe(&engine(), Path::new(&result.source), || false).unwrap();
    assert!(info.height > info.width);
    assert_eq!(info.rotation_degrees, 0.0);
    assert!(info.audio.is_empty());
    assert_eq!(fingerprint(&source), before);
}

#[test]
fn preview_rejects_hdr_and_cancelled_work_without_changing_sources() {
    let fixture = Fixture::new();
    let base = fixture.0.join("base.mp4");
    let hdr = fixture.0.join("hdr.mp4");
    ffmpeg(
        &[
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=64x48:rate=24:duration=1",
            "-c:v",
            "libopenh264",
        ],
        &base,
    );
    ffmpeg(
        &[
            "-nostdin",
            "-v",
            "error",
            "-i",
            base.to_str().unwrap(),
            "-c",
            "copy",
            "-bsf:v",
            "h264_metadata=colour_primaries=9:transfer_characteristics=16:matrix_coefficients=9",
        ],
        &hdr,
    );
    let directory = fixture.0.join("preview");
    fs::create_dir(&directory).unwrap();
    let before = fingerprint(&hdr);
    assert!(
        crate::video_preview::build_assets(&engine(), &hdr, &directory, true, || false)
            .err()
            .unwrap()
            .contains("HDR")
    );
    assert!(fs::read_dir(&directory).unwrap().next().is_none());
    assert!(
        crate::video_preview::build_assets(&engine(), &base, &directory, true, || true).is_err()
    );
    assert_eq!(fingerprint(&hdr), before);
}

#[test]
fn filmstrip_uses_original_streaming_path_for_a_large_clip() {
    let fixture = Fixture::new();
    let source = fixture.0.join("large.mp4");
    ffmpeg(
        &[
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=64x48:rate=24:duration=1",
            "-c:v",
            "libopenh264",
        ],
        &source,
    );
    fs::OpenOptions::new()
        .write(true)
        .open(&source)
        .unwrap()
        .set_len(64 * 1024 * 1024)
        .unwrap();
    let original =
        crate::media_stream::validated_video(fixture.0.to_str().unwrap(), source.to_str().unwrap())
            .unwrap();
    let outside = fixture.0.join("subfolder");
    fs::create_dir(&outside).unwrap();
    assert!(crate::media_stream::validated_video(
        outside.to_str().unwrap(),
        source.to_str().unwrap()
    )
    .is_err());
    let directory = fixture.0.join("filmstrip");
    fs::create_dir(&directory).unwrap();
    let result =
        crate::video_preview::build_assets(&engine(), &original, &directory, false, || false)
            .unwrap();
    assert_eq!(result.source, original.to_string_lossy());
    assert!(!result.proxy);
    assert_eq!(result.filmstrip.len(), 6);
    assert!(!directory.join("preview.mp4").exists());
    assert!(result.filmstrip.iter().all(|f| f.image.len() < 90_000));
}

#[cfg(windows)]
#[test]
fn closing_the_process_job_terminates_native_work() {
    let mut child = std::process::Command::new(video_engine::tool(&engine(), "ffmpeg").unwrap())
        .args([
            "-nostdin",
            "-v",
            "error",
            "-re",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=64x48:rate=24",
            "-f",
            "null",
            "-",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let job = video_engine::ProcessJob::attach(&child).unwrap();
    drop(job);
    let started = std::time::Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if started.elapsed() > Duration::from_secs(2) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("Native video work survived its owning job");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
