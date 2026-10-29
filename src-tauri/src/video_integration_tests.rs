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

/// Run alone in release mode: the native peak recorder is process-wide.
#[cfg(windows)]
#[test]
#[ignore = "release acceptance: use scripts/accept-video.ps1"]
fn video_release_acceptance() {
    use crate::{
        video_export::{self, Control, Request, Stage},
        video_project::{self, VideoTime},
        video_render::ExportSize,
    };
    use std::{
        io::Write,
        sync::{Arc, Mutex},
        time::Instant,
    };
    if cfg!(debug_assertions) {
        panic!("Acceptance must use an optimized release build");
    }
    let report_path =
        std::env::var_os("OPENPHOTO_ACCEPTANCE_OUTPUT").expect("Set an acceptance report path");
    struct AcceptanceControl {
        started: Instant,
        stages: Mutex<Vec<(Stage, f64)>>,
    }
    impl Control for AcceptanceControl {
        fn cancelled(&self) -> bool {
            false
        }
        fn update(&self, stage: Stage, _: Option<f64>, _: u64) {
            let mut stages = self.stages.lock().unwrap();
            if stages.last().is_none_or(|(old, _)| *old != stage) {
                stages.push((stage, self.started.elapsed().as_secs_f64()));
            }
        }
        fn begin_publish(&self) -> Result<(), String> {
            self.update(Stage::Publishing, None, 0);
            Ok(())
        }
    }
    let fixture = Fixture::new();
    let source = fixture.0.join("1080p acceptance.mp4");
    ffmpeg(
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=1920x1080:rate=30:duration=8",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=8",
            "-c:v",
            "libopenh264",
            "-b:v",
            "8000k",
            "-c:a",
            "aac",
            "-ac",
            "2",
        ],
        &source,
    );
    let before = fingerprint(&source);
    let info = video_probe::probe(&engine(), &source, || false).unwrap();
    let mut project =
        video_project::new_project(fixture.0.to_str().unwrap(), &source, info).unwrap();
    let base = project.asset.info.time_base.clone().unwrap();
    project.clips[0].source_in = VideoTime::from_seconds(0.37, &base).unwrap();
    project.clips[0].source_out = VideoTime::from_seconds(7.61, &base).unwrap();
    project.clips[0].edits.crop.width = 9000;
    project.clips[0].edits.quarter_turns = 2;
    project.clips[0].edits.gain_percent = 75;
    let output = fixture.0.join("accepted.mp4");
    video_engine::reset_native_peak();
    let control = Arc::new(AcceptanceControl {
        started: Instant::now(),
        stages: Mutex::new(vec![]),
    });
    let report = video_export::export_file(
        Request {
            engine: &engine(),
            root: fixture.0.to_str().unwrap(),
            project: &project,
            destination: &output,
            size: ExportSize::Hd1080,
            acknowledge_first_audio: false,
        },
        control.clone(),
    )
    .unwrap();
    let elapsed = control.started.elapsed().as_secs_f64();
    let peak = video_engine::native_peak();
    assert!(
        peak > 0,
        "Windows native-process memory measurement was unavailable"
    );
    assert_eq!(fingerprint(&source), before);
    assert_eq!((report.width, report.height), (1920, 1080));
    assert!((report.duration_seconds - 7.24).abs() < 0.1);
    assert_eq!(fs::metadata(&output).unwrap().len(), report.bytes);
    assert_no_export_temporaries(&fixture.0);
    let stages = control.stages.lock().unwrap();
    for required in [
        Stage::VerifyingSource,
        Stage::Encoding,
        Stage::CheckingOutput,
        Stage::DecodingOutput,
        Stage::Publishing,
    ] {
        assert!(stages.iter().any(|(s, _)| *s == required));
    }
    let document = serde_json::json!({
        "schema_version":1,"app_version":env!("CARGO_PKG_VERSION"),"engine_version":video_engine::VERSION,
        "scope":"Optimized native export pipeline using synthetic media; excludes GUI/WebView, startup, seeking, fixture generation, and real-footage acceptance",
        "fixture":{"width":1920,"height":1080,"source_seconds":8,"fps":30,"audio_hz":48000,"trim_in_seconds":0.37,"trim_out_seconds":7.61},
        "export_wall_seconds":elapsed,"media_seconds_per_wall_second":7.24/elapsed,
        "maximum_individual_native_child_peak_working_set_bytes":peak,
        "memory_scope":"Maximum Windows-reported PeakWorkingSetSize of an individual owned FFmpeg/ffprobe process, sampled every 20 ms; not combined process-tree or whole-app memory",
        "stage_started_seconds":*stages,"output":{"width":report.width,"height":report.height,"seconds":report.duration_seconds,"bytes":report.bytes},
        "checks":{"source_unchanged":true,"stream_validation":true,"full_decode":true,"publication":true,"temporary_cleanup":true},
        "performance_budget":null
    });
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(report_path)
        .unwrap();
    file.write_all(&serde_json::to_vec_pretty(&document).unwrap())
        .unwrap();
    file.sync_all().unwrap();
}
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
    ffmpeg_with_timeout(args, destination, Duration::from_secs(30));
}
fn ffmpeg_with_timeout(args: &[&str], destination: &Path, timeout: Duration) {
    let mut owned: Vec<OsString> = args.iter().map(OsString::from).collect();
    owned.push(destination.into());
    let borrowed: Vec<&OsStr> = owned.iter().map(OsString::as_os_str).collect();
    video_engine::run(
        &video_engine::tool(&engine(), "ffmpeg").unwrap(),
        &borrowed,
        timeout,
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
            "settb=1/1000000,setpts=PTS+3000*sin(N),select='not(mod(n,3))+not(mod(n,7))'",
            "-fps_mode",
            "vfr",
            "-enc_time_base:v",
            "1/1000000",
            "-movie_timescale",
            "1000000",
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
    use base64::Engine;
    let poster = crate::video_poster::poster(
        &engine(),
        fixture.0.to_str().unwrap(),
        source.to_str().unwrap(),
        64,
    )
    .unwrap();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(poster.strip_prefix("data:image/png;base64,").unwrap())
        .unwrap();
    let image = image::load_from_memory(&bytes).unwrap();
    assert!(image.height() > image.width());
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
    assert!(crate::video_poster::poster(
        &engine(),
        fixture.0.to_str().unwrap(),
        hdr.to_str().unwrap(),
        64
    )
    .unwrap_err()
    .contains("HDR"));
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
        .set_len(80 * 1024 * 1024)
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
    use base64::Engine;
    let before = crate::video_project::fingerprint(&source, || false).unwrap();
    let poster = crate::video_poster::poster(
        &engine(),
        fixture.0.to_str().unwrap(),
        source.to_str().unwrap(),
        160,
    )
    .unwrap();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(poster.strip_prefix("data:image/png;base64,").unwrap())
        .unwrap();
    let image = image::load_from_memory(&bytes).unwrap();
    assert!(image.width() <= 160 && image.height() <= 160);
    assert_eq!(
        crate::video_project::fingerprint(&source, || false).unwrap(),
        before
    );
}

#[test]
fn video_posters_cache_by_source_identity_and_reject_unavailable_media() {
    use base64::Engine;
    let fixture = Fixture::new();
    let source = fixture.0.join("poster source.mp4");
    ffmpeg(
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=red:size=96x64:rate=10:duration=1",
            "-c:v",
            "libopenh264",
        ],
        &source,
    );
    let render = || {
        crate::video_poster::poster(
            &engine(),
            fixture.0.to_str().unwrap(),
            source.to_str().unwrap(),
            64,
        )
        .unwrap()
    };
    let red = render();
    assert_eq!(render(), red);
    let image = |value: &str| {
        image::load_from_memory(
            &base64::engine::general_purpose::STANDARD
                .decode(value.strip_prefix("data:image/png;base64,").unwrap())
                .unwrap(),
        )
        .unwrap()
        .to_rgb8()
    };
    let red_image = image(&red);
    assert_eq!(red_image.dimensions(), (64, 42));
    assert!(red_image.get_pixel(32, 21)[0] > 200);
    ffmpeg(
        &[
            "-y",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:size=96x64:rate=10:duration=1",
            "-c:v",
            "libopenh264",
        ],
        &source,
    );
    let blue = render();
    assert_ne!(red, blue);
    assert!(image(&blue).get_pixel(32, 21)[2] > 200);
    let other = Fixture::new();
    assert!(crate::video_poster::poster(
        &engine(),
        other.0.to_str().unwrap(),
        source.to_str().unwrap(),
        64
    )
    .is_err());
    let broken = fixture.0.join("broken.mp4");
    fs::write(&broken, b"not video").unwrap();
    assert!(crate::video_poster::poster(
        &engine(),
        fixture.0.to_str().unwrap(),
        broken.to_str().unwrap(),
        64
    )
    .is_err());
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

fn pixels(path: &Path) -> Vec<u8> {
    video_engine::run(
        &video_engine::tool(&engine(), "ffmpeg").unwrap(),
        &[
            "-v".as_ref(),
            "error".as_ref(),
            "-i".as_ref(),
            path.as_os_str(),
            "-frames:v".as_ref(),
            "1".as_ref(),
            "-pix_fmt".as_ref(),
            "rgb24".as_ref(),
            "-f".as_ref(),
            "rawvideo".as_ref(),
            "pipe:1".as_ref(),
        ],
        Duration::from_secs(10),
    )
    .unwrap()
}
fn audio_rms(path: &Path) -> f64 {
    let data = video_engine::run(
        &video_engine::tool(&engine(), "ffmpeg").unwrap(),
        &[
            "-v".as_ref(),
            "error".as_ref(),
            "-i".as_ref(),
            path.as_os_str(),
            "-vn".as_ref(),
            "-ac".as_ref(),
            "1".as_ref(),
            "-f".as_ref(),
            "f32le".as_ref(),
            "pipe:1".as_ref(),
        ],
        Duration::from_secs(10),
    )
    .unwrap();
    let values: Vec<f64> = data
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f64::from(f32::from_le_bytes(*b)))
        .collect();
    let middle = &values[4800..values.len() - 4800];
    (middle.iter().map(|s| s * s).sum::<f64>() / middle.len() as f64).sqrt()
}
#[test]
fn saved_project_renders_trim_crop_rotation_flip_canvas_and_audio_without_touching_source() {
    use crate::video_project::{self, Crop, VideoTime};
    let fixture = Fixture::new();
    let source = fixture.0.join("project source.mp4");
    ffmpeg(&["-nostdin","-v","error","-f","lavfi","-i","color=c=red:size=96x64:rate=30:duration=2,drawbox=x=48:y=0:w=48:h=64:color=blue:t=fill","-f","lavfi","-i","sine=frequency=440:sample_rate=48000:duration=2","-c:v","libopenh264","-c:a","aac","-ac","2"],&source);
    let before = fingerprint(&source);
    let info = video_probe::probe(&engine(), &source, || false).unwrap();
    let mut project = video_project::new_project(
        fixture.0.to_str().unwrap(),
        &fs::canonicalize(&source).unwrap(),
        info,
    )
    .unwrap();
    let base = project.asset.info.time_base.clone().unwrap();
    project.clips[0].source_in = VideoTime::from_seconds(0.5, &base).unwrap();
    project.clips[0].source_out = VideoTime::from_seconds(1.5, &base).unwrap();
    project.output.width = 96;
    project.output.height = 64;
    project.clips[0].edits.flip_horizontal = true;
    project.clips[0].edits.gain_percent = 50;
    let directory = fixture.0.join("flip");
    fs::create_dir(&directory).unwrap();
    let result = crate::video_edit_preview::build(
        &engine(),
        fixture.0.to_str().unwrap(),
        &project,
        &directory,
        || false,
    )
    .unwrap();
    assert!((result.info.duration_seconds - 1.0).abs() < 0.05);
    let rgb = pixels(Path::new(&result.source));
    let left = (32 * 96 + 10) * 3;
    let right = (32 * 96 + 85) * 3;
    assert!(rgb[left + 2] > rgb[left] + 100);
    assert!(rgb[right] > rgb[right + 2] + 100);
    let gain = audio_rms(Path::new(&result.source)) / audio_rms(&source);
    assert!((gain - 0.5).abs() < 0.06, "gain={gain}");
    project.clips[0].edits.flip_horizontal = false;
    project.clips[0].edits.crop = Crop {
        x: 5000,
        y: 0,
        width: 5000,
        height: 10000,
    };
    project.clips[0].edits.quarter_turns = 1;
    project.clips[0].edits.muted = true;
    project.output.width = 64;
    project.output.height = 48;
    let directory = fixture.0.join("crop");
    fs::create_dir(&directory).unwrap();
    let result = crate::video_edit_preview::build(
        &engine(),
        fixture.0.to_str().unwrap(),
        &project,
        &directory,
        || false,
    )
    .unwrap();
    assert!(result.info.audio.is_empty());
    assert_eq!((result.info.width, result.info.height), (64, 48));
    let rgb = pixels(Path::new(&result.source));
    assert!(rgb[2] > rgb[0] + 100);
    project.clips[0].edits = video_project::Edits::default();
    project.output.width = 96;
    project.output.height = 96;
    let directory = fixture.0.join("fit");
    fs::create_dir(&directory).unwrap();
    let result = crate::video_edit_preview::build(
        &engine(),
        fixture.0.to_str().unwrap(),
        &project,
        &directory,
        || false,
    )
    .unwrap();
    let rgb = pixels(Path::new(&result.source));
    assert!(rgb[0] < 15 && rgb[1] < 15 && rgb[2] < 15);
    project.output.fit = "fill".into();
    let directory = fixture.0.join("fill");
    fs::create_dir(&directory).unwrap();
    let result = crate::video_edit_preview::build(
        &engine(),
        fixture.0.to_str().unwrap(),
        &project,
        &directory,
        || false,
    )
    .unwrap();
    let rgb = pixels(Path::new(&result.source));
    assert!(rgb[0] > rgb[2] + 100);
    assert_eq!(fingerprint(&source), before);
}

#[test]
fn edited_vfr_preview_keeps_selected_source_timestamps() {
    use crate::video_project::{self, VideoTime};
    let fixture = Fixture::new();
    let source = fixture.0.join("variable.mp4");
    ffmpeg(
        &[
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=96x64:rate=30:duration=2",
            "-vf",
            "select='not(mod(n,3))+not(mod(n,7))'",
            "-fps_mode",
            "vfr",
            "-c:v",
            "libopenh264",
        ],
        &source,
    );
    let info = video_probe::probe(&engine(), &source, || false).unwrap();
    let mut p = video_project::new_project(
        fixture.0.to_str().unwrap(),
        &fs::canonicalize(&source).unwrap(),
        info,
    )
    .unwrap();
    let base = p.asset.info.time_base.clone().unwrap();
    p.clips[0].source_in = VideoTime::from_seconds(0.4, &base).unwrap();
    p.clips[0].source_out = VideoTime::from_seconds(1.3, &base).unwrap();
    p.output.width = 96;
    p.output.height = 64;
    let directory = fixture.0.join("edit");
    fs::create_dir(&directory).unwrap();
    let result = crate::video_edit_preview::build(
        &engine(),
        fixture.0.to_str().unwrap(),
        &p,
        &directory,
        || false,
    )
    .unwrap();
    let expected: Vec<f64> = frame_times(&source)
        .into_iter()
        .filter(|t| *t >= 0.4 - 0.001 && *t < 1.3 - 0.001)
        .map(|t| t - 0.4)
        .collect();
    let actual = frame_times(Path::new(&result.source));
    assert_eq!(actual.len(), expected.len());
    for (a, b) in actual.iter().zip(expected) {
        assert!((a - b).abs() < 0.001, "{a} vs {b}");
    }
}

#[derive(Default)]
struct ExportControl {
    cancel: std::sync::atomic::AtomicBool,
    cancel_at: Option<crate::video_export::Stage>,
    no_space: bool,
    race_destination: Option<PathBuf>,
    corrupt_directory: Option<PathBuf>,
    mutate_source: Option<PathBuf>,
    stages: std::sync::Mutex<Vec<crate::video_export::Stage>>,
}
#[test]
fn brightness_contrast_preview_and_export_match_decoded_pixels_and_keep_black_borders() {
    use crate::{
        video_export::{self, Request},
        video_project,
        video_render::ExportSize,
    };
    let fixture = Fixture::new();
    let source = fixture.0.join("gray levels.mp4");
    ffmpeg(&["-nostdin","-v","error","-f","lavfi","-i","color=c=0x404040:size=96x64:rate=10:duration=0.5,drawbox=x=48:y=0:w=48:h=64:color=0xc0c0c0:t=fill","-c:v","libopenh264"], &source);
    let before = fingerprint(&source);
    let info = video_probe::probe(&engine(), &source, || false).unwrap();
    let mut p = video_project::new_project(fixture.0.to_str().unwrap(), &source, info).unwrap();
    p.output.width = 192;
    p.output.height = 192;
    let input = pixels(&source);
    for (index, (brightness, contrast)) in
        [(20, 0), (-20, 0), (0, 50), (20, -50), (100, 0), (0, -100)]
            .into_iter()
            .enumerate()
    {
        p.clips[0].edits.brightness = brightness;
        p.clips[0].edits.contrast = contrast;
        let output = fixture.0.join(format!("adjusted-{index}.mp4"));
        video_export::export_file(
            Request {
                engine: &engine(),
                root: fixture.0.to_str().unwrap(),
                project: &p,
                destination: &output,
                size: ExportSize::Project,
                acknowledge_first_audio: false,
            },
            std::sync::Arc::new(ExportControl::default()),
        )
        .unwrap();
        let directory = fixture.0.join(format!("preview-{index}"));
        fs::create_dir(&directory).unwrap();
        let preview = crate::video_edit_preview::build(
            &engine(),
            fixture.0.to_str().unwrap(),
            &p,
            &directory,
            || false,
        )
        .unwrap();
        let rendered = pixels(&output);
        let preview_pixels = pixels(Path::new(&preview.source));
        assert_eq!(rendered.len(), preview_pixels.len());
        let error = rendered
            .iter()
            .zip(&preview_pixels)
            .map(|(a, b)| (f64::from(*a) - f64::from(*b)).abs())
            .sum::<f64>()
            / rendered.len() as f64;
        assert!(error < 4.0, "preview/export difference {error}");
        for x in [12, 84] {
            for channel in 0..3 {
                let original = f64::from(input[(32 * 96 + x) * 3 + channel]);
                let expected = ((original - 127.5) * (1.0 + f64::from(contrast) / 100.0)
                    + 127.5
                    + 255.0 * f64::from(brightness) / 100.0)
                    .clamp(0.0, 255.0);
                let actual = f64::from(rendered[(96 * 192 + x * 2) * 3 + channel]);
                assert!(
                    (actual - expected).abs() < 7.0,
                    "brightness={brightness} contrast={contrast}: {actual} vs {expected}"
                );
            }
        }
        assert!(
            rendered[..192 * 16 * 3].iter().all(|v| *v < 8),
            "padding must remain black"
        );
        assert_eq!(
            frame_times(&output),
            frame_times(Path::new(&preview.source))
        );
    }
    assert_eq!(fingerprint(&source), before);
}
impl crate::video_export::Control for ExportControl {
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
    fn update(&self, stage: crate::video_export::Stage, fraction: Option<f64>, _: u64) {
        use crate::video_export::Stage;
        assert!(fraction.is_none_or(|f| f < 1.0));
        self.stages.lock().unwrap().push(stage);
        if self.cancel_at == Some(stage) {
            self.cancel.store(true, Ordering::SeqCst);
        }
        if stage == Stage::DecodingOutput {
            if let Some(root) = &self.corrupt_directory {
                for entry in fs::read_dir(root).unwrap().flatten() {
                    if entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".openphoto-")
                    {
                        fs::write(entry.path(), b"broken after header validation").unwrap();
                    }
                }
            }
            if let Some(source) = &self.mutate_source {
                fs::write(source, b"externally changed source").unwrap();
            }
        }
    }
    fn begin_publish(&self) -> Result<(), String> {
        if self.cancelled() {
            return Err("Export cancelled".into());
        }
        self.stages
            .lock()
            .unwrap()
            .push(crate::video_export::Stage::Publishing);
        if let Some(path) = &self.race_destination {
            fs::write(path, b"external completed file").unwrap();
        }
        Ok(())
    }
    fn available_space(&self, _: &Path) -> Result<Option<u64>, String> {
        Ok(if self.no_space { Some(0) } else { None })
    }
}
fn assert_no_export_temporaries(root: &Path) {
    assert!(!fs::read_dir(root).unwrap().flatten().any(|entry| entry
        .file_name()
        .to_string_lossy()
        .starts_with(".openphoto-")));
}
fn sample_pixels(path: &Path) -> Vec<u8> {
    video_engine::run(
        &video_engine::tool(&engine(), "ffmpeg").unwrap(),
        &[
            "-v".as_ref(),
            "error".as_ref(),
            "-i".as_ref(),
            path.as_os_str(),
            "-frames:v".as_ref(),
            "1".as_ref(),
            "-vf".as_ref(),
            "scale=96:54".as_ref(),
            "-pix_fmt".as_ref(),
            "rgb24".as_ref(),
            "-f".as_ref(),
            "rawvideo".as_ref(),
            "pipe:1".as_ref(),
        ],
        Duration::from_secs(10),
    )
    .unwrap()
}
#[test]
fn export_publishes_decoded_vfr_edits_and_matches_preview_geometry() {
    use crate::{
        video_export::{self, Request, Stage},
        video_project::{self, VideoTime},
        video_render::ExportSize,
    };
    let fixture = Fixture::new();
    let source = fixture.0.join("旅行 source.mp4");
    ffmpeg(
        &[
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=96x64:rate=30:duration=2",
            "-vf",
            "select='not(mod(n,3))+not(mod(n,7))'",
            "-fps_mode",
            "vfr",
            "-c:v",
            "libopenh264",
        ],
        &source,
    );
    let before = fingerprint(&source);
    let info = video_probe::probe(&engine(), &source, || false).unwrap();
    let mut p = video_project::new_project(fixture.0.to_str().unwrap(), &source, info).unwrap();
    let base = p.asset.info.time_base.clone().unwrap();
    p.clips[0].source_in = VideoTime::from_seconds(0.4, &base).unwrap();
    p.clips[0].source_out = VideoTime::from_seconds(1.3, &base).unwrap();
    p.clips[0].edits.quarter_turns = 1;
    p.clips[0].edits.flip_vertical = true;
    p.clips[0].edits.crop.width = 7500;
    p.output.width = 192;
    p.output.height = 108;
    let control = std::sync::Arc::new(ExportControl::default());
    let output = fixture.0.join("旅行 edited clip.mp4");
    let report = video_export::export_file(
        Request {
            engine: &engine(),
            root: fixture.0.to_str().unwrap(),
            project: &p,
            destination: &output,
            size: ExportSize::Project,
            acknowledge_first_audio: false,
        },
        control.clone(),
    )
    .unwrap();
    assert_eq!((report.width, report.height), (192, 108));
    assert!(output.exists());
    assert!((report.duration_seconds - 0.9).abs() < 0.1);
    let directory = fixture.0.join("preview");
    fs::create_dir(&directory).unwrap();
    let preview = crate::video_edit_preview::build(
        &engine(),
        fixture.0.to_str().unwrap(),
        &p,
        &directory,
        || false,
    )
    .unwrap();
    let a = sample_pixels(&output);
    let b = sample_pixels(Path::new(&preview.source));
    let error = a
        .iter()
        .zip(b)
        .map(|(a, b)| (f64::from(*a) - f64::from(b)).abs())
        .sum::<f64>()
        / a.len() as f64;
    assert!(error < 6.0, "pixel error={error}");
    let expected: Vec<f64> = frame_times(&source)
        .into_iter()
        .filter(|t| *t >= 0.4 - 0.001 && *t < 1.3 - 0.001)
        .map(|t| t - 0.4)
        .collect();
    let actual = frame_times(&output);
    assert_eq!(actual.len(), expected.len());
    for (a, b) in actual.iter().zip(expected) {
        assert!((a - b).abs() < 0.001);
    }
    let stages = control.stages.lock().unwrap();
    assert!(stages.contains(&Stage::DecodingOutput));
    assert_eq!(stages.last(), Some(&Stage::Publishing));
    assert_no_export_temporaries(&fixture.0);
    assert_eq!(fingerprint(&source), before);
}
#[test]
fn export_preserves_audio_offsets_gain_and_explicit_multiple_track_policy() {
    use crate::{
        video_export::{self, Request},
        video_project,
        video_render::ExportSize,
    };
    let fixture = Fixture::new();
    let source = fixture.0.join("audio.mp4");
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
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=880:sample_rate=48000:duration=2",
            "-map",
            "0:v",
            "-map",
            "1:a",
            "-map",
            "2:a",
            "-c:v",
            "libopenh264",
            "-c:a",
            "aac",
            "-ac",
            "2",
        ],
        &source,
    );
    let before = fingerprint(&source);
    let info = video_probe::probe(&engine(), &source, || false).unwrap();
    let mut p = video_project::new_project(fixture.0.to_str().unwrap(), &source, info).unwrap();
    p.output.width = 96;
    p.output.height = 64;
    p.clips[0].edits.gain_percent = 50;
    let output = fixture.0.join("audio-export.mp4");

    assert!(video_export::export_file(
        Request {
            engine: &engine(),
            root: fixture.0.to_str().unwrap(),
            project: &p,
            destination: &output,
            size: ExportSize::Project,
            acknowledge_first_audio: false
        },
        std::sync::Arc::new(ExportControl::default())
    )
    .unwrap_err()
    .contains("first audio"));
    assert!(!output.exists());
    let report = video_export::export_file(
        Request {
            engine: &engine(),
            root: fixture.0.to_str().unwrap(),
            project: &p,
            destination: &output,
            size: ExportSize::Project,
            acknowledge_first_audio: true,
        },
        std::sync::Arc::new(ExportControl::default()),
    )
    .unwrap();
    assert!(report
        .warnings
        .iter()
        .any(|w| w.contains("first source audio")));
    let info = video_probe::probe(&engine(), &output, || false).unwrap();
    assert_eq!(info.audio.len(), 1);
    let audio_start = info.audio[0]
        .start_time
        .as_ref()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    assert!((audio_start - 0.178).abs() < 0.05);
    let directory = fixture.0.join("preview");
    fs::create_dir(&directory).unwrap();
    let preview = crate::video_edit_preview::build(
        &engine(),
        fixture.0.to_str().unwrap(),
        &p,
        &directory,
        || false,
    )
    .unwrap();
    assert!((audio_rms(&output) / audio_rms(Path::new(&preview.source)) - 1.0).abs() < 0.05);
    p.clips[0].edits.muted = true;
    let silent = fixture.0.join("silent-export.mp4");
    video_export::export_file(
        Request {
            engine: &engine(),
            root: fixture.0.to_str().unwrap(),
            project: &p,
            destination: &silent,
            size: ExportSize::Project,
            acknowledge_first_audio: false,
        },
        std::sync::Arc::new(ExportControl::default()),
    )
    .unwrap();
    assert!(video_probe::probe(&engine(), &silent, || false)
        .unwrap()
        .audio
        .is_empty());
    assert_eq!(fingerprint(&source), before);
    assert_no_export_temporaries(&fixture.0);
}
#[test]
fn export_cancellation_failures_and_racing_destinations_never_publish_partial_files() {
    use crate::{
        video_export::{self, Request, Stage},
        video_project,
        video_render::ExportSize,
    };
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
            "testsrc2=size=96x64:rate=24:duration=1",
            "-c:v",
            "libopenh264",
        ],
        &source,
    );
    let before = fingerprint(&source);
    let info = video_probe::probe(&engine(), &source, || false).unwrap();
    let mut p = video_project::new_project(fixture.0.to_str().unwrap(), &source, info).unwrap();
    p.output.width = 96;
    p.output.height = 64;
    for (index, control) in [
        ExportControl {
            cancel_at: Some(Stage::Encoding),
            ..Default::default()
        },
        ExportControl {
            cancel_at: Some(Stage::DecodingOutput),
            ..Default::default()
        },
        ExportControl {
            no_space: true,
            ..Default::default()
        },
        ExportControl {
            corrupt_directory: Some(fixture.0.clone()),
            ..Default::default()
        },
    ]
    .into_iter()
    .enumerate()
    {
        let output = fixture.0.join(format!("failed-{index}.mp4"));
        assert!(video_export::export_file(
            Request {
                engine: &engine(),
                root: fixture.0.to_str().unwrap(),
                project: &p,
                destination: &output,
                size: ExportSize::Project,
                acknowledge_first_audio: false
            },
            std::sync::Arc::new(control)
        )
        .is_err());
        assert!(!output.exists());
        assert_no_export_temporaries(&fixture.0);
        assert_eq!(fingerprint(&source), before);
    }
    let output = fixture.0.join("race.mp4");
    let control = ExportControl {
        race_destination: Some(output.clone()),
        ..Default::default()
    };
    assert!(video_export::export_file(
        Request {
            engine: &engine(),
            root: fixture.0.to_str().unwrap(),
            project: &p,
            destination: &output,
            size: ExportSize::Project,
            acknowledge_first_audio: false
        },
        std::sync::Arc::new(control)
    )
    .is_err());
    assert_eq!(fs::read(output).unwrap(), b"external completed file");
    assert_eq!(fingerprint(&source), before);
    assert_no_export_temporaries(&fixture.0);
    let changed = fixture.0.join("changed.mp4");
    let control = ExportControl {
        mutate_source: Some(source.clone()),
        ..Default::default()
    };
    assert!(video_export::export_file(
        Request {
            engine: &engine(),
            root: fixture.0.to_str().unwrap(),
            project: &p,
            destination: &changed,
            size: ExportSize::Project,
            acknowledge_first_audio: false
        },
        std::sync::Arc::new(control)
    )
    .is_err());
    assert!(!changed.exists());
    assert_no_export_temporaries(&fixture.0);
}

#[test]
fn export_1080p_portrait_preset_is_validated_and_hdr_is_rejected() {
    use crate::{
        video_export::{self, Request},
        video_project,
        video_render::ExportSize,
    };
    assert_eq!(
        serde_json::from_str::<ExportSize>("\"hd1080\"").unwrap(),
        ExportSize::Hd1080
    );
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
            "testsrc2=size=64x48:rate=24:duration=0.5",
            "-c:v",
            "libopenh264",
        ],
        &source,
    );
    let before = fingerprint(&source);
    let info = video_probe::probe(&engine(), &source, || false).unwrap();
    let mut p = video_project::new_project(fixture.0.to_str().unwrap(), &source, info).unwrap();
    p.output.width = 2160;
    p.output.height = 3840;
    let output = fixture.0.join("portrait.mp4");
    let report = video_export::export_file(
        Request {
            engine: &engine(),
            root: fixture.0.to_str().unwrap(),
            project: &p,
            destination: &output,
            size: ExportSize::Hd1080,
            acknowledge_first_audio: false,
        },
        std::sync::Arc::new(ExportControl::default()),
    )
    .unwrap();
    assert_eq!((report.width, report.height), (1080, 1920));
    assert_eq!(fingerprint(&source), before);
    let hdr = fixture.0.join("hdr.mp4");
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
    let info = video_probe::probe(&engine(), &hdr, || false).unwrap();
    let p = video_project::new_project(fixture.0.to_str().unwrap(), &hdr, info).unwrap();
    let rejected = fixture.0.join("rejected.mp4");
    assert!(video_export::export_file(
        Request {
            engine: &engine(),
            root: fixture.0.to_str().unwrap(),
            project: &p,
            destination: &rejected,
            size: ExportSize::Project,
            acknowledge_first_audio: false
        },
        std::sync::Arc::new(ExportControl::default())
    )
    .unwrap_err()
    .contains("HDR"));
    assert!(!rejected.exists());
    assert_no_export_temporaries(&fixture.0);
}

#[test]
fn export_longer_vfr_audio_fixture_keeps_timestamps_and_audio_ends_aligned() {
    use crate::{
        video_export::{self, Request},
        video_project::{self, VideoTime},
        video_render::ExportSize,
    };
    let fixture = Fixture::new();
    let source = fixture.0.join("long.mp4");
    ffmpeg(
        &[
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=96x64:rate=30:duration=90",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=90",
            "-vf",
            "select='not(mod(n,2))+not(mod(n,7))'",
            "-fps_mode",
            "vfr",
            "-c:v",
            "libopenh264",
            "-c:a",
            "aac",
            "-ac",
            "2",
        ],
        &source,
    );
    let before = fingerprint(&source);
    let info = video_probe::probe(&engine(), &source, || false).unwrap();
    let mut p = video_project::new_project(fixture.0.to_str().unwrap(), &source, info).unwrap();
    let base = p.asset.info.time_base.clone().unwrap();
    p.clips[0].source_in = VideoTime::from_seconds(1.23, &base).unwrap();
    p.clips[0].source_out = VideoTime::from_seconds(88.67, &base).unwrap();
    p.output.width = 96;
    p.output.height = 64;
    let output = fixture.0.join("long-export.mp4");
    video_export::export_file(
        Request {
            engine: &engine(),
            root: fixture.0.to_str().unwrap(),
            project: &p,
            destination: &output,
            size: ExportSize::Project,
            acknowledge_first_audio: false,
        },
        std::sync::Arc::new(ExportControl::default()),
    )
    .unwrap();
    let start = p.clips[0].source_in.seconds().unwrap();
    let end = p.clips[0].source_out.seconds().unwrap();
    let expected: Vec<f64> = frame_times(&source)
        .into_iter()
        .filter(|t| *t >= start - 0.00001 && *t < end - 0.00001)
        .map(|t| t - start)
        .collect();
    let actual = frame_times(&output);
    assert_eq!(actual.len(), expected.len());
    for (a, b) in actual.iter().zip(expected) {
        assert!((a - b).abs() < 0.001, "{a} vs {b}");
    }
    let info = video_probe::probe(&engine(), &output, || false).unwrap();
    let audio = &info.audio[0];
    let audio_end = audio.start_time.as_ref().unwrap().parse::<f64>().unwrap()
        + audio.duration_seconds.unwrap();
    assert!((audio_end - (end - start)).abs() < 0.05);
    assert_eq!(fingerprint(&source), before);
    assert_no_export_temporaries(&fixture.0);
}

#[test]
fn independent_stream_validation_rejects_short_video_hidden_by_long_audio() {
    use crate::{
        video_project,
        video_render::{self, ExportSize, RenderOptions},
    };
    let fixture = Fixture::new();
    let source = fixture.0.join("source.mp4");
    let bad = fixture.0.join("short-video.mp4");
    for (path, length) in [(&source, "2"), (&bad, "0.5")] {
        ffmpeg(
            &[
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                &format!("testsrc2=size=64x48:rate=30:duration={length}"),
                "-f",
                "lavfi",
                "-i",
                "sine=sample_rate=48000:duration=2",
                "-c:v",
                "libopenh264",
                "-c:a",
                "aac",
                "-ac",
                "2",
            ],
            path,
        );
    }
    let info = video_probe::probe(&engine(), &source, || false).unwrap();
    let mut p = video_project::new_project(fixture.0.to_str().unwrap(), &source, info).unwrap();
    p.output.width = 64;
    p.output.height = 48;
    let plan = video_render::plan(
        &engine(),
        fixture.0.to_str().unwrap(),
        &p,
        &fixture.0.join("output.mp4"),
        RenderOptions {
            preview: false,
            size: ExportSize::Project,
            acknowledge_first_audio: false,
        },
        || false,
    )
    .unwrap();
    assert!(
        video_render::validate_output(&engine(), &p, &plan, &bad, || false)
            .unwrap_err()
            .contains("independent timing")
    );
    // Unequal tracks are valid when they match the verified source.
    let info = video_probe::probe(&engine(), &bad, || false).unwrap();
    let mut p = video_project::new_project(fixture.0.to_str().unwrap(), &bad, info).unwrap();
    p.output.width = 64;
    p.output.height = 48;
    let plan = video_render::plan(
        &engine(),
        fixture.0.to_str().unwrap(),
        &p,
        &fixture.0.join("output.mp4"),
        RenderOptions {
            preview: false,
            size: ExportSize::Project,
            acknowledge_first_audio: false,
        },
        || false,
    )
    .unwrap();
    video_render::validate_output(&engine(), &p, &plan, &bad, || false).unwrap();
    video_render::encode(&engine(), &plan, || false, |_| {}).unwrap();
    video_render::validate_output(&engine(), &p, &plan, &fixture.0.join("output.mp4"), || {
        false
    })
    .unwrap();
    let mut old = serde_json::to_value(&p.asset.info).unwrap();
    old.as_object_mut()
        .unwrap()
        .remove("video_duration_seconds");
    let restored: crate::video_probe::VideoInfo = serde_json::from_value(old).unwrap();
    assert!(restored.video_duration_seconds.is_none());
}
#[test]
fn packet_timing_fallback_handles_matroska_without_stream_duration() {
    let fixture = Fixture::new();
    let source = fixture.0.join("timing.mkv");
    ffmpeg(
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=64x48:rate=30:duration=2",
            "-f",
            "lavfi",
            "-i",
            "sine=sample_rate=48000:duration=2",
            "-c:v",
            "libopenh264",
            "-c:a",
            "aac",
        ],
        &source,
    );
    let info = video_probe::probe(&engine(), &source, || false).unwrap();
    assert!(info.video_duration_seconds.is_none());
    let (video, audio) =
        crate::video_timing::source_spans(&engine(), &source, &info, 0.4, 1.0, || false).unwrap();
    assert!((video.end - 1.0).abs() < 0.05);
    assert!((audio.unwrap().end - 1.0).abs() < 0.05);
}

#[test]
fn uhd_exports_preserve_landscape_portrait_and_1080p_limit() {
    use crate::{
        video_export::{self, Request},
        video_project,
        video_render::{target_bitrate, ExportSize},
    };
    let fixture = Fixture::new();
    let source = fixture.0.join("uhd.mp4");
    ffmpeg(
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=3840x2160:rate=15:duration=1",
            "-c:v",
            "libopenh264",
        ],
        &source,
    );
    let info = video_probe::probe(&engine(), &source, || false).unwrap();
    let mut p = video_project::new_project(fixture.0.to_str().unwrap(), &source, info).unwrap();
    for (name, width, height, turn, size, expected, bitrate) in [
        (
            "landscape",
            3840,
            2160,
            0,
            ExportSize::Project,
            (3840, 2160),
            48_000_000,
        ),
        (
            "limited",
            3840,
            2160,
            0,
            ExportSize::Hd1080,
            (1920, 1080),
            12_000_000,
        ),
        (
            "portrait",
            2160,
            3840,
            1,
            ExportSize::Project,
            (2160, 3840),
            48_000_000,
        ),
    ] {
        p.output.width = width;
        p.output.height = height;
        p.clips[0].edits.quarter_turns = turn;
        assert_eq!(target_bitrate(&p, size), bitrate);
        let report = video_export::export_file(
            Request {
                engine: &engine(),
                root: fixture.0.to_str().unwrap(),
                project: &p,
                destination: &fixture.0.join(format!("{name}.mp4")),
                size,
                acknowledge_first_audio: false,
            },
            std::sync::Arc::new(ExportControl::default()),
        )
        .unwrap();
        assert_eq!((report.width, report.height), expected);
    }
}

#[cfg(windows)]
#[test]
#[ignore = "real media acceptance: use scripts/accept-video.ps1 -Source"]
fn video_real_media_acceptance() {
    use crate::{
        video_export::{self, Control, Request, Stage},
        video_project::{self, VideoTime},
        video_render::ExportSize,
    };
    use std::{io::Write, sync::Arc, time::Instant};
    if cfg!(debug_assertions) {
        panic!("Real-media acceptance requires an optimized release build");
    }
    struct RealControl;
    impl Control for RealControl {
        fn cancelled(&self) -> bool {
            false
        }
        fn update(&self, _: Stage, _: Option<f64>, _: u64) {}
        fn begin_publish(&self) -> Result<(), String> {
            Ok(())
        }
    }
    let source =
        fs::canonicalize(std::env::var_os("OPENPHOTO_REAL_SOURCE").expect("Choose a real source"))
            .unwrap();
    let report_path = PathBuf::from(
        std::env::var_os("OPENPHOTO_ACCEPTANCE_OUTPUT").expect("Choose a report path"),
    );
    let output_root = report_path.parent().unwrap();
    let root = source.parent().unwrap().to_str().unwrap();
    let before = video_project::fingerprint(&source, || false).unwrap();
    let info = video_probe::probe(&engine(), &source, || false).unwrap();
    assert!(!info.hdr, "This acceptance preset requires SDR footage");
    assert!(
        info.duration_seconds >= 4.61,
        "Choose a clip at least 4.61 seconds long"
    );
    use base64::Engine;
    let poster =
        crate::video_poster::poster(&engine(), root, source.to_str().unwrap(), 640).unwrap();
    let png = base64::engine::general_purpose::STANDARD
        .decode(poster.strip_prefix("data:image/png;base64,").unwrap())
        .unwrap();
    let image = image::load_from_memory(&png).unwrap();
    assert!(image.width() <= 640 && image.height() <= 640);
    fs::write(output_root.join("poster.png"), png).unwrap();
    let mut p = video_project::new_project(root, &source, info.clone()).unwrap();
    let base = info.time_base.as_deref().unwrap();
    p.clips[0].source_in = VideoTime::from_seconds(0.37, base).unwrap();
    p.clips[0].source_out = VideoTime::from_seconds(4.61, base).unwrap();
    p.clips[0].edits.gain_percent = 75;
    let (a, b) = info
        .sample_aspect_ratio
        .as_deref()
        .unwrap_or("1:1")
        .split_once(':')
        .unwrap();
    let sar = a.parse::<f64>().unwrap() / b.parse::<f64>().unwrap();
    let mut native = (
        (f64::from(info.width) * sar / 2.0).floor() as u32 * 2,
        info.height / 2 * 2,
    );
    if (info.rotation_degrees / 90.0).round() as i64 % 2 != 0 {
        native = (native.1, native.0);
    }
    let mut outputs = vec![];
    for (name, width, height) in [
        ("source-resolution", native.0, native.1),
        ("uhd-4k-canvas", 3840, 2160),
    ] {
        p.output.width = width;
        p.output.height = height;
        let output = output_root.join(format!("{name}.mp4"));
        video_engine::reset_native_peak();
        let started = Instant::now();
        let result = video_export::export_file(
            Request {
                engine: &engine(),
                root,
                project: &p,
                destination: &output,
                size: ExportSize::Project,
                acknowledge_first_audio: info.audio.len() > 1,
            },
            Arc::new(RealControl),
        )
        .unwrap();
        let elapsed = started.elapsed().as_secs_f64();
        assert_eq!((result.width, result.height), (width, height));
        assert!((result.duration_seconds - 4.24).abs() < 0.1);
        assert_eq!(
            video_project::fingerprint(&source, || false).unwrap(),
            before
        );
        video_project::write_copy(
            &p,
            &output_root.join(format!("{name}.openphoto-video.json")),
        )
        .unwrap();
        outputs.push(serde_json::json!({"name":name,"width":width,"height":height,"output_seconds":result.duration_seconds,"bytes":result.bytes,"export_wall_seconds":elapsed,"maximum_individual_native_child_peak_working_set_bytes":video_engine::native_peak(),"source_unchanged":true,"stream_validation":true,"full_decode":true,"publication":true}));
    }
    assert_no_export_temporaries(output_root);
    let document = serde_json::json!({"schema_version":1,"app_version":env!("CARGO_PKG_VERSION"),"engine_version":video_engine::VERSION,"scope":"Real-footage native exports; excludes GUI and whole-app memory. UHD canvas may letterbox or upscale the source; it does not add captured detail.","source_filename":source.file_name().unwrap().to_string_lossy(),"source_info":info,"trim_seconds":[0.37,4.61],"gain_percent":75,"outputs":outputs,"source_sha256":before.1,"source_bytes":before.0});
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(report_path)
        .unwrap();
    file.write_all(&serde_json::to_vec_pretty(&document).unwrap())
        .unwrap();
    file.sync_all().unwrap();
}

// Inspect random-access packets independently of the render graph, retaining only
// their timestamps. Flat scenes must still get periodic decoder entry points.
fn random_access_gap(path: &Path) -> f64 {
    use std::sync::{Arc, Mutex};
    let keys = Arc::new(Mutex::new(Vec::<f64>::new()));
    let copy = keys.clone();
    video_engine::run_lines(
        &video_engine::tool(&engine(), "ffprobe").unwrap(),
        &[
            "-v".as_ref(),
            "error".as_ref(),
            "-select_streams".as_ref(),
            "v:0".as_ref(),
            "-show_packets".as_ref(),
            "-show_entries".as_ref(),
            "packet=pts_time,flags".as_ref(),
            "-of".as_ref(),
            "compact=p=0:nk=0".as_ref(),
            path.as_os_str(),
        ],
        Duration::from_secs(180),
        || false,
        move |line| {
            let text = String::from_utf8_lossy(line);
            let mut pts = None;
            let mut key = false;
            for field in text.trim().split('|') {
                if let Some((name, value)) = field.split_once('=') {
                    match name {
                        "pts_time" => pts = crate::video_timing::number(Some(value)),
                        "flags" => key = value.contains('K'),
                        _ => {}
                    }
                }
            }
            if key {
                let mut keys = copy.lock().map_err(|e| e.to_string())?;
                if keys.len() >= 100_000 {
                    return Err("Too many random-access points in acceptance media".into());
                }
                keys.push(pts.ok_or("Random-access packet has no timestamp")?);
            }
            Ok(())
        },
    )
    .unwrap();
    let keys = Arc::try_unwrap(keys).unwrap().into_inner().unwrap();
    assert!(!keys.is_empty(), "Output has no random-access packets");
    let info = video_probe::probe(&engine(), path, || false).unwrap();
    let start = crate::video_timing::number(info.start_time.as_deref()).unwrap_or(0.0);
    let end = start + info.video_duration_seconds.unwrap_or(info.duration_seconds);
    let gap = keys
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .chain([keys[0] - start, end - keys[keys.len() - 1]])
        .fold(0.0, f64::max);
    assert!(
        gap <= 2.25,
        "Output requires too much decoding before a seek: {gap:.3}s gap between access points"
    );
    gap
}
#[test]
fn previews_and_exports_keep_regular_access_points_even_without_scene_changes() {
    let fixture = Fixture::new();
    let source = fixture.0.join("flat.mp4");
    ffmpeg(
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=navy:size=96x64:rate=24:duration=14",
            "-c:v",
            "libopenh264",
        ],
        &source,
    );
    let identity = fingerprint(&source);
    let info = video_probe::probe(&engine(), &source, || false).unwrap();
    let mut project =
        crate::video_project::new_project(fixture.0.to_str().unwrap(), &source, info).unwrap();
    project.output.width = 96;
    project.output.height = 64;
    let output = fixture.0.join("flat-export.mp4");
    crate::video_export::export_file(
        crate::video_export::Request {
            engine: &engine(),
            root: fixture.0.to_str().unwrap(),
            project: &project,
            destination: &output,
            size: crate::video_render::ExportSize::Project,
            acknowledge_first_audio: false,
        },
        std::sync::Arc::new(ExportControl::default()),
    )
    .unwrap();
    random_access_gap(&output);
    let previews = fixture.0.join("proxy");
    fs::create_dir(&previews).unwrap();
    let proxy =
        crate::video_preview::build_assets(&engine(), &source, &previews, true, || false).unwrap();
    random_access_gap(Path::new(&proxy.source));
    assert_eq!(fingerprint(&source), identity);
}

// Seek between two earlier random-access points so either neighboring keyframe
// selected by the demuxer precedes the range. Exact-boundary seeks can jump forward.
fn preroll_seek(path: &Path, stream: usize, start: f64) -> Option<f64> {
    use std::sync::{atomic::AtomicBool, Arc, Mutex};
    let key = Arc::new(Mutex::new(Vec::<f64>::with_capacity(2)));
    let copy = key.clone();
    let done = Arc::new(AtomicBool::new(false));
    let complete = done.clone();
    let stream = stream.to_string();
    let result = video_engine::run_lines(
        &video_engine::tool(&engine(), "ffprobe").unwrap(),
        &[
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
            "packet=pts_time,dts_time,flags".as_ref(),
            "-of".as_ref(),
            "compact=p=0:nk=0".as_ref(),
            path.as_os_str(),
        ],
        Duration::from_secs(180),
        || done.load(Ordering::Acquire),
        move |line| {
            if complete.load(Ordering::Acquire) {
                return Ok(());
            }
            let text = String::from_utf8_lossy(line);
            let mut pts = None;
            let mut dts = None;
            let mut is_key = false;
            for field in text.trim().split('|') {
                if let Some((name, value)) = field.split_once('=') {
                    match name {
                        "pts_time" => pts = crate::video_timing::number(Some(value)),
                        "dts_time" => dts = crate::video_timing::number(Some(value)),
                        "flags" => is_key = value.contains('K'),
                        _ => {}
                    }
                }
            }
            if let Some(pts) = pts {
                if is_key && pts <= start {
                    let time = dts.unwrap_or(pts);
                    let mut keys = copy.lock().map_err(|e| e.to_string())?;
                    if keys.last().is_none_or(|last| time > *last) {
                        if keys.len() == 2 {
                            keys.remove(0);
                        }
                        keys.push(time);
                    }
                }
                if pts >= start {
                    complete.store(true, Ordering::Release);
                }
            }
            Ok(())
        },
    );
    if let Err(error) = result {
        assert!(
            done.load(Ordering::Acquire) && error == "Video processing cancelled or superseded",
            "Source keyframe inspection failed: {error}"
        );
    }
    let keys = Arc::try_unwrap(key).unwrap().into_inner().unwrap();
    match keys.as_slice() {
        [earlier, later] => Some(earlier + (later - earlier) / 2.0).filter(|t| t.is_finite()),
        _ => None, // No initial seek is safer when only the first access point is known.
    }
}
// Inspect decoded presentation order until a frame beyond the selected range is observed.
// Stopping at a packet timestamp can discard a still-buffered B-frame inside the range.
#[derive(Debug)]
struct FrameWindow {
    start: f64,
    end: f64,
    frames: Vec<f64>,
    complete: bool,
    first_seen: Option<f64>,
    time_base: Option<f64>,
}
impl FrameWindow {
    fn new(start: f64, end: f64) -> Self {
        Self {
            start,
            end,
            frames: vec![],
            complete: false,
            first_seen: None,
            time_base: None,
        }
    }
    fn consume(&mut self, line: &[u8]) -> Result<bool, String> {
        // A terminated decoder can leave a partial final record.
        // Once the presentation range is complete, subsequent output cannot change it.
        if self.complete {
            return Ok(true);
        }
        if !line.ends_with(b"\n") {
            return Err("Incomplete decoded-frame record".into());
        }
        let text = std::str::from_utf8(line).map_err(|e| e.to_string())?.trim();
        if let Some(base) = text.strip_prefix("#tb 0:") {
            let (a, b) = base
                .trim()
                .split_once('/')
                .ok_or("Missing decoded-frame time base")?;
            let a = a.parse::<u32>().map_err(|e| e.to_string())?;
            let b = b.parse::<u32>().map_err(|e| e.to_string())?;
            if a == 0 || b == 0 {
                return Err("Invalid decoded-frame time base".into());
            }
            self.time_base = Some(f64::from(a) / f64::from(b));
            return Ok(false);
        }
        if text.is_empty() || text.starts_with('#') {
            return Ok(false);
        }
        let fields: Vec<_> = text.split(',').map(str::trim).collect();
        if fields.len() != 6 || fields[0] != "0" || !fields[5].starts_with("0x") {
            return Err("Invalid decoded-frame record".into());
        }
        let ticks = fields[2].parse::<i64>().map_err(|e| e.to_string())?;
        let time = ticks as f64
            * self
                .time_base
                .ok_or("Decoded-frame record before time base")?;
        self.first_seen.get_or_insert(time);
        if time >= self.end {
            self.complete = true;
            return Ok(true);
        }
        if time >= self.start {
            if self.frames.len() >= 50_000 {
                return Err("Source window exceeds the frame inspection limit".into());
            }
            self.frames.push(time);
        }
        Ok(false)
    }
}
fn range_frame_times(path: &Path, stream: usize, start: f64, end: f64) -> Vec<f64> {
    use std::sync::{atomic::AtomicBool, Arc, Mutex};
    let seek = preroll_seek(path, stream, start);
    let stream = format!("0:{stream}");
    let frames = Arc::new(Mutex::new(FrameWindow::new(start, end)));
    let copy = frames.clone();
    let done = Arc::new(AtomicBool::new(false));
    let complete = done.clone();
    // Decode source frames directly. FFprobe interval seeking can exit successfully
    // without emitting any frames. Framecrc emits a record only after a full decode,
    // while copyts/demux time base retain original VFR timestamps independently of
    // the render graph. Disable accurate-seek discarding to retain decoder preroll.
    let mut args: Vec<OsString> = [
        "-nostdin",
        "-v",
        "error",
        "-xerror",
        "-threads",
        "2",
        "-copyts",
        "-max_alloc",
        "268435456",
        "-protocol_whitelist",
        "file,pipe",
        "-format_whitelist",
        "mov,matroska,webm,avi",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    if let Some(time) = seek {
        args.extend([
            "-noaccurate_seek".into(),
            "-ss".into(),
            format!("{time:.9}").into(),
        ]);
    }
    args.extend(["-i".into(), path.into(), "-map".into(), stream.into()]);
    args.extend(
        [
            "-an",
            "-sn",
            "-dn",
            "-c:v",
            "rawvideo",
            "-threads:v",
            "1",
            "-enc_time_base:v",
            "demux",
            "-fps_mode",
            "passthrough",
            "-f",
            "framecrc",
            "pipe:1",
        ]
        .into_iter()
        .map(Into::into),
    );
    let borrowed: Vec<_> = args.iter().map(OsString::as_os_str).collect();
    let result = video_engine::run_lines(
        &video_engine::tool(&engine(), "ffmpeg").unwrap(),
        &borrowed,
        Duration::from_secs(180),
        || done.load(Ordering::Acquire),
        move |line| {
            if copy.lock().map_err(|e| e.to_string())?.consume(line)? {
                complete.store(true, Ordering::Release);
            }
            Ok(())
        },
    );
    if let Err(error) = result {
        assert!(
            done.load(Ordering::Acquire) && error == "Video processing cancelled or superseded",
            "Source frame inspection failed: {error}"
        );
    }
    let window = Arc::try_unwrap(frames).unwrap().into_inner().unwrap();
    assert!(!window.frames.is_empty(),"Source inspection yielded no frames for {start:.9}..{end:.9}; seek target {seek:?}; first decoded timestamp {:?}",window.first_seen);
    window.frames
}
#[test]
fn completed_frame_windows_ignore_buffered_and_partial_probe_output() {
    let mut window = FrameWindow::new(0.37, 1.61);
    assert!(!window.consume(b"#tb 0: 1/1000000\n").unwrap());
    assert!(!window
        .consume(b"0, 383333, 383333, 16667, 4608, 0x12345678\n")
        .unwrap());
    assert!(!window
        .consume(b"0, 1600000, 1600000, 16667, 4608, 0x12345678\n")
        .unwrap());
    assert!(window
        .consume(b"0, 1616667, 1616667, 16667, 4608, 0x12345678\n")
        .unwrap());
    assert!(window
        .consume(b"0, 10000000, 10000000, 16667, 4608, 0x12345678\n")
        .unwrap());
    assert!(window.consume(b"0, 1").unwrap());
    assert_eq!(window.frames.len(), 2);
    assert!((window.frames[0] - 0.383333).abs() < 0.000001);
    assert!((window.frames[1] - 1.6).abs() < 0.000001);
}
#[test]
fn decoded_frame_records_require_valid_time_bases_and_complete_records() {
    for base in [
        b"#tb 0: 0/1\n".as_slice(),
        b"#tb 0: 1/0\n",
        b"#tb 0: invalid\n",
    ] {
        assert!(FrameWindow::new(0.0, 1.0).consume(base).is_err());
    }
    let mut window = FrameWindow::new(0.0, 1.0);
    assert!(window.consume(b"0, 0, 0, 512, 4608, 0x12345678\n").is_err());
    window.consume(b"#tb 0: 1/15360\n").unwrap();
    assert!(window.consume(b"0, 512, 512").is_err());
    assert!(window
        .consume(b"0, x, invalid, 512, 4608, 0x12345678\n")
        .is_err());
    assert!(!window
        .consume(b"0, -256, -256, 256, 4608, 0x12345678\n")
        .unwrap());
    assert!(!window
        .consume(b"0, 512, 512, 256, 4608, 0x12345678\n")
        .unwrap());
    assert_eq!(window.frames, vec![1.0 / 30.0]);
}
#[test]
fn range_inspection_drains_reordered_frames_before_stopping() {
    let fixture = Fixture::new();
    let source = fixture.0.join("reordered.mp4");
    ffmpeg(
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=64x48:rate=60:duration=2",
            "-c:v",
            "mpeg4",
            "-bf",
            "2",
        ],
        &source,
    );
    let expected: Vec<f64> = frame_times(&source)
        .into_iter()
        .filter(|t| *t >= 0.37 && *t < 1.61)
        .collect();
    assert!(!expected.is_empty());
    let actual = range_frame_times(&source, 0, 0.37, 1.61);
    assert_eq!(actual.len(), expected.len());
    assert!(actual
        .iter()
        .zip(expected)
        .all(|(a, b)| (a - b).abs() < 0.000001));
}
#[test]
fn frame_inspection_keeps_ranges_adjacent_to_random_access_boundaries() {
    let fixture = Fixture::new();
    let source = fixture.0.join("key-boundaries.mp4");
    ffmpeg(
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=64x48:rate=10:duration=60",
            "-c:v",
            "libopenh264",
            "-g",
            "100",
        ],
        &source,
    );
    let all = frame_times(&source);
    for start in [0.37, 10.000001, 20.000001, 49.999999, 50.000001] {
        let end = start + 4.24;
        let expected: Vec<f64> = all
            .iter()
            .copied()
            .filter(|t| *t >= start && *t < end)
            .collect();
        assert!(!expected.is_empty());
        let actual = range_frame_times(&source, 0, start, end);
        assert_eq!(actual.len(), expected.len(), "Source range {start}..{end}");
        assert!(
            actual
                .iter()
                .zip(expected)
                .all(|(a, b)| (a - b).abs() < 0.000001),
            "Source range {start}..{end}"
        );
    }
}
fn verify_long_windows(source: &Path, output_root: &Path) -> serde_json::Value {
    use crate::{
        video_export::{self, Control, Request, Stage},
        video_project::{self, VideoTime},
        video_render::ExportSize,
    };
    use std::{sync::Arc, time::Instant};
    struct LongControl;
    impl Control for LongControl {
        fn cancelled(&self) -> bool {
            false
        }
        fn update(&self, _: Stage, _: Option<f64>, _: u64) {}
        fn begin_publish(&self) -> Result<(), String> {
            Ok(())
        }
    }
    let info = video_probe::probe(&engine(), source, || false).unwrap();
    assert!(
        info.duration_seconds >= 3600.0,
        "Choose a recording at least one hour long"
    );
    assert!(
        !info.hdr,
        "Long-recording acceptance currently requires SDR footage"
    );
    let root = source.parent().unwrap().to_str().unwrap();
    let mut project = video_project::new_project(root, source, info.clone()).unwrap();
    let identity = (project.asset.bytes, project.asset.sha256.clone());
    // Preserve a supported square-pixel source canvas; this gate is about seeking/timing.
    assert_eq!(info.sample_aspect_ratio.as_deref(), Some("1:1"));
    project.output.width = info.width / 2 * 2;
    project.output.height = info.height / 2 * 2;
    if (info.rotation_degrees / 90.0).round() as i64 % 2 != 0 {
        std::mem::swap(&mut project.output.width, &mut project.output.height);
    }
    let base = info.time_base.as_deref().unwrap();
    let origin = crate::video_timing::number(info.format_start_time.as_deref()).unwrap_or(0.0);
    let mut results = vec![];
    for (name, start) in [
        ("beginning", 0.37),
        ("middle", info.duration_seconds / 2.0 + 0.37),
        ("end", info.duration_seconds - 4.61),
    ] {
        project.clips[0].source_in = VideoTime::from_seconds(start, base).unwrap();
        project.clips[0].source_out = VideoTime::from_seconds(start + 4.24, base).unwrap();
        let actual_start = project.clips[0].source_in.seconds().unwrap();
        let actual_end = project.clips[0].source_out.seconds().unwrap();
        eprintln!("Continuous export: verifying {name} decoded window");
        let expected = range_frame_times(
            source,
            info.video_stream,
            origin + actual_start,
            origin + actual_end,
        );
        assert!(!expected.is_empty());
        let output = output_root.join(format!("{name}.mp4"));
        let started = Instant::now();
        let report = video_export::export_file(
            Request {
                engine: &engine(),
                root,
                project: &project,
                destination: &output,
                size: ExportSize::Project,
                acknowledge_first_audio: info.audio.len() > 1,
            },
            Arc::new(LongControl),
        )
        .unwrap();
        let elapsed = started.elapsed().as_secs_f64();
        let actual = frame_times(&output);
        assert_eq!(expected.len(), actual.len(), "Frame loss in {name} window");
        let max_error = expected
            .iter()
            .zip(&actual)
            .map(|(a, b)| (a - origin - actual_start - b).abs())
            .fold(0.0, f64::max);
        assert!(max_error < 0.001, "Timestamp error in {name}: {max_error}");
        let output_info = video_probe::probe(&engine(), &output, || false).unwrap();
        // The production validator checks audio against its actual intersection with this window.
        video_project::write_copy(
            &project,
            &output_root.join(format!("{name}.openphoto-video.json")),
        )
        .unwrap();
        results.push(serde_json::json!({"window":name,"source_in_seconds":actual_start,"source_out_seconds":actual_end,"decoded_frames":actual.len(),"max_frame_timestamp_error_seconds":max_error,"export_wall_seconds":elapsed,"output_seconds":report.duration_seconds,"width":report.width,"height":report.height,"bytes":report.bytes,"audio":output_info.audio,"full_decode":true,"independent_stream_validation":true}));
    }
    assert_eq!(
        video_project::fingerprint(source, || false).unwrap(),
        identity
    );
    assert_no_export_temporaries(output_root);
    serde_json::json!({"app_version":env!("CARGO_PKG_VERSION"),"engine_version":video_engine::VERSION,"scope":"Beginning/middle/end short-window native exports from an hour-scale recording; does not establish complete hour-long export or perceptual A/V synchronization or GUI playback","source_filename":source.file_name().unwrap().to_string_lossy(),"source_duration_seconds":info.duration_seconds,"source_bytes":identity.0,"source_sha256":identity.1,"source_unchanged":true,"windows":results})
}
#[test]
fn hour_scale_source_seeks_keep_decoded_timestamps_at_three_positions() {
    let fixture = Fixture::new();
    let source = fixture.0.join("hour.mp4");
    ffmpeg_with_timeout(
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=64x48:rate=10:duration=3605",
            "-f",
            "lavfi",
            "-i",
            "sine=sample_rate=8000:duration=3605",
            "-c:v",
            "libopenh264",
            "-c:a",
            "aac",
        ],
        &source,
        Duration::from_secs(120),
    );
    verify_long_windows(&source, &fixture.0);
}
#[test]
#[ignore = "hour-scale real media acceptance: scripts/accept-video.ps1 -Source -LongRecording"]
fn video_long_recording_acceptance() {
    use std::io::Write;
    if cfg!(debug_assertions) {
        panic!("Long-recording acceptance requires an optimized release build");
    }
    let source =
        fs::canonicalize(std::env::var_os("OPENPHOTO_REAL_SOURCE").expect("Choose a real source"))
            .unwrap();
    let report_path = PathBuf::from(
        std::env::var_os("OPENPHOTO_ACCEPTANCE_OUTPUT").expect("Choose a report path"),
    );
    let report = verify_long_windows(&source, report_path.parent().unwrap());
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(report_path)
        .unwrap();
    file.write_all(&serde_json::to_vec_pretty(&report).unwrap())
        .unwrap();
    file.sync_all().unwrap();
}

/// Run explicitly in release mode, on user-selected footage at least one hour long.
#[cfg(windows)]
#[test]
#[ignore = "full-recording real media acceptance: scripts/accept-video.ps1 -Source -FullRecording"]
fn video_full_recording_acceptance() {
    if cfg!(debug_assertions) {
        panic!("Use an optimized release build");
    }
    let source = fs::canonicalize(
        std::env::var_os("OPENPHOTO_REAL_SOURCE").expect("Choose a real recording"),
    )
    .unwrap();
    let report_path = PathBuf::from(
        std::env::var_os("OPENPHOTO_ACCEPTANCE_OUTPUT").expect("Choose a report path"),
    );
    verify_full_recording(&source, &report_path, 3600.0);
}
#[cfg(windows)]
#[test]
fn continuous_export_keeps_decoded_timestamps_across_full_sequence() {
    let fixture = Fixture::new();
    let source = fixture.0.join("continuous-vfr.mp4");
    ffmpeg(
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=96x64:rate=30:duration=12",
            "-f",
            "lavfi",
            "-i",
            "sine=sample_rate=48000:duration=12",
            "-vf",
            "settb=1/1000000,setpts=PTS+3000*sin(N),select='not(mod(n,3))+not(mod(n,7))'",
            "-fps_mode",
            "vfr",
            "-enc_time_base:v",
            "1/1000000",
            "-movie_timescale",
            "1000000",
            "-c:v",
            "libopenh264",
            "-c:a",
            "aac",
        ],
        &source,
    );
    verify_full_recording(&source, &fixture.0.join("acceptance.json"), 8.0);
}
#[cfg(windows)]
fn verify_full_recording(source: &Path, report_path: &Path, minimum_duration: f64) {
    use crate::{
        video_export::{self, Control, Request, Stage},
        video_project::{self, VideoTime},
        video_render::ExportSize,
    };
    use std::{
        io::Write,
        sync::{Arc, Mutex},
        time::Instant,
    };
    struct RecordingControl {
        started: Instant,
        stages: Mutex<Vec<(Stage, f64)>>,
    }
    impl Control for RecordingControl {
        fn cancelled(&self) -> bool {
            false
        }
        fn update(&self, stage: Stage, _: Option<f64>, _: u64) {
            let mut stages = self.stages.lock().unwrap();
            if stages.last().is_none_or(|(previous, _)| *previous != stage) {
                eprintln!(
                    "Full recording: {stage:?} ({:.1}s)",
                    self.started.elapsed().as_secs_f64()
                );
                stages.push((stage, self.started.elapsed().as_secs_f64()));
            }
        }
        fn begin_publish(&self) -> Result<(), String> {
            self.update(Stage::Publishing, None, 0);
            Ok(())
        }
    }
    let output_root = report_path.parent().unwrap();
    assert!(!report_path.exists(), "Do not overwrite an earlier report");
    let info = video_probe::probe(&engine(), source, || false).unwrap();
    assert!(
        info.duration_seconds >= minimum_duration,
        "The recording is shorter than the acceptance minimum"
    );
    assert!(!info.hdr, "Full-recording acceptance requires SDR footage");
    assert_eq!(info.sample_aspect_ratio.as_deref(), Some("1:1"));
    let root = source.parent().unwrap().to_str().unwrap();
    let mut project = video_project::new_project(root, source, info.clone()).unwrap();
    let identity = (project.asset.bytes, project.asset.sha256.clone());
    project.output.width = info.width / 2 * 2;
    project.output.height = info.height / 2 * 2;
    if (info.rotation_degrees / 90.0).round() as i64 % 2 != 0 {
        std::mem::swap(&mut project.output.width, &mut project.output.height);
    }
    let base = info.time_base.as_deref().unwrap();
    project.clips[0].source_in = VideoTime::from_seconds(0.37, base).unwrap();
    project.clips[0].source_out =
        VideoTime::from_seconds(info.duration_seconds - 0.37, base).unwrap();
    let start = project.clips[0].source_in.seconds().unwrap();
    let duration = project.clips[0].source_out.seconds().unwrap() - start;
    let destination = output_root.join("full-recording.mp4");
    video_engine::reset_native_peak();
    let control = Arc::new(RecordingControl {
        started: Instant::now(),
        stages: Mutex::new(vec![]),
    });
    let report = video_export::export_file(
        Request {
            engine: &engine(),
            root,
            project: &project,
            destination: &destination,
            size: ExportSize::Hd1080,
            acknowledge_first_audio: info.audio.len() > 1,
        },
        control.clone(),
    )
    .unwrap();
    let export_seconds = control.started.elapsed().as_secs_f64();
    let maximum_random_access_gap_seconds = random_access_gap(&destination);
    let origin = crate::video_timing::number(info.format_start_time.as_deref()).unwrap_or(0.0);
    let mut windows = vec![];
    for (name, at) in [
        ("beginning", 0.37),
        ("middle", duration / 2.0),
        ("end", duration - 4.61),
    ] {
        let expected = range_frame_times(
            source,
            info.video_stream,
            origin + start + at,
            origin + start + at + 4.24,
        );
        let actual = range_frame_times(&destination, 0, at, at + 4.24);
        assert!(!expected.is_empty(), "Empty reference window: {name}");
        assert_eq!(
            expected.len(),
            actual.len(),
            "Frame loss in continuous export at {name}"
        );
        let max_error = expected
            .iter()
            .zip(&actual)
            .map(|(a, b)| (a - origin - start - b).abs())
            .fold(0.0, f64::max);
        assert!(
            max_error < 0.001,
            "Continuous-export timestamp drift at {name}: {max_error}"
        );
        windows.push(serde_json::json!({"position":name,"output_start_seconds":at,"decoded_frames":actual.len(),"max_frame_timestamp_error_seconds":max_error}));
    }
    assert_eq!(
        video_project::fingerprint(source, || false).unwrap(),
        identity
    );
    assert_no_export_temporaries(output_root);
    project.output.width = report.width;
    project.output.height = report.height;
    video_project::write_copy(
        &project,
        &output_root.join("full-recording.openphoto-video.json"),
    )
    .unwrap();
    let stages = control.stages.lock().unwrap();
    for required in [
        Stage::VerifyingSource,
        Stage::Encoding,
        Stage::CheckingOutput,
        Stage::DecodingOutput,
        Stage::Publishing,
    ] {
        assert!(stages.iter().any(|(stage, _)| *stage == required));
    }
    let document = serde_json::json!({"schema_version":1,"app_version":env!("CARGO_PKG_VERSION"),"engine_version":video_engine::VERSION,"scope":"One continuous hour-scale native export with full output decode and beginning/middle/end decoded frame timing; excludes GUI, perceptual A/V assessment, and whole-app memory","source_filename":source.file_name().unwrap().to_string_lossy(),"source_bytes":identity.0,"source_sha256":identity.1,"source_unchanged":true,"source_duration_seconds":info.duration_seconds,"trim_seconds":[start,start+duration],"output_seconds":report.duration_seconds,"width":report.width,"height":report.height,"bytes":report.bytes,"export_wall_seconds":export_seconds,"media_seconds_per_wall_second":duration/export_seconds,"maximum_individual_native_child_peak_working_set_bytes":video_engine::native_peak(),"stages":*stages,"windows":windows,"full_output_decoded":true,"maximum_random_access_gap_seconds":maximum_random_access_gap_seconds,"independent_audio_video_stream_validation":true,"output_published":true});
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(report_path)
        .unwrap();
    file.write_all(&serde_json::to_vec_pretty(&document).unwrap())
        .unwrap();
    file.sync_all().unwrap();
}
