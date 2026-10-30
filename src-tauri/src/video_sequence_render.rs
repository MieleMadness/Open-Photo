//! Bounded hard-cut compiler: one source decoder at a time, one video encode per clip.
use crate::{
    video_engine, video_probe, video_project,
    video_render::{self, ExportSize, Plan, RenderOptions},
    video_sequence::Sequence,
    video_timing::Span,
};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

/// Local, verified source roots are separate from the portable edit document.
#[derive(Clone)]
pub struct Snapshot {
    pub project: Sequence,
    pub roots: BTreeMap<String, String>,
}
impl Snapshot {
    pub fn validate(&self) -> Result<(), String> {
        crate::video_sequence::validate(&self.project)?;
        if self.project.clips.is_empty() {
            return Err("Append a clip before rendering this project".into());
        }
        for clip in &self.project.clips {
            if !self.roots.contains_key(&clip.asset_id) {
                return Err("Relink every timeline source before rendering".into());
            }
        }
        Ok(())
    }
    pub fn single(&self, index: usize) -> Result<(String, video_project::Project), String> {
        let clip = self
            .project
            .clips
            .get(index)
            .ok_or("Timeline clip no longer available")?;
        let asset = self
            .project
            .assets
            .iter()
            .find(|a| a.id == clip.asset_id)
            .ok_or("Timeline source no longer available")?;
        let root = self
            .roots
            .get(&asset.id)
            .ok_or("Relink this timeline source")?
            .clone();
        Ok((
            root,
            crate::video_sequence::single(&self.project, asset, clip.clone()),
        ))
    }
    pub fn revalidate(&self, cancel: impl Fn() -> bool) -> Result<(), String> {
        for asset in &self.project.assets {
            if !self.project.clips.iter().any(|c| c.asset_id == asset.id) {
                continue;
            }
            let root = self
                .roots
                .get(&asset.id)
                .ok_or("Relink this timeline source")?;
            let p = crate::video_sequence::single(
                &self.project,
                asset,
                crate::video_sequence::default_clip(asset)?,
            );
            video_project::source(root, &p, &cancel)?;
        }
        Ok(())
    }
    pub fn duration(&self) -> Result<f64, String> {
        self.project
            .clips
            .iter()
            .map(|c| Ok(c.source_out.seconds()? - c.source_in.seconds()?))
            .sum()
    }
    pub fn bitrate(&self, size: ExportSize) -> Result<u64, String> {
        let (_, p) = self.single(0)?;
        Ok(video_render::target_bitrate(&p, size))
    }
}

struct Scratch {
    path: Option<PathBuf>,
    parent: PathBuf,
}
impl Scratch {
    fn new(parent: &Path) -> Result<Self, String> {
        let parent = fs::canonicalize(parent).map_err(|e| e.to_string())?;
        let path = parent.join(format!(".openphoto-parts-{}", video_project::id()));
        fs::create_dir(&path).map_err(|e| e.to_string())?;
        Ok(Self {
            path: Some(path),
            parent,
        })
    }
    fn path(&self) -> &Path {
        self.path.as_deref().expect("live scratch directory")
    }
    fn close(&mut self) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if !path.exists() {
            self.path = None;
            return Ok(());
        }
        let resolved =
            fs::canonicalize(path).map_err(|e| format!("Temporary parts cleanup failed: {e}"))?;
        if resolved.parent() != Some(self.parent.as_path())
            || fs::symlink_metadata(path)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
        {
            return Err("Temporary parts cleanup failed: directory ownership changed".into());
        }
        fs::remove_dir_all(path)
            .map_err(|e| format!("Temporary parts cleanup failed: {}: {e}", path.display()))?;
        self.path = None;
        Ok(())
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
pub struct Compiled {
    pub plan: Plan,
    pub encoded_fraction: f64,
    scratch: Option<Scratch>,
}
impl Compiled {
    pub fn single(plan: Plan) -> Self {
        Self {
            plan,
            encoded_fraction: 0.0,
            scratch: None,
        }
    }
    pub fn close(&mut self) -> Result<(), String> {
        if let Some(scratch) = &mut self.scratch {
            scratch.close()?;
        }
        Ok(())
    }
}

/// Rounding cumulative boundaries bounds total rounding error to half an output frame.
fn frame_allocation(project: &Sequence) -> Result<Vec<u64>, String> {
    let mut seconds = 0.0;
    let mut previous = 0u64;
    let mut result = vec![];
    for clip in &project.clips {
        seconds += clip.source_out.seconds()? - clip.source_in.seconds()?;
        let end = (seconds * f64::from(project.frame_rate)).round() as u64;
        if end <= previous {
            return Err("A timeline clip is too short for the project frame rate".into());
        }
        result.push(end - previous);
        previous = end;
    }
    Ok(result)
}
fn progress_args(args: &mut Vec<OsString>, output: &Path, format: &str, max_bytes: u64) {
    args.extend([
        "-map_metadata".into(),
        "-1".into(),
        "-map_chapters".into(),
        "-1".into(),
        "-fs".into(),
        max_bytes.to_string().into(),
        "-progress".into(),
        "pipe:1".into(),
        "-nostats".into(),
        "-f".into(),
        format.into(),
        output.into(),
    ]);
}
struct PartOptions<'a> {
    frames: u64,
    fps: u32,
    audio: bool,
    bitrate: u64,
    output: &'a Path,
    max_bytes: u64,
}
fn part_args(
    prepared: &video_render::Prepared,
    edits: &video_project::Edits,
    options: PartOptions<'_>,
) -> Vec<OsString> {
    let PartOptions {
        frames,
        fps,
        audio,
        bitrate,
        output,
        max_bytes,
    } = options;
    let duration = frames as f64 / f64::from(fps);
    let samples = frames * 48000 / u64::from(fps);
    let mut args: Vec<OsString> = [
        "-nostdin",
        "-v",
        "error",
        "-xerror",
        "-y",
        "-max_alloc",
        "268435456",
        "-threads",
        "2",
        "-filter_threads",
        "1",
        "-protocol_whitelist",
        "file,pipe",
        "-format_whitelist",
        "mov,matroska,webm,avi",
        "-ss",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    args.extend([
        format!("{:.9}", prepared.start).into(),
        "-i".into(),
        prepared.source.clone().into(),
    ]);
    let real_audio = audio && !edits.muted && prepared.audio_span.is_some();
    if audio && !real_audio {
        args.extend([
            "-f".into(),
            "lavfi".into(),
            "-i".into(),
            "anullsrc=r=48000:cl=stereo".into(),
        ]);
    }
    let filter=format!("trim=start=0:end={:.9},setpts=PTS-STARTPTS,fps=fps={fps}:start_time=0,tpad=stop_mode=clone:stop_duration={:.9},trim=end_frame={frames},setpts=N/({fps}*TB),{},scale=out_color_matrix=bt709:out_range=tv,format=yuv420p,setsar=1",prepared.duration,duration+1.0/f64::from(fps),prepared.filter);
    args.extend([
        "-map".into(),
        format!("0:{}", prepared.info.video_stream).into(),
        "-vf".into(),
        filter.into(),
    ]);
    if audio {
        let (map, filter) = if real_audio {
            // Preserve decoded audio timestamps, including encoder delay and a late first sample.
            (format!("0:{}",prepared.info.audio[0].stream),format!("atrim=start=0:end={:.9},aformat=sample_fmts=flt:channel_layouts=stereo,aresample=48000:async=1:first_pts=0,volume={:.4},apad=whole_len={samples},atrim=end_sample={samples},asetpts=N/SR/TB",prepared.duration,f64::from(edits.gain_percent)/100.0))
        } else {
            ("1:a:0".into(),format!("aformat=sample_fmts=flt:channel_layouts=stereo,atrim=end_sample={samples},asetpts=N/SR/TB"))
        };
        args.extend([
            "-map".into(),
            map.into(),
            "-af".into(),
            filter.into(),
            "-c:a".into(),
            "pcm_f32le".into(),
            "-ar".into(),
            "48000".into(),
            "-ac".into(),
            "2".into(),
        ]);
    } else {
        args.push("-an".into());
    }
    args.extend([
        "-t".into(),
        format!("{duration:.9}").into(),
        "-fps_mode".into(),
        "cfr".into(),
        "-c:v".into(),
        "libopenh264".into(),
        "-bf".into(),
        "0".into(),
        "-b:v".into(),
        bitrate.to_string().into(),
        "-force_key_frames:v".into(),
        video_engine::H264_KEYFRAME_EXPRESSION.into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-colorspace".into(),
        "bt709".into(),
        "-color_primaries".into(),
        "bt709".into(),
        "-color_trc".into(),
        "bt709".into(),
        "-color_range".into(),
        "tv".into(),
    ]);
    progress_args(&mut args, output, "matroska", max_bytes);
    args
}

pub(crate) fn build_preview(
    engine: &Path,
    snapshot: &Snapshot,
    directory: &Path,
    cancel: impl Fn() -> bool,
) -> Result<crate::video_preview::PreviewAssets, String> {
    let output = directory.join("sequence-preview.mp4");
    let mut compiled = compile(
        engine,
        snapshot,
        &output,
        RenderOptions {
            preview: true,
            size: ExportSize::Project,
            acknowledge_first_audio: true,
        },
        &cancel,
        |_| {},
    )?;
    video_render::encode(engine, &compiled.plan, &cancel, |_| {})?;
    let info = video_render::validate_output(engine, &compiled.plan, &output, &cancel)?;
    video_render::decode_check(engine, &output, &cancel, |_| {})?;
    snapshot.revalidate(&cancel)?;
    compiled.close()?;
    Ok(crate::video_preview::PreviewAssets {
        source: output.to_string_lossy().into_owned(),
        proxy: true,
        info,
        filmstrip: vec![],
        warnings: compiled.plan.warnings,
    })
}
pub fn compile(
    engine: &Path,
    snapshot: &Snapshot,
    output: &Path,
    options: RenderOptions,
    cancel: impl Fn() -> bool,
    progress: impl Fn(f64) + Send + Sync + 'static,
) -> Result<Compiled, String> {
    snapshot.validate()?;
    if snapshot.project.clips.len() == 1 {
        let (root, p) = snapshot.single(0)?;
        return Ok(Compiled {
            plan: video_render::plan(engine, &root, &p, output, options, cancel)?,
            encoded_fraction: 0.0,
            scratch: None,
        });
    }
    let scratch = Scratch::new(output.parent().ok_or("Output folder unavailable")?)?;
    let directory = scratch.path().to_owned();
    let result = (|| {
        let frames = frame_allocation(&snapshot.project)?;
        let total_frames: u64 = frames.iter().sum();
        let fps = snapshot.project.frame_rate;
        let duration = total_frames as f64 / f64::from(fps);
        let max_bytes = if options.preview {
            512 * 1024 * 1024
        } else {
            video_render::MAX_EXPORT_BYTES
        };
        let bitrate = if options.preview {
            2_000_000
        } else {
            snapshot.bitrate(options.size)?
        };
        let mut manifest = String::from("ffconcat version 1.0\n");
        let mut width = 0;
        let mut height = 0;
        let mut info = None;
        let mut completed = 0u64;
        let mut retained_bytes = 0u64;
        let progress = std::sync::Arc::new(progress);
        let mut contexts = Vec::with_capacity(frames.len());
        for index in 0..frames.len() {
            if cancel() {
                return Err("Video render cancelled".into());
            }
            let (root, p) = snapshot.single(index)?;
            let prepared = video_render::prepare(engine, &root, &p, options, &cancel)?;
            if prepared
                .info
                .color_primaries
                .as_deref()
                .is_some_and(|p| p != "bt709")
                || prepared
                    .info
                    .color_transfer
                    .as_deref()
                    .is_some_and(|p| p != "bt709")
            {
                return Err("Multi-clip output currently requires SDR Rec.709 sources; wide-gamut conversion needs separate validation".into());
            }
            contexts.push(prepared);
        }
        let audio = contexts
            .iter()
            .enumerate()
            .any(|(i, p)| !snapshot.project.clips[i].edits.muted && p.audio_span.is_some());
        if options.preview {
            let required = (duration
                * (2.0 * bitrate as f64 / 8.0 * 1.25 + if audio { 384000.0 } else { 0.0 }))
            .ceil() as u64
                + 64 * 1024 * 1024;
            if crate::video_export::available_space(&directory)?.is_some_and(|free| free < required)
            {
                return Err(format!(
                    "Not enough temporary space for the sequence preview (allow {} MB)",
                    required.div_ceil(1024 * 1024)
                ));
            }
        }
        for (index, (prepared, count)) in contexts.into_iter().zip(frames.iter()).enumerate() {
            if cancel() {
                return Err("Video render cancelled".into());
            }
            let part = directory.join(format!("clip-{index:04}.mkv"));
            let args = part_args(
                &prepared,
                &snapshot.project.clips[index].edits,
                PartOptions {
                    frames: *count,
                    fps,
                    audio,
                    bitrate,
                    output: &part,
                    max_bytes,
                },
            );
            let length = *count as f64 / f64::from(fps);
            let before = completed as f64 / f64::from(fps);
            let notify = progress.clone();
            video_engine::run_progress(
                &video_engine::tool(engine, "ffmpeg")?,
                &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                Duration::from_secs(21600),
                &cancel,
                move |p| {
                    notify(
                        ((before + p.seconds.clamp(0.0, length)) / duration).clamp(0.0, 1.0) * 0.75,
                    )
                },
            )?;
            let result = video_probe::probe(engine, &part, &cancel)?;
            if result.width != prepared.width
                || result.height != prepared.height
                || result.codec != "h264"
                || result.audio.len() != usize::from(audio)
                || result.audio.first().is_some_and(|a| {
                    a.codec != "pcm_f32le"
                        || a.channels != Some(2)
                        || a.sample_rate.as_deref() != Some("48000")
                })
            {
                return Err("A normalized clip failed stream or canvas validation".into());
            }
            retained_bytes += fs::metadata(&part).map_err(|e| e.to_string())?.len();
            if retained_bytes >= max_bytes + ((duration * 384000.0) as u64) {
                return Err("Normalized project parts exceed the export size limit".into());
            }
            width = prepared.width;
            height = prepared.height;
            info = Some(prepared.info);
            completed += count;
            manifest.push_str(&format!("file clip-{index:04}.mkv\nduration {length:.9}\n"));
        }
        let list = directory.join("sequence.ffconcat");
        fs::File::create(&list)
            .and_then(|mut f| f.write_all(manifest.as_bytes()))
            .map_err(|e| e.to_string())?;
        let mut args: Vec<OsString> = [
            "-nostdin",
            "-v",
            "error",
            "-xerror",
            "-y",
            "-max_alloc",
            "268435456",
            "-threads",
            "2",
            "-protocol_whitelist",
            "file,pipe",
            "-format_whitelist",
            "concat,matroska",
            "-safe",
            "1",
            "-f",
            "concat",
            "-i",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        // FFmpeg URL joining misreads Windows verbatim paths. Keep this list local to the job.
        args.push("sequence.ffconcat".into());
        args.extend([
            "-map".into(),
            "0:v:0".into(),
            "-c:v".into(),
            "copy".into(),
            "-bsf:v".into(),
            format!(
                "setts=pts=N*{}:dts=N*{}:duration={}:time_base=1/90000:prescale=1",
                90000 / fps,
                90000 / fps,
                90000 / fps
            )
            .into(),
        ]);
        if audio {
            args.extend([
                "-map".into(),
                "0:a:0".into(),
                "-af".into(),
                format!(
                    "atrim=end_sample={},asetpts=N/SR/TB",
                    total_frames * 48000 / u64::from(fps)
                )
                .into(),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "192k".into(),
                "-ar".into(),
                "48000".into(),
                "-ac".into(),
                "2".into(),
            ]);
        } else {
            args.push("-an".into());
        }
        args.extend([
            "-t".into(),
            format!("{duration:.9}").into(),
            "-movflags".into(),
            "+faststart".into(),
            "-video_track_timescale".into(),
            "90000".into(),
            "-movie_timescale".into(),
            "90000".into(),
        ]);
        progress_args(&mut args, output, "mp4", max_bytes);
        let mut info = info.ok_or("No timeline video")?;
        info.frame_rate = Some(format!("{fps}/1"));
        let warnings=vec![format!("Hard cuts use a shared {fps} fps SDR Rec.709 canvas. Source trims remain exact in the project; output cut positions use its frame clock."),"Video is encoded once per clip; continuous audio is encoded once as AAC stereo. Silent or muted intervals are filled with silence when the project has audio. Source metadata, subtitles and chapters are excluded.".into()];
        Ok(Compiled {
            plan: Plan {
                info,
                width,
                height,
                video_span: Span {
                    start: 0.0,
                    end: duration,
                },
                audio_span: audio.then_some(Span {
                    start: 0.0,
                    end: duration,
                }),
                expect_audio: audio,
                expected_frames: Some(total_frames),
                working_directory: Some(directory.clone()),
                duration,
                max_bytes,
                timeout: Duration::from_secs(21600),
                args,
                warnings,
            },
            encoded_fraction: 0.75,
            scratch: Some(scratch),
        })
    })();
    if result.is_err() && directory.exists() {
        return Err(format!(
            "{}; Temporary parts cleanup failed: {}",
            result.as_ref().err().unwrap(),
            directory.display()
        ));
    }
    result
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::video_project::VideoTime;
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn engine() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/video-engine")
    }
    fn fixture() -> (Fixture, Snapshot) {
        let dir =
            std::env::temp_dir().join(format!("sequence-render-短片-{}", video_project::id()));
        fs::create_dir(&dir).unwrap();
        let mut snapshot = Snapshot {
            project: Sequence {
                schema_version: 2,
                id: video_project::id(),
                title: "Short cuts".into(),
                assets: vec![],
                clips: vec![],
                output: video_project::Output {
                    width: 96,
                    height: 64,
                    fit: "fit".into(),
                },
                frame_rate: 30,
            },
            roots: BTreeMap::new(),
        };
        for (index, color, size, rate, audio) in [
            (0, "red", "96x64", 24, true),
            (1, "blue", "64x96", 60, false),
        ] {
            let file = dir.join(format!("source-{index} 世界.mp4"));
            let mut args: Vec<OsString> = [
                "-nostdin",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                &if index==0{format!("color=red:size={size}:rate={rate}:duration=0.2[r];color=green:size={size}:rate={rate}:duration=0.2[g];[r][g]concat=n=2:v=1:a=0")}else{format!("color={color}:size={size}:rate={rate}:duration=0.4")},
            ]
            .into_iter()
            .map(Into::into)
            .collect();
            if audio {
                args.extend([
                    "-f".into(),
                    "lavfi".into(),
                    "-i".into(),
                    "sine=frequency=880:sample_rate=48000:duration=0.4".into(),
                    "-c:a".into(),
                    "aac".into(),
                    "-ac".into(),
                    "2".into(),
                ]);
            } else {
                args.push("-an".into());
            }
            args.extend([
                "-t".into(),
                "0.4".into(),
                "-c:v".into(),
                "libopenh264".into(),
                "-b:v".into(),
                "100000".into(),
                "-pix_fmt".into(),
                "yuv420p".into(),
                file.clone().into(),
            ]);
            video_engine::run(
                &video_engine::tool(&engine(), "ffmpeg").unwrap(),
                &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                Duration::from_secs(30),
            )
            .unwrap();
            let info = video_probe::probe(&engine(), &file, || false).unwrap();
            let mut p = video_project::new_project(dir.to_str().unwrap(), &file, info).unwrap();
            let base = p.asset.info.time_base.as_deref().unwrap();
            p.clips[0].source_in =
                VideoTime::from_seconds(if index == 0 { 0.05 } else { 0.0 }, base).unwrap();
            p.clips[0].source_out =
                VideoTime::from_seconds(if index == 0 { 0.25 } else { 0.3 }, base).unwrap();
            snapshot
                .roots
                .insert(p.asset.id.clone(), dir.to_string_lossy().into_owned());
            snapshot.project.assets.push(p.asset);
            snapshot.project.clips.extend(p.clips);
        }
        (Fixture(dir), snapshot)
    }
    fn pixel(file: &Path, time: &str) -> Vec<u8> {
        video_engine::run(
            &video_engine::tool(&engine(), "ffmpeg").unwrap(),
            &[
                "-v".as_ref(),
                "error".as_ref(),
                "-ss".as_ref(),
                time.as_ref(),
                "-i".as_ref(),
                file.as_os_str(),
                "-vf".as_ref(),
                "scale=1:1".as_ref(),
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
    fn rms(file: &Path, time: &str) -> f64 {
        let data = video_engine::run(
            &video_engine::tool(&engine(), "ffmpeg").unwrap(),
            &[
                "-v".as_ref(),
                "error".as_ref(),
                "-ss".as_ref(),
                time.as_ref(),
                "-i".as_ref(),
                file.as_os_str(),
                "-t".as_ref(),
                "0.1".as_ref(),
                "-vn".as_ref(),
                "-ac".as_ref(),
                "1".as_ref(),
                "-ar".as_ref(),
                "48000".as_ref(),
                "-f".as_ref(),
                "f32le".as_ref(),
                "pipe:1".as_ref(),
            ],
            Duration::from_secs(10),
        )
        .unwrap();
        let values = data.as_chunks::<4>().0;
        assert!(!values.is_empty());
        (values
            .iter()
            .map(|v| f64::from(f32::from_le_bytes(*v)).powi(2))
            .sum::<f64>()
            / values.len() as f64)
            .sqrt()
    }
    #[test]
    fn short_cuts_preserve_order_trims_silent_intervals_and_originals_with_one_frame_clock() {
        let (fixture, snapshot) = fixture();
        let output = fixture.0.join("assembled.mp4");
        let mut compiled = compile(
            &engine(),
            &snapshot,
            &output,
            RenderOptions {
                preview: false,
                size: ExportSize::Project,
                acknowledge_first_audio: false,
            },
            || false,
            |_| {},
        )
        .unwrap();
        video_render::encode(&engine(), &compiled.plan, || false, |_| {}).unwrap();
        let info =
            video_render::validate_output(&engine(), &compiled.plan, &output, || false).unwrap();
        assert_eq!(info.frame_count, Some(15));
        assert_eq!(info.audio.len(), 1);
        video_render::decode_check(&engine(), &output, || false, |_| {}).unwrap();
        let green = pixel(&output, "0.15");
        assert!(green[1] > green[0] + 30, "{green:?}");
        let red = pixel(&output, "0.1");
        let blue = pixel(&output, "0.35");
        assert!(red[0] > red[2] + 50, "{red:?}");
        assert!(blue[2] > blue[0] + 50, "{blue:?}");
        assert!(rms(&output, "0.04") > 0.02);
        assert!(rms(&output, "0.30") < 0.002);
        snapshot.revalidate(|| false).unwrap();
        compiled.close().unwrap();
        assert!(!fs::read_dir(&fixture.0).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".openphoto-parts-")));
    }
    fn finish(snapshot: &Snapshot, output: &Path, preview: bool) -> video_probe::VideoInfo {
        let mut compiled = compile(
            &engine(),
            snapshot,
            output,
            RenderOptions {
                preview,
                size: ExportSize::Project,
                acknowledge_first_audio: false,
            },
            || false,
            |_| {},
        )
        .unwrap();
        video_render::encode(&engine(), &compiled.plan, || false, |_| {}).unwrap();
        let info =
            video_render::validate_output(&engine(), &compiled.plan, output, || false).unwrap();
        video_render::decode_check(&engine(), output, || false, |_| {}).unwrap();
        compiled.close().unwrap();
        info
    }
    #[test]
    fn mute_gain_repeated_sources_and_sixty_fps_keep_continuous_audio_policy() {
        let (fixture, mut snapshot) = fixture();
        let full = fixture.0.join("full.mp4");
        finish(&snapshot, &full, false);
        let reference = rms(&full, "0.04");
        snapshot.project.clips[0].edits.gain_percent = 50;
        let quiet = fixture.0.join("quiet.mp4");
        finish(&snapshot, &quiet, false);
        let ratio = rms(&quiet, "0.04") / reference;
        assert!((0.45..0.55).contains(&ratio), "{ratio}");
        snapshot.project.clips[0].edits.muted = true;
        let silent = fixture.0.join("silent.mp4");
        assert!(finish(&snapshot, &silent, false).audio.is_empty());
        let mut repeat = snapshot.project.clips[0].clone();
        repeat.id = video_project::id();
        repeat.edits.muted = false;
        repeat.edits.gain_percent = 25;
        snapshot.project.clips.push(repeat);
        snapshot.project.frame_rate = 60;
        let repeated = fixture.0.join("repeated.mp4");
        let info = finish(&snapshot, &repeated, false);
        assert_eq!(info.frame_count, Some(42));
        assert!(rms(&repeated, "0.04") < 0.002);
        assert!(rms(&repeated, "0.30") < 0.002);
        assert!(rms(&repeated, "0.55") > 0.005);
        snapshot.revalidate(|| false).unwrap();
    }
    #[test]
    fn preview_and_export_share_geometry_edits_and_frame_clock() {
        let (fixture, mut snapshot) = fixture();
        snapshot.project.output.width = 1920;
        snapshot.project.output.height = 1080;
        snapshot.project.clips[0].edits.quarter_turns = 1;
        snapshot.project.clips[0].edits.brightness = 15;
        let preview = build_preview(&engine(), &snapshot, &fixture.0, || false).unwrap();
        let output = fixture.0.join("exported.mp4");
        let info = finish(&snapshot, &output, false);
        assert_eq!((preview.info.width, preview.info.height), (960, 540));
        assert_eq!((info.width, info.height), (1920, 1080));
        assert_eq!(preview.info.frame_count, info.frame_count);
        if let Some(folder) = std::env::var_os("OPENPHOTO_SEQUENCE_ARTIFACT_DIR") {
            let folder = PathBuf::from(folder);
            fs::create_dir_all(&folder).unwrap();
            let mut document =
                crate::safe_output::StagedOutput::new(&folder.join("project.json")).unwrap();
            document
                .file()
                .write_all(&serde_json::to_vec_pretty(&snapshot.project).unwrap())
                .unwrap();
            document.commit().unwrap();
            for asset in &snapshot.project.assets {
                let source =
                    Path::new(snapshot.roots.get(&asset.id).unwrap()).join(&asset.relative_path);
                let mut copy =
                    crate::safe_output::StagedOutput::new(&folder.join(&asset.relative_path))
                        .unwrap();
                copy.file().write_all(&fs::read(source).unwrap()).unwrap();
                copy.commit().unwrap();
            }
            for (name, path) in [
                ("sequence-preview.mp4", Path::new(&preview.source)),
                ("sequence-export.mp4", output.as_path()),
            ] {
                let mut staged = crate::safe_output::StagedOutput::new(&folder.join(name)).unwrap();
                staged.file().write_all(&fs::read(path).unwrap()).unwrap();
                staged.commit().unwrap();
            }
        }

        for time in ["0.1", "0.35"] {
            let a = pixel(Path::new(&preview.source), time);
            let b = pixel(&output, time);
            assert!(a.iter().zip(b).all(|(x, y)| x.abs_diff(y) < 8));
        }
    }
    #[test]
    fn cumulative_frame_allocation_avoids_rounding_drift() {
        let (_fixture, mut snapshot) = fixture();
        let mut clip = snapshot.project.clips[0].clone();
        let base = snapshot.project.assets[0]
            .info
            .time_base
            .as_deref()
            .unwrap();
        clip.source_in = VideoTime::from_seconds(0.0, base).unwrap();
        clip.source_out = VideoTime::from_seconds(0.04, base).unwrap();
        snapshot.project.clips = (0..100)
            .map(|_| {
                let mut c = clip.clone();
                c.id = video_project::id();
                c
            })
            .collect();
        assert_eq!(
            &frame_allocation(&snapshot.project).unwrap()[..5],
            &[1, 1, 2, 1, 1]
        );
        assert_eq!(
            frame_allocation(&snapshot.project)
                .unwrap()
                .iter()
                .sum::<u64>(),
            120
        );
        snapshot.project.frame_rate = 60;
        assert_eq!(
            frame_allocation(&snapshot.project)
                .unwrap()
                .iter()
                .sum::<u64>(),
            240
        );
    }
    struct TestControl {
        mode: &'static str,
        destination: PathBuf,
        source: PathBuf,
        cancel: std::sync::atomic::AtomicBool,
        publish: std::sync::atomic::AtomicBool,
    }
    impl crate::video_export::Control for TestControl {
        fn cancelled(&self) -> bool {
            self.cancel.load(std::sync::atomic::Ordering::SeqCst)
        }
        fn update(&self, stage: crate::video_export::Stage, fraction: Option<f64>, _: u64) {
            if self.mode == "cancel" && stage == crate::video_export::Stage::Encoding {
                self.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            if self.mode == "source"
                && stage == crate::video_export::Stage::VerifyingSource
                && fraction.is_some_and(|f| f > 0.9)
            {
                fs::write(&self.source, b"external source change").unwrap();
            }
        }
        fn begin_publish(&self) -> Result<(), String> {
            if self.cancelled() {
                return Err("Cancelled before publication".into());
            }
            if self.mode == "race" {
                fs::write(&self.destination, b"existing output").unwrap();
            }
            self.publish
                .store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
        fn available_space(&self, _: &Path) -> Result<Option<u64>, String> {
            Ok(Some(if self.mode == "space" { 0 } else { u64::MAX }))
        }
    }
    #[test]
    fn publication_pipeline_rejects_cancellation_space_source_changes_and_racing_destinations() {
        for mode in ["success", "cancel", "space", "source", "race"] {
            let (fixture, snapshot) = fixture();
            let destination = fixture.0.join("published.mp4");
            let control = std::sync::Arc::new(TestControl {
                mode,
                destination: destination.clone(),
                source: fixture.0.join(&snapshot.project.assets[0].relative_path),
                cancel: std::sync::atomic::AtomicBool::new(false),
                publish: std::sync::atomic::AtomicBool::new(false),
            });
            let result = crate::video_export::export_sequence_file(
                crate::video_export::SequenceRequest {
                    engine: &engine(),
                    snapshot: &snapshot,
                    destination: &destination,
                    size: ExportSize::Project,
                    acknowledge_first_audio: false,
                },
                control.clone(),
            );
            if mode == "success" {
                let report = result.unwrap();
                assert!((report.duration_seconds - 0.5).abs() < 0.01);
                assert!(control.publish.load(std::sync::atomic::Ordering::SeqCst));
            } else {
                assert!(result.is_err(), "{mode}");
                if mode == "race" {
                    assert_eq!(fs::read(&destination).unwrap(), b"existing output");
                } else {
                    assert!(!destination.exists(), "{mode}");
                }
            }
            assert!(
                !fs::read_dir(&fixture.0).unwrap().any(|entry| entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".openphoto-")),
                "{mode}"
            );
        }
    }
    fn replace_first_source(snapshot: &mut Snapshot, folder: &Path, file: &Path) {
        let info = video_probe::probe(&engine(), file, || false).unwrap();
        let p = video_project::new_project(folder.to_str().unwrap(), file, info).unwrap();
        let old = snapshot.project.assets[0].id.clone();
        let base = p.asset.info.time_base.as_deref().unwrap();
        for clip in &mut snapshot.project.clips {
            if clip.asset_id == old {
                clip.asset_id = p.asset.id.clone();
                clip.source_in =
                    VideoTime::from_seconds(clip.source_in.seconds().unwrap(), base).unwrap();
                clip.source_out =
                    VideoTime::from_seconds(clip.source_out.seconds().unwrap(), base).unwrap();
            }
        }
        snapshot.roots.remove(&old);
        snapshot
            .roots
            .insert(p.asset.id.clone(), folder.to_string_lossy().into_owned());
        snapshot.project.assets[0] = p.asset;
    }
    #[test]
    fn delayed_original_audio_and_multiple_track_acknowledgement_are_preserved() {
        let (fixture, mut snapshot) = fixture();
        let original = fixture.0.join(&snapshot.project.assets[0].relative_path);
        let delayed = fixture.0.join("delayed.mp4");
        let args: Vec<OsString> = ["-v", "error", "-i"]
            .into_iter()
            .map(Into::into)
            .chain([original.clone().into()])
            .chain(["-itsoffset", "0.08", "-i"].into_iter().map(Into::into))
            .chain([original.clone().into()])
            .chain(
                ["-map", "0:v:0", "-map", "1:a:0", "-c", "copy", "-t", "0.4"]
                    .into_iter()
                    .map(Into::into),
            )
            .chain([delayed.clone().into()])
            .collect();
        video_engine::run(
            &video_engine::tool(&engine(), "ffmpeg").unwrap(),
            &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
            Duration::from_secs(30),
        )
        .unwrap();
        replace_first_source(&mut snapshot, &fixture.0, &delayed);
        let output = fixture.0.join("delayed-output.mp4");
        finish(&snapshot, &output, false);
        let bytes = video_engine::run(
            &video_engine::tool(&engine(), "ffmpeg").unwrap(),
            &[
                "-v".as_ref(),
                "error".as_ref(),
                "-i".as_ref(),
                output.as_os_str(),
                "-t".as_ref(),
                "0.01".as_ref(),
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
        let values = bytes.as_chunks::<4>().0;
        let initial = (values
            .iter()
            .map(|v| f64::from(f32::from_le_bytes(*v)).powi(2))
            .sum::<f64>()
            / values.len() as f64)
            .sqrt();
        assert!(initial < 0.002, "{initial}");
        assert!(rms(&output, "0.08") > 0.02);
        let multi = fixture.0.join("multiple-tracks.mp4");
        let args: Vec<OsString> = ["-v", "error", "-i"]
            .into_iter()
            .map(Into::into)
            .chain([original.into()])
            .chain(
                [
                    "-f",
                    "lavfi",
                    "-i",
                    "sine=frequency=440:sample_rate=48000:duration=0.4",
                    "-map",
                    "0:v:0",
                    "-map",
                    "0:a:0",
                    "-map",
                    "1:a:0",
                    "-c:v",
                    "copy",
                    "-c:a",
                    "aac",
                    "-ac",
                    "2",
                    "-t",
                    "0.4",
                ]
                .into_iter()
                .map(Into::into),
            )
            .chain([multi.clone().into()])
            .collect();
        video_engine::run(
            &video_engine::tool(&engine(), "ffmpeg").unwrap(),
            &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
            Duration::from_secs(30),
        )
        .unwrap();
        replace_first_source(&mut snapshot, &fixture.0, &multi);
        let output = fixture.0.join("first-track.mp4");
        let rejected = compile(
            &engine(),
            &snapshot,
            &output,
            RenderOptions {
                preview: false,
                size: ExportSize::Project,
                acknowledge_first_audio: false,
            },
            || false,
            |_| {},
        );
        assert!(rejected.err().unwrap().contains("first audio"));
        let mut accepted = compile(
            &engine(),
            &snapshot,
            &output,
            RenderOptions {
                preview: false,
                size: ExportSize::Project,
                acknowledge_first_audio: true,
            },
            || false,
            |_| {},
        )
        .unwrap();
        video_render::encode(&engine(), &accepted.plan, || false, |_| {}).unwrap();
        let info =
            video_render::validate_output(&engine(), &accepted.plan, &output, || false).unwrap();
        assert_eq!(info.audio.len(), 1);
        accepted.close().unwrap();
        snapshot.revalidate(|| false).unwrap();
    }
    #[test]
    fn joined_uhd_portrait_square_and_hd_caps_produce_valid_mp4() {
        let (fixture, mut snapshot) = fixture();
        for (index, (width, height, size, expected)) in [
            (3840, 2160, ExportSize::Uhd4k, (3840, 2160)),
            (2160, 3840, ExportSize::Uhd4k, (2160, 3840)),
            (3840, 2160, ExportSize::Hd1080, (1920, 1080)),
            (1080, 1080, ExportSize::Project, (1080, 1080)),
        ]
        .into_iter()
        .enumerate()
        {
            snapshot.project.output.width = width;
            snapshot.project.output.height = height;
            let output = fixture.0.join(format!("canvas-{index}.mp4"));
            let mut compiled = compile(
                &engine(),
                &snapshot,
                &output,
                RenderOptions {
                    preview: false,
                    size,
                    acknowledge_first_audio: false,
                },
                || false,
                |_| {},
            )
            .unwrap();
            video_render::encode(&engine(), &compiled.plan, || false, |_| {}).unwrap();
            let info = video_render::validate_output(&engine(), &compiled.plan, &output, || false)
                .unwrap();
            video_render::decode_check(&engine(), &output, || false, |_| {}).unwrap();
            assert_eq!((info.width, info.height), expected);
            assert_eq!(info.frame_count, Some(15));
            compiled.close().unwrap();
        }
        snapshot.revalidate(|| false).unwrap();
    }
}
