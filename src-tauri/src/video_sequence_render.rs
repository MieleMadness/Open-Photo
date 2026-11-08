//! Bounded hard-cut compiler: one source decoder at a time, one video encode per clip.
#[cfg(all(test, windows))]
include!("crossfade_preview_part_decoder.rs");
#[cfg(all(test, windows))]
include!("crossfade_preview_titles.rs");
include!("sequence_audio_filter.rs");
include!("sequence_mux.rs");
#[cfg(windows)]
include!("sequence_crossfade_render.rs");
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
    pub(crate) transition: Option<crate::video_workspace::transitions::RenderBinding>,
}
impl Snapshot {
    pub fn validate(&self) -> Result<(), String> {
        self.validate_with_cancel(&|| false)
    }
    pub(crate) fn validate_with_cancel(&self, cancel: &impl Fn() -> bool) -> Result<(), String> {
        crate::video_sequence::validate(&self.project)?;
        if let Some(binding) = &self.transition {
            crate::crossfade_layer_clock::plan(
                &self.project,
                &binding
                    .edges
                    .iter()
                    .map(|edge| crate::crossfade_clock::Boundary {
                        from_clip_id: &edge.from_clip_id,
                        to_clip_id: &edge.to_clip_id,
                        frames: edge.frames,
                    })
                    .collect::<Vec<_>>(),
            )?;
        }
        for source in &self.project.audio_sources {
            let root = self
                .roots
                .get(&source.id)
                .ok_or("Relink every independent audio source before rendering")?;
            let path = video_probe::safe_audio_path(
                root,
                &Path::new(root)
                    .join(&source.relative_path)
                    .to_string_lossy(),
            )?;
            let (bytes, hash) = video_project::fingerprint(&path, cancel)?;
            if bytes != source.bytes || hash != source.sha256 {
                return Err("Independent audio file has changed; relink the original".into());
            }
        }
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
            crate::video_sequence::single(&self.project, asset, clip.clone())?,
        ))
    }
    pub fn revalidate(&self, cancel: impl Fn() -> bool) -> Result<(), String> {
        if let Some(binding) = &self.transition {
            if !binding.current()? {
                return Err("Transition preview or export became stale".into());
            }
        }
        for asset in &self.project.assets {
            if !self.project.clips.iter().any(|c| c.asset_id == asset.id)
                && !self
                    .project
                    .audio_tracks
                    .iter()
                    .any(|track| track.asset_id == asset.id)
            {
                continue;
            }
            let root = self
                .roots
                .get(&asset.id)
                .ok_or("Relink this timeline source")?;
            crate::sequence_media::source(root, asset, &cancel)?;
        }
        // Independent audio must satisfy the same pre-publication integrity
        // gate as video/photos. A replacement track must never be published.
        for source in &self.project.audio_sources {
            if !self
                .project
                .audio_tracks
                .iter()
                .any(|track| track.asset_id == source.id)
            {
                continue;
            }
            if cancel() {
                return Err("Video source verification cancelled.".into());
            }
            let root = self
                .roots
                .get(&source.id)
                .ok_or("Relink this independent audio source")?;
            let candidate = Path::new(root).join(&source.relative_path);
            let path = video_probe::safe_audio_path(root, &candidate.to_string_lossy())?;
            let (bytes, hash) = video_project::fingerprint(&path, &cancel)?;
            if bytes != source.bytes || hash != source.sha256 {
                return Err("Independent audio file changed; relink the original.".into());
            }
        }
        if let Some(binding) = &self.transition {
            if !binding.current()? {
                return Err("Transition preview or export became stale".into());
            }
        }
        Ok(())
    }
    pub fn duration(&self) -> Result<f64, String> {
        if let Some(binding) = &self.transition {
            let timing = crate::crossfade_layer_clock::plan(
                &self.project,
                &binding
                    .edges
                    .iter()
                    .map(|edge| crate::crossfade_clock::Boundary {
                        from_clip_id: &edge.from_clip_id,
                        to_clip_id: &edge.to_clip_id,
                        frames: edge.frames,
                    })
                    .collect::<Vec<_>>(),
            )?;
            return Ok(timing.total_frames as f64 / f64::from(timing.fps));
        }
        self.project
            .clips
            .iter()
            .map(|c| Ok(c.source_out.seconds()? - c.source_in.seconds()?))
            .sum()
    }
    pub fn has_photos(&self) -> bool {
        self.project.clips.iter().any(|c| {
            self.project
                .assets
                .iter()
                .any(|a| a.id == c.asset_id && a.is_photo())
        })
    }
    pub fn bitrate(&self, size: ExportSize) -> Result<u64, String> {
        let canvas = crate::video_presets::resolve(&self.project.output, size);
        Ok(
            (12_000_000 * u64::from(canvas.width) * u64::from(canvas.height) / (1920 * 1080))
                .clamp(12_000_000, 48_000_000),
        )
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
    authorization: Option<crate::video_workspace::transitions::RenderBinding>,
    #[cfg(windows)]
    segment_pins: Vec<crate::crossfade_windows_guard::PinnedMedia>,
    scratch: Option<Scratch>,
    #[cfg(windows)]
    source_pins: Option<std::sync::Arc<crate::crossfade_windows_guard::PinnedMedia>>,
}
impl Compiled {
    pub fn single(plan: Plan) -> Self {
        Self {
            plan,
            encoded_fraction: 0.0,
            authorization: None,
            #[cfg(windows)]
            segment_pins: vec![],
            scratch: None,
            #[cfg(windows)]
            source_pins: None,
        }
    }
    pub fn close(&mut self) -> Result<(), String> {
        #[cfg(windows)]
        self.segment_pins.clear();
        if let Some(scratch) = &mut self.scratch {
            scratch.close()?;
        }
        Ok(())
    }
    pub(crate) fn verify_authorization(&self) -> Result<(), String> {
        if let Some(binding) = &self.authorization {
            if !binding.current()? {
                return Err("Project or workspace changed before publication".into());
            }
        }
        Ok(())
    }
}

/// Rounding cumulative boundaries bounds total rounding error to half an output frame.
pub(crate) fn frame_allocation(project: &Sequence) -> Result<Vec<u64>, String> {
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
struct PartSource {
    source: PathBuf,
    photo: bool,
    video_stream: usize,
    audio_stream: Option<usize>,
    start: f64,
    duration: f64,
    filter: String,
    width: u32,
    height: u32,
}
impl PartSource {
    fn video(p: video_render::Prepared) -> Self {
        Self {
            source: p.source,
            photo: false,
            video_stream: p.info.video_stream,
            audio_stream: if p.audio_span.is_some() {
                p.info.audio.first().map(|a| a.stream)
            } else {
                None
            },
            start: p.start,
            duration: p.duration,
            filter: p.filter,
            width: p.width,
            height: p.height,
        }
    }
}
#[derive(Clone, Copy)]
struct PartSegment {
    start: u64,
    end: u64,
    rgb: bool,
}
struct PartOptions<'a> {
    frames: u64,
    fps: u32,
    audio: bool,
    bitrate: u64,
    output: &'a Path,
    max_bytes: u64,
    titles_filter: String,
    segment: Option<PartSegment>,
}
fn part_args(
    prepared: &PartSource,
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
        titles_filter,
        segment,
    } = options;
    let selected = segment.unwrap_or(PartSegment {
        start: 0,
        end: frames,
        rgb: false,
    });
    let duration = (selected.end - selected.start) as f64 / f64::from(fps);
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
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    if prepared.photo {
        args.extend([
            "-format_whitelist".into(),
            "image2,png_pipe".into(),
            "-f".into(),
            "image2".into(),
            "-loop".into(),
            "1".into(),
            "-framerate".into(),
            fps.to_string().into(),
        ]);
    } else {
        args.extend([
            "-format_whitelist".into(),
            "mov,matroska,webm,avi".into(),
            "-ss".into(),
            format!("{:.9}", prepared.start).into(),
        ]);
    }
    args.extend(["-i".into(), prepared.source.clone().into()]);
    let real_audio = audio && !edits.muted && prepared.audio_stream.is_some();
    if audio && !real_audio {
        args.extend([
            "-f".into(),
            "lavfi".into(),
            "-i".into(),
            "anullsrc=r=48000:cl=stereo".into(),
        ]);
    }
    let mut filter = normalized_video_filter(prepared, frames, fps, &titles_filter);
    if segment.is_some() {
        filter.push_str(&format!(
            ",trim=start_frame={}:end_frame={},setpts=N/({fps}*TB)",
            selected.start, selected.end
        ));
    }
    if selected.rgb {
        filter.push_str(",format=bgr0");
    }
    args.extend([
        "-map".into(),
        format!("0:{}", prepared.video_stream).into(),
        "-vf".into(),
        filter.into(),
    ]);
    if audio {
        let (map, mut filter) = if real_audio {
            // Preserve decoded audio timestamps, including encoder delay and a late first sample.
            (format!("0:{}",prepared.audio_stream.expect("real audio")),format!("atrim=start=0:end={:.9},aformat=sample_fmts=flt:channel_layouts=stereo,aresample=48000:async=1:first_pts=0,volume={:.4},apad=whole_len={samples},atrim=end_sample={samples},asetpts=N/SR/TB",prepared.duration,f64::from(edits.gain_percent)/100.0))
        } else {
            ("1:a:0".into(),format!("aformat=sample_fmts=flt:channel_layouts=stereo,atrim=end_sample={samples},asetpts=N/SR/TB"))
        };
        if segment.is_some() {
            filter.push_str(&format!(
                ",atrim=start_sample={}:end_sample={},asetpts=N/SR/TB",
                selected.start * 48000 / u64::from(fps),
                selected.end * 48000 / u64::from(fps)
            ));
        }
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
    ]);
    if selected.rgb {
        args.extend([
            "-c:v".into(),
            "ffv1".into(),
            "-level".into(),
            "3".into(),
            "-g".into(),
            "1".into(),
            "-pix_fmt".into(),
            "bgr0".into(),
        ]);
    } else {
        args.extend([
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
        ]);
    }
    args.extend([
        "-colorspace".into(),
        "bt709".into(),
        "-color_primaries".into(),
        "bt709".into(),
        "-color_trc".into(),
        "bt709".into(),
        "-color_range".into(),
        if selected.rgb { "pc" } else { "tv" }.into(),
    ]);
    progress_args(&mut args, output, "matroska", max_bytes);
    args
}

// One normalization graph for encoded parts and the gated RGB part decoder.
fn normalized_video_filter(
    prepared: &PartSource,
    frames: u64,
    fps: u32,
    titles_filter: &str,
) -> String {
    let duration = frames as f64 / f64::from(fps);
    format!(
        "trim=start=0:end={:.9},setpts=PTS-STARTPTS,fps=fps={fps}:start_time=0,tpad=stop_mode=clone:stop_duration={:.9},trim=end_frame={frames},setpts=N/({fps}*TB),{},scale=out_color_matrix=bt709:out_range=tv,format=yuv420p,setsar=1{}",
        prepared.duration,
        duration + 1.0 / f64::from(fps),
        prepared.filter,
        titles_filter
    )
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
    compiled.verify_authorization()?;
    Ok(crate::video_preview::PreviewAssets {
        transition: snapshot.transition.clone(),
        #[cfg(windows)]
        source_ownership: compiled.source_pins.clone(),
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
    snapshot.validate_with_cancel(&cancel)?;
    if snapshot
        .transition
        .as_ref()
        .is_some_and(|binding| !binding.edges.is_empty())
    {
        #[cfg(windows)]
        return compile_crossfade(engine, snapshot, output, options, cancel, progress);
        #[cfg(not(windows))]
        return Err("Crossfade rendering requires Windows file protection".into());
    }
    #[cfg(windows)]
    let sources = if snapshot.transition.is_some() {
        Some(pin_sequence_sources(snapshot, &cancel)?)
    } else {
        None
    };
    if snapshot.project.clips.len() == 1
        && !snapshot.has_photos()
        && snapshot.project.titles.is_empty()
        && snapshot.project.audio_tracks.is_empty()
        && snapshot.transition.is_none()
    {
        let (root, p) = snapshot.single(0)?;
        return Ok(Compiled {
            plan: video_render::plan(engine, &root, &p, output, options, cancel)?,
            encoded_fraction: 0.0,
            authorization: None,
            #[cfg(windows)]
            segment_pins: vec![],
            scratch: None,
            #[cfg(windows)]
            source_pins: None,
        });
    }
    let scratch = Scratch::new(output.parent().ok_or("Output folder unavailable")?)?;
    let directory = scratch.path().to_owned();
    let result = (|| {
        let frames = frame_allocation(&snapshot.project)?;
        let total_frames: u64 = frames.iter().sum();
        let fps = snapshot.project.frame_rate;
        let duration = total_frames as f64 / f64::from(fps);
        let title_spans = snapshot
            .project
            .titles
            .iter()
            .map(|title| crate::crossfade_layer_clock::title_span(title, fps, total_frames))
            .collect::<Result<Vec<_>, String>>()?;
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
        for (index, frame_count) in frames.iter().enumerate() {
            if cancel() {
                return Err("Video render cancelled".into());
            }
            let prepared =
                prepare_sequence_part(engine, snapshot, index, *frame_count, options, &cancel)?;
            #[cfg(windows)]
            if let Some(sources) = &sources {
                sources.verify_prepared(snapshot, index, &prepared)?;
            }
            contexts.push(prepared);
        }
        let audio = !snapshot.project.audio_tracks.is_empty()
            || contexts
                .iter()
                .enumerate()
                .any(|(i, p)| !snapshot.project.clips[i].edits.muted && p.audio_stream.is_some());
        if options.preview {
            let required = (duration
                * (2.0 * bitrate as f64 / 8.0 * 1.25 + if audio { 384000.0 } else { 0.0 }))
            .ceil() as u64
                + 64 * 1024 * 1024
                + if snapshot.has_photos() {
                    256 * 1024 * 1024
                } else {
                    0
                };
            if crate::video_export::available_space(&directory)?.is_some_and(|free| free < required)
            {
                return Err(format!(
                    "Not enough temporary space for the sequence preview (allow {} MB)",
                    required.div_ceil(1024 * 1024)
                ));
            }
        }
        for (index, (mut prepared, count)) in contexts.into_iter().zip(frames.iter()).enumerate() {
            if cancel() {
                return Err("Video render cancelled".into());
            }
            let normalized_photo =
                prepare_sequence_photo(&mut prepared, snapshot, index, &directory)?;
            let part = directory.join(format!("clip-{index:04}.mkv"));
            let mut titles_filter = sequence_title_filter(
                snapshot,
                &directory,
                &format!("title-{index:04}"),
                completed,
                *count,
                &title_spans,
            )?;
            // Dip-to-black fades are applied to normalized clip frames before concat.
            // Each side fades for half the saved duration; no overlapping frames or timing shift.
            // This first transition type is video-only: linked and layered audio remain untouched.
            let entering = snapshot
                .project
                .transitions
                .iter()
                .find(|transition| transition.to_clip_id == snapshot.project.clips[index].id)
                .map(|transition| transition.duration_seconds / 2.0)
                .unwrap_or(0.0);
            let exiting = snapshot
                .project
                .transitions
                .iter()
                .find(|transition| transition.from_clip_id == snapshot.project.clips[index].id)
                .map(|transition| transition.duration_seconds / 2.0)
                .unwrap_or(0.0);
            let normalized_seconds = *count as f64 / f64::from(fps);
            let overlays = std::mem::take(&mut titles_filter);
            if entering + exiting + 1.0 / f64::from(fps) > normalized_seconds {
                return Err("Normalized clip is too short for video dip-to-black".into());
            }
            if entering > 0.0 {
                titles_filter.push_str(&format!(",fade=t=in:st=0:d={entering:.6}"));
            }
            if exiting > 0.0 {
                let start = normalized_seconds - exiting;
                titles_filter.push_str(&format!(",fade=t=out:st={start:.6}:d={exiting:.6}"));
            }
            // Global titles remain visible over a video dip, matching composed previews.
            titles_filter.push_str(&overlays);
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
                    titles_filter,
                    segment: None,
                },
            );
            #[cfg(windows)]
            let caption_pins = pin_caption_files(
                snapshot,
                &directory,
                &format!("title-{index:04}"),
                completed,
                *count,
                &title_spans,
                &cancel,
            )?;
            let length = *count as f64 / f64::from(fps);
            let before = completed as f64 / f64::from(fps);
            let notify = progress.clone();
            video_engine::run_progress_in(
                &video_engine::tool(engine, "ffmpeg")?,
                &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                Duration::from_secs(21600),
                &cancel,
                move |p| {
                    notify(
                        ((before + p.seconds.clamp(0.0, length)) / duration).clamp(0.0, 1.0) * 0.75,
                    )
                },
                Some(&directory),
            )?;
            #[cfg(windows)]
            drop(caption_pins);
            if let Some(path) = normalized_photo {
                fs::remove_file(path).map_err(|e| e.to_string())?;
            }
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
            info = Some(result);
            completed += count;
            manifest.push_str(&format!("file clip-{index:04}.mkv\nduration {length:.9}\n"));
        }
        let list = directory.join("sequence.ffconcat");
        fs::File::create(&list)
            .and_then(|mut f| f.write_all(manifest.as_bytes()))
            .map_err(|e| e.to_string())?;
        Ok(Compiled {
            plan: mux_plan(
                engine,
                SequenceMux {
                    snapshot,
                    directory: &directory,
                    output,
                    total_frames,
                    width,
                    height,
                    info,
                    audio,
                    max_bytes,
                    crossfade: false,
                    #[cfg(windows)]
                    verified_sources: sources.as_ref().map(|s| &s.paths),
                },
                &cancel,
            )?,
            encoded_fraction: 0.75,
            authorization: snapshot.transition.clone(),
            #[cfg(windows)]
            segment_pins: vec![],
            scratch: Some(scratch),
            #[cfg(windows)]
            source_pins: sources.map(|s| s.pins),
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
    include!("sequence_phase3_tests.rs");
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
            transition: None,
            project: Sequence {
                schema_version: 3,
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
                titles: vec![],
                audio_tracks: vec![],
                audio_sources: vec![],
                transitions: vec![],
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
            snapshot.project.assets.push(p.asset.into());
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
    #[test]
    fn crossfade_receipts_reject_swapped_and_modified_normalized_parts() {
        let (fixture, snapshot) = fixture();
        let owned = fs::canonicalize(&fixture.0).unwrap();
        let paths = [
            owned.join("normalized-0.mkv"),
            owned.join("normalized-1.mkv"),
        ];
        fs::write(&paths[0], b"normalized first").unwrap();
        fs::write(&paths[1], b"normalized second").unwrap();
        let frames = [6_u64, 9_u64];
        let mut receipts = Vec::new();
        for (i, clip) in snapshot.project.clips.iter().enumerate() {
            let asset = snapshot
                .project
                .assets
                .iter()
                .find(|asset| asset.id == clip.asset_id)
                .unwrap();
            let (_, part_sha256) = video_project::fingerprint(&paths[i], || false).unwrap();
            receipts.push(crate::crossfade_provenance::NormalizedReceipt {
                clip_id: clip.id.clone(),
                asset_id: asset.id.clone(),
                saved_source_sha256: asset.sha256.clone(),
                part: paths[i].clone(),
                part_sha256,
                frames: frames[i],
            });
        }
        let verify = || {
            crate::crossfade_provenance::verify_ordered_receipts(
                &snapshot,
                &receipts,
                &paths,
                &frames,
                || false,
            )
        };
        verify().unwrap();
        let fps = snapshot.project.frame_rate;
        let clocks = [
            crate::crossfade_clock::ClipClock {
                id: "first",
                frames: frames[0],
            },
            crate::crossfade_clock::ClipClock {
                id: "second",
                frames: frames[1],
            },
        ];
        let boundary = [crate::crossfade_clock::Boundary {
            from_clip_id: "first",
            to_clip_id: "second",
            frames: 3,
        }];
        let plan =
            crate::crossfade_preflight::plan_allocated(&clocks, &boundary, &[true, false], fps)
                .unwrap();
        let bundled_engine = engine();
        let scratch = owned.join("untrusted-receipt-render.mp4");
        let published = owned.join("untrusted-receipt-published.mp4");
        let request = crate::crossfade_command::CrossfadeExecution {
            engine: &bundled_engine,
            plan: &plan,
            inputs: &paths,
            output: &scratch,
            fps,
            max_bytes: 64 * 1024 * 1024,
        };
        // The receipt boundary must reject an untrusted swap before it could
        // attempt FFmpeg or commit any user-visible MP4.
        let rejected = crate::crossfade_pipeline::render_verify_publish_snapshot_receipts(
            request,
            &published,
            &snapshot,
            &[
                crate::crossfade_provenance::NormalizedReceipt {
                    clip_id: receipts[1].clip_id.clone(),
                    asset_id: receipts[1].asset_id.clone(),
                    saved_source_sha256: receipts[1].saved_source_sha256.clone(),
                    part: paths[1].clone(),
                    part_sha256: receipts[1].part_sha256.clone(),
                    frames: frames[1],
                },
                crate::crossfade_provenance::NormalizedReceipt {
                    clip_id: receipts[0].clip_id.clone(),
                    asset_id: receipts[0].asset_id.clone(),
                    saved_source_sha256: receipts[0].saved_source_sha256.clone(),
                    part: paths[0].clone(),
                    part_sha256: receipts[0].part_sha256.clone(),
                    frames: frames[0],
                },
            ],
            || false,
        );
        assert!(rejected.is_err());
        assert!(!scratch.exists());
        assert!(!published.exists());
        assert!(crate::crossfade_provenance::verify_ordered_receipts(
            &snapshot,
            &receipts,
            &[paths[1].clone(), paths[0].clone()],
            &frames,
            || false,
        )
        .is_err());
        assert!(crate::crossfade_provenance::verify_ordered_receipts(
            &snapshot,
            &receipts,
            &paths,
            &frames,
            || true,
        )
        .is_err());
        fs::write(&paths[0], b"normalized FIRST").unwrap();
        assert!(verify().is_err());
    }

    #[test]
    fn crossfade_manifest_uses_saved_hashes_and_rejects_changed_video() {
        let (fixture, snapshot) = fixture();
        let manifest =
            crate::crossfade_snapshot::SourceManifest::from_snapshot(&snapshot, || false).unwrap();
        assert_eq!(manifest.expectations().len(), 2);
        manifest.verify(|| false).unwrap();
        let clock_clips = [
            crate::crossfade_clock::ClipClock {
                id: "a",
                frames: 12,
            },
            crate::crossfade_clock::ClipClock {
                id: "b",
                frames: 12,
            },
        ];
        let boundaries = [crate::crossfade_clock::Boundary {
            from_clip_id: "a",
            to_clip_id: "b",
            frames: 3,
        }];
        let plan = crate::crossfade_preflight::plan_allocated(
            &clock_clips,
            &boundaries,
            &[true, false],
            30,
        )
        .unwrap();
        let nonexistent_input = [fixture.0.join("never-normalized.mkv")];
        let scratch = fixture.0.join("never-rendered.mp4");
        let publication = fixture.0.join("never-published.mp4");
        let bundled_engine = engine();
        let request = crate::crossfade_command::CrossfadeExecution {
            engine: &bundled_engine,
            plan: &plan,
            inputs: &nonexistent_input,
            output: &scratch,
            fps: 30,
            max_bytes: 64 * 1024 * 1024,
        };
        assert!(crate::crossfade_pipeline::render_verify_publish_snapshot(
            request,
            &publication,
            &snapshot,
            || false,
        )
        .is_err());
        assert!(!scratch.exists());
        assert!(!publication.exists());
        let changed = fixture.0.join("source-0 世界.mp4");
        let mut bytes = fs::read(&changed).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        fs::write(&changed, bytes).unwrap();
        assert!(manifest.verify(|| false).is_err());
        assert!(
            crate::crossfade_snapshot::SourceManifest::from_snapshot(&snapshot, || false).is_err()
        );
        assert!(manifest.verify(|| true).is_err());
    }

    #[test]
    fn audio_track_mixes_into_native_project_preview() {
        let (fixture, mut snapshot) = fixture();
        let source = &snapshot.project.assets[0];
        assert!(!source.video().unwrap().info.audio.is_empty());
        snapshot
            .project
            .audio_tracks
            .push(crate::video_sequence::AudioTrack {
                id: video_project::id(),
                asset_id: source.id.clone(),
                start_seconds: 0.2,
                source_in_seconds: 0.0,
                duration_seconds: 0.2,
                gain_percent: 60,
                fade_in_seconds: 0.04,
                fade_out_seconds: 0.04,
            });
        let preview = build_preview(&engine(), &snapshot, &fixture.0, || false).unwrap();
        assert_eq!(preview.info.audio.len(), 1);
        assert!(Path::new(&preview.source).exists());
    }
    #[test]
    fn standalone_wav_is_mixed_into_a_verified_project_preview() {
        let (fixture, mut snapshot) = fixture();
        let audio = fixture.0.join("music standalone 世界.wav");
        video_engine::run(
            &video_engine::tool(&engine(), "ffmpeg").unwrap(),
            &[
                "-nostdin".as_ref(),
                "-v".as_ref(),
                "error".as_ref(),
                "-f".as_ref(),
                "lavfi".as_ref(),
                "-i".as_ref(),
                "sine=frequency=440:sample_rate=48000:duration=0.5".as_ref(),
                "-c:a".as_ref(),
                "pcm_s16le".as_ref(),
                audio.as_os_str(),
            ],
            Duration::from_secs(30),
        )
        .unwrap();
        let info = video_probe::probe_audio_only(&engine(), &audio, || false).unwrap();
        let peaks = video_probe::decoded_waveform(
            &engine(),
            &audio,
            info.stream,
            0.0,
            0.3,
            "mp3,wav,flac,mov",
        )
        .unwrap();
        assert_eq!(peaks.len(), 64);
        assert!(peaks.iter().any(|peak| *peak > 0));
        assert!(video_probe::decoded_waveform(
            &engine(),
            &audio,
            info.stream,
            0.0,
            121.0,
            "mp3,wav,flac,mov",
        )
        .is_err());
        let (bytes, sha256) = video_project::fingerprint(&audio, || false).unwrap();
        let id = video_project::id();
        snapshot
            .project
            .audio_sources
            .push(crate::video_sequence::AudioSource {
                id: id.clone(),
                relative_path: "music standalone 世界.wav".into(),
                sha256,
                bytes,
                info,
            });
        snapshot
            .roots
            .insert(id.clone(), fixture.0.to_string_lossy().into_owned());
        snapshot
            .project
            .audio_tracks
            .push(crate::video_sequence::AudioTrack {
                id: video_project::id(),
                asset_id: id.clone(),
                start_seconds: 0.1,
                source_in_seconds: 0.0,
                duration_seconds: 0.3,
                gain_percent: 75,
                fade_in_seconds: 0.05,
                fade_out_seconds: 0.05,
            });
        let manifest =
            crate::crossfade_snapshot::SourceManifest::from_snapshot(&snapshot, || false).unwrap();
        assert_eq!(manifest.expectations().len(), 3);
        manifest.verify(|| false).unwrap();
        let preview = build_preview(&engine(), &snapshot, &fixture.0, || false).unwrap();
        assert_eq!(preview.info.audio.len(), 1);
        assert!(Path::new(&preview.source).exists());
        let altered = snapshot.project.audio_sources[0].sha256.clone();
        snapshot.project.audio_sources[0].sha256 = "0".repeat(64);
        assert!(build_preview(&engine(), &snapshot, &fixture.0, || false).is_err());
        snapshot.project.audio_sources[0].sha256 = altered;
        snapshot.revalidate(|| false).unwrap();
        // The preview was valid, but a changed track must fail final
        // source revalidation even if all video sources still match.
        fs::write(&audio, b"modified music after snapshot").unwrap();
        assert!(snapshot.revalidate(|| false).is_err());
        assert!(manifest.verify(|| false).is_err());
    }
    #[test]
    fn dip_to_black_preserves_project_frame_count_and_fades_the_cut() {
        let (fixture, mut snapshot) = fixture();
        let from_clip_id = snapshot.project.clips[0].id.clone();
        let to_clip_id = snapshot.project.clips[1].id.clone();
        snapshot
            .project
            .transitions
            .push(crate::video_sequence::CutTransition {
                from_clip_id,
                to_clip_id,
                kind: crate::video_sequence::CutTransitionKind::DipToBlack,
                duration_seconds: 0.16,
            });
        crate::video_sequence::validate(&snapshot.project).unwrap();
        let preview = build_preview(&engine(), &snapshot, &fixture.0, || false).unwrap();
        assert_eq!(preview.info.frame_count, Some(15));
        // First clip ends around 0.2 s; incoming clip starts dark, then returns to blue.
        let near_cut = pixel(Path::new(&preview.source), "0.200");
        let after = pixel(Path::new(&preview.source), "0.400");
        assert!(
            near_cut.iter().all(|channel| *channel < 85),
            "cut was not dark: {near_cut:?}"
        );
        assert!(after[2] > 90, "blue clip did not fade back in: {after:?}");
    }

    #[test]
    fn dip_to_black_preview_and_export_share_video_frames_and_audio_timing() {
        let (fixture, mut snapshot) = fixture();
        let left = snapshot.project.clips[0].id.clone();
        let right = snapshot.project.clips[1].id.clone();
        snapshot
            .project
            .transitions
            .push(crate::video_sequence::CutTransition {
                from_clip_id: left,
                to_clip_id: right,
                kind: crate::video_sequence::CutTransitionKind::DipToBlack,
                duration_seconds: 0.16,
            });
        let preview = build_preview(&engine(), &snapshot, &fixture.0, || false).unwrap();
        let output = fixture.0.join("dip-parity.mp4");
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
        video_render::decode_check(&engine(), &output, || false, |_| {}).unwrap();
        assert_eq!(info.frame_count, preview.info.frame_count);
        assert_eq!(info.frame_count, Some(15));
        assert_eq!(info.audio.len(), preview.info.audio.len());
        // Both output paths normalize frames identically before the sequence concat.
        for time in ["0.100", "0.200", "0.400"] {
            let from_preview = pixel(Path::new(&preview.source), time);
            let from_export = pixel(&output, time);
            for channel in 0..3 {
                let difference =
                    (i32::from(from_preview[channel]) - i32::from(from_export[channel])).abs();
                assert!(
                    difference <= 20,
                    "preview/export pixel mismatch at {time}, channel {channel}: {from_preview:?} vs {from_export:?}"
                );
            }
        }
        let at_cut = pixel(&output, "0.200");
        let after_cut = pixel(&output, "0.400");
        assert!(at_cut.iter().all(|channel| *channel < 85), "{at_cut:?}");
        assert!(after_cut[2] > 90, "{after_cut:?}");
        // Video-only transitions must not shift the source-linked audio timeline.
        assert!(rms(&output, "0.04") > 0.02);
        assert!(rms(&output, "0.30") < 0.002);
        snapshot.revalidate(|| false).unwrap();
        compiled.close().unwrap();
    }

    #[test]
    fn timed_titles_are_rendered_by_the_bundled_windows_video_engine() {
        let (fixture, mut snapshot) = fixture();
        snapshot
            .project
            .titles
            .push(crate::video_sequence::TitleOverlay {
                id: "test-title".into(),
                text: "Open Photo".into(),
                start_seconds: 0.05,
                end_seconds: 0.24,
                x_percent: 50,
                y_percent: 50,
            });
        let preview = build_preview(&engine(), &snapshot, &fixture.0, || false).unwrap();
        assert!(Path::new(&preview.source).exists());
        assert_eq!(preview.info.frame_count, Some(15));
    }

    #[test]
    fn photos_join_videos_and_still_only_output_uses_the_same_validated_frame_clock() {
        let (fixture, mut snapshot) = fixture();
        let path = fixture.0.join("yellow still 世界.png");
        image::RgbaImage::from_pixel(96, 64, image::Rgba([255, 255, 0, 255]))
            .save(&path)
            .unwrap();
        let asset = crate::sequence_media::new_photo(&fixture.0, &path).unwrap();
        let original = video_project::fingerprint(&path, || false).unwrap();
        let mut clip = crate::sequence_media::default_clip(&asset).unwrap();
        clip.source_out = VideoTime::from_seconds(0.2, "1/1000000").unwrap();
        clip.edits.flip_horizontal = true;
        clip.edits.brightness = -5;
        snapshot
            .roots
            .insert(asset.id.clone(), fixture.0.to_string_lossy().into_owned());
        snapshot.project.assets.push(asset.clone());
        snapshot.project.clips.insert(1, clip.clone());
        let preview = build_preview(&engine(), &snapshot, &fixture.0, || false).unwrap();
        assert_eq!(preview.info.frame_count, Some(21));
        assert_eq!(preview.info.audio.len(), 1);
        let yellow = pixel(Path::new(&preview.source), "0.3");
        assert!(
            yellow[0] > 180 && yellow[1] > 180 && yellow[2] < 30,
            "{yellow:?}"
        );
        assert!(rms(Path::new(&preview.source), "0.25") < 0.002);
        let output = fixture.0.join("mixed.mp4");
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
        assert_eq!(info.frame_count, Some(21));
        video_render::decode_check(&engine(), &output, || false, |_| {}).unwrap();
        compiled.close().unwrap();
        assert_eq!(pixel(&output, "0.3"), yellow);
        let mixed_project = snapshot.project.clone();
        snapshot.project.clips = vec![clip];
        let single_output = fixture.0.join("still-only.mp4");
        let mut still = compile(
            &engine(),
            &snapshot,
            &single_output,
            RenderOptions {
                preview: false,
                size: ExportSize::Project,
                acknowledge_first_audio: false,
            },
            || false,
            |_| {},
        )
        .unwrap();
        video_render::encode(&engine(), &still.plan, || false, |_| {}).unwrap();
        let info = video_render::validate_output(&engine(), &still.plan, &single_output, || false)
            .unwrap();
        assert_eq!(info.frame_count, Some(6));
        assert!(info.audio.is_empty());
        video_render::decode_check(&engine(), &single_output, || false, |_| {}).unwrap();
        still.close().unwrap();
        snapshot.revalidate(|| false).unwrap();
        assert_eq!(
            video_project::fingerprint(&path, || false).unwrap(),
            original
        );
        if let Some(folder) = std::env::var_os("OPENPHOTO_PHOTO_ARTIFACT_DIR") {
            let folder = PathBuf::from(folder);
            fs::create_dir_all(&folder).unwrap();
            for (name, bytes) in [
                (
                    "project.json".to_string(),
                    serde_json::to_vec_pretty(&mixed_project).unwrap(),
                ),
                (
                    "sequence-preview.mp4".into(),
                    fs::read(&preview.source).unwrap(),
                ),
            ] {
                let mut staged = crate::safe_output::StagedOutput::new(&folder.join(name)).unwrap();
                staged.file().write_all(&bytes).unwrap();
                staged.commit().unwrap();
            }
            for asset in mixed_project.assets {
                let mut staged =
                    crate::safe_output::StagedOutput::new(&folder.join(&asset.relative_path))
                        .unwrap();
                staged
                    .file()
                    .write_all(&fs::read(fixture.0.join(&asset.relative_path)).unwrap())
                    .unwrap();
                staged.commit().unwrap();
            }
        }
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
        let base = snapshot.project.assets[0].time_base();
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
        snapshot.project.assets[0] = p.asset.into();
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

    #[test]
    fn existing_normalized_parts_feed_crossfade_with_missing_source_audio() {
        // One source has audible AAC; the second has no audio stream. Reuse the
        // production clip-part preprocessor, but keep graph execution test-only.
        let (fixture, snapshot) = fixture();
        let owned_directory = fs::canonicalize(&fixture.0).unwrap();
        let frames = frame_allocation(&snapshot.project).unwrap();
        assert_eq!(frames, vec![6, 9]);
        let rate = snapshot.project.frame_rate;
        let render_options = RenderOptions {
            preview: false,
            size: ExportSize::Project,
            acknowledge_first_audio: false,
        };
        let mut parts = Vec::new();
        let mut receipts = Vec::new();
        for (index, count) in frames.iter().enumerate() {
            let (root, project) = snapshot.single(index).unwrap();
            // A receipt minted after normalization cannot by itself prove
            // that the encoder read the expected saved original. Keep a
            // verified Windows handle pinned throughout the FFmpeg pass.
            let clip = &snapshot.project.clips[index];
            let asset = snapshot
                .project
                .assets
                .iter()
                .find(|asset| asset.id == clip.asset_id)
                .unwrap();
            let source = crate::sequence_media::source(
                snapshot.roots.get(&asset.id).unwrap(),
                asset,
                || false,
            )
            .unwrap();
            let _original_pin = crate::crossfade_windows_guard::PinnedMedia::acquire(
                &[(&source, asset.bytes, asset.sha256.as_str())],
                || false,
            )
            .unwrap();
            let prepared =
                video_render::prepare(&engine(), &root, &project, render_options, || false)
                    .unwrap();
            let part = PartSource::video(prepared);
            assert_eq!(part.audio_stream.is_some(), index == 0);
            let output = owned_directory.join(format!("crossfade-part-{index}.mkv"));
            let args = part_args(
                &part,
                &snapshot.project.clips[index].edits,
                PartOptions {
                    frames: *count,
                    fps: rate,
                    audio: true,
                    bitrate: 200_000,
                    output: &output,
                    max_bytes: 64 * 1024 * 1024,
                    titles_filter: String::new(),
                    segment: None,
                },
            );
            video_engine::run(
                &video_engine::tool(&engine(), "ffmpeg").unwrap(),
                &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                Duration::from_secs(90),
            )
            .unwrap();
            let normalized = video_probe::probe(&engine(), &output, || false).unwrap();
            assert_eq!((normalized.width, normalized.height), (96, 64));
            assert_eq!(normalized.audio.len(), 1);
            assert_eq!(normalized.audio[0].sample_rate.as_deref(), Some("48000"));
            // Mint the receipt directly inside the trusted normalization
            // fixture, after FFmpeg succeeds and the output is probed.
            // No externally supplied post-hoc receipt is accepted here.
            crate::sequence_media::source(snapshot.roots.get(&asset.id).unwrap(), asset, || false)
                .unwrap();
            let (_, part_sha256) = video_project::fingerprint(&output, || false).unwrap();
            receipts.push(crate::crossfade_provenance::NormalizedReceipt {
                clip_id: clip.id.clone(),
                asset_id: asset.id.clone(),
                saved_source_sha256: asset.sha256.clone(),
                part: output.clone(),
                part_sha256,
                frames: *count,
            });
            parts.push(output);
        }
        crate::crossfade_provenance::verify_ordered_receipts(
            &snapshot,
            &receipts,
            &parts,
            &frames,
            || false,
        )
        .unwrap();
        let mut reversed = parts.clone();
        reversed.reverse();
        assert!(crate::crossfade_provenance::verify_ordered_receipts(
            &snapshot,
            &receipts,
            &reversed,
            &frames,
            || false,
        )
        .is_err());
        // Even replacing exactly one byte of a valid normalized MKV must
        // invalidate its trusted, already-minted provenance receipt.
        let mut normalized_bytes = fs::read(&parts[0]).unwrap();
        let last = normalized_bytes.len() - 1;
        normalized_bytes[last] ^= 1;
        fs::write(&parts[0], &normalized_bytes).unwrap();
        assert!(crate::crossfade_provenance::verify_ordered_receipts(
            &snapshot,
            &receipts,
            &parts,
            &frames,
            || false,
        )
        .is_err());
        normalized_bytes[last] ^= 1;
        fs::write(&parts[0], &normalized_bytes).unwrap();
        crate::crossfade_provenance::verify_ordered_receipts(
            &snapshot,
            &receipts,
            &parts,
            &frames,
            || false,
        )
        .unwrap();

        // Confirm the real bundled FFmpeg can read every input while pinned
        // Windows handles deny ordinary writer/delete access.
        let manifest =
            crate::crossfade_snapshot::SourceManifest::from_snapshot(&snapshot, || false).unwrap();
        let sources = manifest.expectations();
        let original_expected: Vec<_> = sources
            .iter()
            .map(|source| (source.path, source.bytes, source.sha256))
            .collect();
        let original_pins =
            crate::crossfade_windows_guard::PinnedMedia::acquire(&original_expected, || false)
                .unwrap();
        let normalized_expected = receipts
            .iter()
            .map(|receipt| {
                (
                    receipt.part.as_path(),
                    fs::metadata(&receipt.part).unwrap().len(),
                    receipt.part_sha256.as_str(),
                )
            })
            .collect::<Vec<_>>();
        let normalized_pins =
            crate::crossfade_windows_guard::PinnedMedia::acquire(&normalized_expected, || false)
                .unwrap();
        assert_eq!(normalized_pins.count(), 2);
        assert!(fs::OpenOptions::new().write(true).open(&parts[0]).is_err());
        assert!(fs::remove_file(&parts[0]).is_err());

        let clips = [
            crate::crossfade_clock::ClipClock {
                id: "a",
                frames: frames[0],
            },
            crate::crossfade_clock::ClipClock {
                id: "b",
                frames: frames[1],
            },
        ];
        let edges = [crate::crossfade_clock::Boundary {
            from_clip_id: "a",
            to_clip_id: "b",
            frames: 3,
        }];
        let clock = crate::crossfade_clock::compile(&clips, &edges).unwrap();
        let graph = crate::crossfade_graph::plan(&clock, rate).unwrap();
        assert_eq!(graph.expected_frames, 12);
        let output = fixture.0.join("crossfade-normalized.mp4");
        let args: Vec<OsString> = vec![
            "-nostdin".into(),
            "-v".into(),
            "error".into(),
            "-i".into(),
            parts[0].clone().into(),
            "-i".into(),
            parts[1].clone().into(),
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
        video_engine::run(
            &video_engine::tool(&engine(), "ffmpeg").unwrap(),
            &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
            Duration::from_secs(90),
        )
        .unwrap();
        // FFmpeg completed with both guard groups alive; release the handles
        // before checking that normal write access is restored.
        drop(normalized_pins);
        drop(original_pins);
        let opened = fs::OpenOptions::new().write(true).open(&parts[0]).unwrap();
        drop(opened);
        let result = video_probe::probe(&engine(), &output, || false).unwrap();
        assert_eq!(result.frame_count, Some(12));
        assert_eq!((result.width, result.height), (96, 64));
        assert_eq!(result.audio.len(), 1);
        assert_eq!(result.audio[0].sample_rate.as_deref(), Some("48000"));
        assert!((result.duration_seconds - 0.4).abs() < 0.05);
        // Full Phase I acceptance: saved originals, trusted normalizer-minted
        // receipts, pinned Windows handles, FFmpeg render, decoded audio/frame
        // checks and SHA-256-verified atomic publication in one native test.
        let saved_clips = [
            crate::crossfade_clock::ClipClock {
                id: &snapshot.project.clips[0].id,
                frames: frames[0],
            },
            crate::crossfade_clock::ClipClock {
                id: &snapshot.project.clips[1].id,
                frames: frames[1],
            },
        ];
        let saved_boundaries = [crate::crossfade_clock::Boundary {
            from_clip_id: &snapshot.project.clips[0].id,
            to_clip_id: &snapshot.project.clips[1].id,
            frames: 3,
        }];
        let verified_plan = crate::crossfade_preflight::plan_allocated(
            &saved_clips,
            &saved_boundaries,
            &[true, false],
            rate,
        )
        .unwrap();
        let scratch = owned_directory.join("phase-i-native-scratch.mp4");
        let published = owned_directory.join("phase-i-verified-output.mp4");
        let verified_engine = engine();
        let request = crate::crossfade_command::CrossfadeExecution {
            engine: &verified_engine,
            plan: &verified_plan,
            inputs: &parts,
            output: &scratch,
            fps: rate,
            max_bytes: 64 * 1024 * 1024,
        };
        crate::crossfade_pipeline::render_verify_publish_snapshot_receipts(
            request,
            &published,
            &snapshot,
            &receipts,
            || false,
        )
        .unwrap();
        assert!(!scratch.exists());
        assert!(published.exists());
        let verified = video_probe::probe(&engine(), &published, || false).unwrap();
        assert_eq!(verified.frame_count, Some(12));
        assert_eq!(verified.audio.len(), 1);
        let repeated = crate::crossfade_pipeline::render_verify_publish_snapshot_receipts(
            crate::crossfade_command::CrossfadeExecution {
                engine: &verified_engine,
                plan: &verified_plan,
                inputs: &parts,
                output: &scratch,
                fps: rate,
                max_bytes: 64 * 1024 * 1024,
            },
            &published,
            &snapshot,
            &receipts,
            || false,
        );
        assert!(repeated.is_err());
        assert!(!scratch.exists());
        snapshot.revalidate(|| false).unwrap();
    }
}

fn prepare_sequence_part(
    engine: &Path,
    snapshot: &Snapshot,
    index: usize,
    frame_count: u64,
    options: RenderOptions,
    cancel: &impl Fn() -> bool,
) -> Result<PartSource, String> {
    let fps = snapshot.project.frame_rate;
    let clip = &snapshot.project.clips[index];
    let asset = snapshot
        .project
        .assets
        .iter()
        .find(|a| a.id == clip.asset_id)
        .ok_or("Missing source")?;
    let root = snapshot.roots.get(&asset.id).ok_or("Relink this source")?;
    let prepared = if asset.is_photo() {
        let source = crate::sequence_media::source(root, asset, cancel)?;
        let output = crate::video_presets::resolve(
            &snapshot.project.output,
            if options.preview {
                ExportSize::Project
            } else {
                options.size
            },
        );
        let (filter, width, height) =
            video_project::edit_filters(&clip.edits, &output, options.preview)?;
        PartSource {
            source,
            photo: true,
            video_stream: 0,
            audio_stream: None,
            start: 0.0,
            duration: frame_count as f64 / f64::from(fps),
            filter,
            width,
            height,
        }
    } else {
        let (root, p) = snapshot.single(index)?;
        let prepared = video_render::prepare(engine, &root, &p, options, cancel)?;
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
        PartSource::video(prepared)
    };
    Ok(prepared)
}

fn prepare_sequence_photo(
    prepared: &mut PartSource,
    snapshot: &Snapshot,
    index: usize,
    directory: &Path,
) -> Result<Option<PathBuf>, String> {
    Ok(if prepared.photo {
        let image = crate::sequence_media::decode_photo(&prepared.source)?;
        let asset = snapshot
            .project
            .assets
            .iter()
            .find(|a| a.id == snapshot.project.clips[index].asset_id)
            .ok_or("Missing source")?;
        if asset.media
            != (crate::sequence_media::Media::Photo {
                width: image.width(),
                height: image.height(),
            })
        {
            return Err("Photo geometry differs from its verified project metadata".into());
        }
        let normalized = directory.join(format!("photo-{index:04}.png"));
        crate::sequence_media::flatten_photo(image)
            .save_with_format(&normalized, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        prepared.source = normalized.clone();
        Some(normalized)
    } else {
        None
    })
}

fn sequence_title_filter(
    snapshot: &Snapshot,
    directory: &Path,
    prefix: &str,
    global_start: u64,
    frames: u64,
    title_spans: &[crate::crossfade_layer_clock::Span],
) -> Result<String, String> {
    let mut titles_filter = String::new();
    for (title_index, title) in snapshot.project.titles.iter().enumerate() {
        let span = &title_spans[title_index];
        let start = span.start.max(global_start).saturating_sub(global_start);
        let end = span
            .end
            .min(global_start + frames)
            .saturating_sub(global_start);
        if end <= start {
            continue;
        }
        // Text is carried in a private local text file, never interpolated into FFmpeg syntax.
        let text_path = directory.join(format!("{prefix}-{title_index:03}.txt"));
        fs::write(&text_path, &title.text).map_err(|e| e.to_string())?;
        let filename = text_path
            .file_name()
            .ok_or("Title file name unavailable")?
            .to_string_lossy();
        // Relative paths avoid Windows drive-colon escaping in FFmpeg's filter parser.
        let x = f64::from(title.x_percent) / 100.0;
        let y = f64::from(title.y_percent) / 100.0;
        titles_filter.push_str(&format!(
                    ",drawtext=fontfile='C\\:/Windows/Fonts/arial.ttf':textfile={filename}:expansion=none:fontcolor=white:fontsize=h/18:x=(w-text_w)*{x:.3}:y=(h-text_h)*{y:.3}:borderw=2:bordercolor=black:enable='gte(n,{start})*lt(n,{end})'"
                ));
    }
    Ok(titles_filter)
}
