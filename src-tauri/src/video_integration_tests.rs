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
