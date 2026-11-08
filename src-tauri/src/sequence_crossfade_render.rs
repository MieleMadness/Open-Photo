// Linear-work compiler: encoded bodies plus lossless RGB overlap handles.
// Only two source handles enter a blend; accumulated video is never re-encoded.
struct VerifiedSequenceSources {
    paths: BTreeMap<String, PathBuf>,
    pins: std::sync::Arc<crate::crossfade_windows_guard::PinnedMedia>,
}
impl VerifiedSequenceSources {
    fn verify_prepared(
        &self,
        snapshot: &Snapshot,
        index: usize,
        prepared: &PartSource,
    ) -> Result<(), String> {
        if self.paths.get(&snapshot.project.clips[index].asset_id) != Some(&prepared.source) {
            return Err("Pinned timeline source path changed".into());
        }
        Ok(())
    }
}
fn pin_sequence_sources(
    snapshot: &Snapshot,
    cancel: &impl Fn() -> bool,
) -> Result<VerifiedSequenceSources, String> {
    let mut paths = BTreeMap::new();
    let mut files = BTreeMap::new();
    for asset in &snapshot.project.assets {
        if !snapshot
            .project
            .clips
            .iter()
            .any(|c| c.asset_id == asset.id)
            && !snapshot
                .project
                .audio_tracks
                .iter()
                .any(|t| t.asset_id == asset.id)
        {
            continue;
        }
        let root = snapshot
            .roots
            .get(&asset.id)
            .ok_or("Missing source binding")?;
        let path = crate::sequence_media::source(root, asset, cancel)?;
        if let Some(old) = files.insert(path.clone(), (asset.bytes, asset.sha256.clone())) {
            if old != (asset.bytes, asset.sha256.clone()) {
                return Err("Conflicting pinned source identities".into());
            }
        }
        paths.insert(asset.id.clone(), path);
    }
    for asset in &snapshot.project.audio_sources {
        if !snapshot
            .project
            .audio_tracks
            .iter()
            .any(|t| t.asset_id == asset.id)
        {
            continue;
        }
        let root = snapshot
            .roots
            .get(&asset.id)
            .ok_or("Missing audio binding")?;
        let path = video_probe::safe_audio_path(
            root,
            &Path::new(root).join(&asset.relative_path).to_string_lossy(),
        )?;
        if let Some(old) = files.insert(path.clone(), (asset.bytes, asset.sha256.clone())) {
            if old != (asset.bytes, asset.sha256.clone()) {
                return Err("Conflicting pinned audio identities".into());
            }
        }
        paths.insert(asset.id.clone(), path);
    }
    let expected = files
        .iter()
        .map(|(path, (bytes, hash))| (path.as_path(), *bytes, hash.as_str()))
        .collect::<Vec<_>>();
    let pins = crate::crossfade_windows_guard::PinnedMedia::acquire(&expected, cancel)?;
    if pins.count() != files.len() {
        return Err("Incomplete source ownership".into());
    }
    Ok(VerifiedSequenceSources {
        paths,
        pins: std::sync::Arc::new(pins),
    })
}
fn pin_encoded_piece(
    path: &Path,
    cancel: &impl Fn() -> bool,
) -> Result<crate::crossfade_windows_guard::PinnedMedia, String> {
    let (bytes, hash) = video_project::fingerprint(path, cancel)?;
    crate::crossfade_windows_guard::PinnedMedia::acquire(&[(path, bytes, &hash)], cancel)
}
fn pin_caption_files(
    snapshot: &Snapshot,
    directory: &Path,
    prefix: &str,
    start: u64,
    frames: u64,
    spans: &[crate::crossfade_layer_clock::Span],
    cancel: &impl Fn() -> bool,
) -> Result<Option<crate::crossfade_windows_guard::PinnedMedia>, String> {
    use sha2::{Digest, Sha256};
    let files = snapshot
        .project
        .titles
        .iter()
        .enumerate()
        .filter(|(index, _)| spans[*index].start < start + frames && spans[*index].end > start)
        .map(|(index, title)| {
            (
                directory.join(format!("{prefix}-{index:03}.txt")),
                title.text.len() as u64,
                format!("{:x}", Sha256::digest(title.text.as_bytes())),
            )
        })
        .collect::<Vec<_>>();
    if files.is_empty() {
        return Ok(None);
    }
    let expected = files
        .iter()
        .map(|(path, bytes, hash)| (path.as_path(), *bytes, hash.as_str()))
        .collect::<Vec<_>>();
    Ok(Some(crate::crossfade_windows_guard::PinnedMedia::acquire(
        &expected, cancel,
    )?))
}
fn verify_encoded_piece(
    engine: &Path,
    path: &Path,
    expected_frames: u64,
    rgb: bool,
    audio: bool,
    canvas: (u32, u32),
    cancel: &impl Fn() -> bool,
) -> Result<video_probe::VideoInfo, String> {
    let info = video_probe::probe(engine, path, cancel)?;
    if (info.width, info.height) != canvas
        || info.codec != if rgb { "ffv1" } else { "h264" }
        || info.audio.len() != usize::from(audio)
        || info.audio.first().is_some_and(|a| {
            a.codec != "pcm_f32le"
                || a.channels != Some(2)
                || a.sample_rate.as_deref() != Some("48000")
        })
    {
        return Err("Crossfade segment failed stream validation".into());
    }
    let raw = video_engine::run_until(
        &video_engine::tool(engine, "ffprobe")?,
        &[
            "-v".as_ref(),
            "error".as_ref(),
            "-select_streams".as_ref(),
            "v:0".as_ref(),
            "-count_frames".as_ref(),
            "-show_entries".as_ref(),
            "stream=nb_read_frames".as_ref(),
            "-of".as_ref(),
            "default=nokey=1:noprint_wrappers=1".as_ref(),
            path.as_os_str(),
        ],
        Duration::from_secs(21600),
        cancel,
    )?;
    if std::str::from_utf8(&raw)
        .map_err(|e| e.to_string())?
        .trim()
        .parse::<u64>()
        .ok()
        != Some(expected_frames)
    {
        return Err("Crossfade segment frame count changed or exceeded its byte budget".into());
    }
    Ok(info)
}
fn encode_crossfade_piece(
    engine: &Path,
    prepared: &PartSource,
    edits: &video_project::Edits,
    options: PartOptions<'_>,
    directory: &Path,
    cancel: &impl Fn() -> bool,
    progress: impl Fn(video_engine::Progress) + Send + 'static,
) -> Result<
    (
        video_probe::VideoInfo,
        crate::crossfade_windows_guard::PinnedMedia,
    ),
    String,
> {
    let segment = options.segment.ok_or("Missing crossfade segment")?;
    if segment.start >= segment.end || segment.end > options.frames {
        return Err("Invalid crossfade segment range".into());
    }
    let (path, audio) = (options.output.to_owned(), options.audio);
    let args = part_args(prepared, edits, options);
    video_engine::run_progress_in(
        &video_engine::tool(engine, "ffmpeg")?,
        &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
        Duration::from_secs(21600),
        cancel,
        progress,
        Some(directory),
    )?;
    let pin = pin_encoded_piece(&path, cancel)?;
    let info = verify_encoded_piece(
        engine,
        &path,
        segment.end - segment.start,
        segment.rgb,
        audio,
        (prepared.width, prepared.height),
        cancel,
    )?;
    Ok((info, pin))
}
struct BlendPiece<'a> {
    a: &'a Path,
    b: &'a Path,
    output: &'a Path,
    directory: &'a Path,
    frames: u64,
    fps: u32,
    audio: bool,
    bitrate: u64,
    max_bytes: u64,
    titles: String,
    canvas: (u32, u32),
}
fn encode_crossfade_blend(
    engine: &Path,
    piece: BlendPiece<'_>,
    cancel: &impl Fn() -> bool,
    progress: impl Fn(video_engine::Progress) + Send + 'static,
) -> Result<
    (
        video_probe::VideoInfo,
        crate::crossfade_windows_guard::PinnedMedia,
    ),
    String,
> {
    let BlendPiece {
        a,
        b,
        output,
        directory,
        frames,
        fps,
        audio,
        bitrate,
        max_bytes,
        titles,
        canvas,
    } = piece;
    // blend's N starts at one in the bundled engine. Tests compare every
    // overlap position to the shared rational sample-center kernel.
    let w = format!("(N-0.5)/{frames}");
    let mut graph=format!("[0:v:0]setpts=N/({fps}*TB),format=gbrp[a];[1:v:0]setpts=N/({fps}*TB),format=gbrp[b];[a][b]blend=all_expr='A*(1-({w}))+B*({w})',format=rgb24{titles},scale=out_color_matrix=bt709:out_range=tv,format=yuv420p,setsar=1[v]");
    if audio {
        let samples = frames * 48000 / u64::from(fps);
        graph.push_str(&format!(";[0:a:0]aeval='val(0)*(1-(n+0.5)/{samples})|val(1)*(1-(n+0.5)/{samples})'[aa];[1:a:0]aeval='val(0)*(n+0.5)/{samples}|val(1)*(n+0.5)/{samples}'[ab];[aa][ab]amix=inputs=2:duration=shortest:normalize=0,atrim=end_sample={samples},asetpts=N/SR/TB[pcm]"));
    }
    let mut args: Vec<OsString> = [
        "-nostdin",
        "-v",
        "error",
        "-xerror",
        "-y",
        "-threads",
        "2",
        "-filter_complex_threads",
        "1",
        "-max_alloc",
        "268435456",
        "-protocol_whitelist",
        "file,pipe",
        "-format_whitelist",
        "matroska",
        "-i",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    args.extend([
        a.into(),
        "-format_whitelist".into(),
        "matroska".into(),
        "-i".into(),
        b.into(),
        "-filter_complex".into(),
        graph.into(),
        "-map".into(),
        "[v]".into(),
        "-frames:v".into(),
        frames.to_string().into(),
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
    if audio {
        args.extend([
            "-map".into(),
            "[pcm]".into(),
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
        format!("{:.9}", frames as f64 / f64::from(fps)).into(),
    ]);
    progress_args(&mut args, output, "matroska", max_bytes);
    video_engine::run_progress_in(
        &video_engine::tool(engine, "ffmpeg")?,
        &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
        Duration::from_secs(21600),
        cancel,
        progress,
        Some(directory),
    )?;
    let pin = pin_encoded_piece(output, cancel)?;
    Ok((
        verify_encoded_piece(engine, output, frames, false, audio, canvas, cancel)?,
        pin,
    ))
}
fn compile_crossfade(
    engine: &Path,
    snapshot: &Snapshot,
    output: &Path,
    options: RenderOptions,
    cancel: impl Fn() -> bool,
    progress: impl Fn(f64) + Send + Sync + 'static,
) -> Result<Compiled, String> {
    let binding = snapshot
        .transition
        .as_ref()
        .ok_or("Missing transition authorization")?;
    let boundaries = binding
        .edges
        .iter()
        .map(|e| crate::crossfade_clock::Boundary {
            from_clip_id: &e.from_clip_id,
            to_clip_id: &e.to_clip_id,
            frames: e.frames,
        })
        .collect::<Vec<_>>();
    let timing = crate::crossfade_layer_clock::plan(&snapshot.project, &boundaries)?;
    snapshot.revalidate(&cancel)?;
    let sources = pin_sequence_sources(snapshot, &cancel)?;
    let scratch = Scratch::new(output.parent().ok_or("Output folder unavailable")?)?;
    let directory = scratch.path().to_owned();
    let allocations = frame_allocation(&snapshot.project)?;
    let mut contexts = allocations
        .iter()
        .enumerate()
        .map(|(index, frames)| {
            let prepared =
                prepare_sequence_part(engine, snapshot, index, *frames, options, &cancel)?;
            sources.verify_prepared(snapshot, index, &prepared)?;
            Ok(prepared)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let canvas = (contexts[0].width, contexts[0].height);
    if contexts.iter().any(|p| (p.width, p.height) != canvas) {
        return Err("Crossfade canvases disagree".into());
    }
    let fps = timing.fps;
    let duration = timing.total_frames as f64 / f64::from(fps);
    let audio = !snapshot.project.audio_tracks.is_empty()
        || contexts
            .iter()
            .enumerate()
            .any(|(i, p)| p.audio_stream.is_some() && !snapshot.project.clips[i].edits.muted);
    let bitrate = if options.preview {
        2_000_000
    } else {
        snapshot.bitrate(options.size)?
    };
    let max_bytes = if options.preview {
        512 * 1024 * 1024
    } else {
        video_render::MAX_EXPORT_BYTES
    };
    let max_overlap = binding.edges.iter().map(|e| e.frames).max().unwrap_or(0);
    let required = (duration
        * (2.0 * bitrate as f64 / 8.0 * 1.25 + if audio { 384000.0 } else { 0.0 })
        + max_overlap as f64 * f64::from(canvas.0) * f64::from(canvas.1) * 16.0
        + max_overlap as f64 / f64::from(fps) * if audio { 768000.0 } else { 0.0 })
    .ceil() as u64
        + 256 * 1024 * 1024;
    if crate::video_export::available_space(&directory)?.is_some_and(|free| free < required) {
        return Err(format!(
            "Not enough crossfade working space (allow {} MB)",
            required.div_ceil(1024 * 1024)
        ));
    }
    let title_spans = snapshot
        .project
        .titles
        .iter()
        .map(|t| crate::crossfade_layer_clock::title_span(t, fps, timing.total_frames))
        .collect::<Result<Vec<_>, String>>()?;
    let progress: std::sync::Arc<dyn Fn(f64) + Send + Sync> = std::sync::Arc::new(progress);
    let total_work =
        allocations.iter().sum::<u64>() + binding.edges.iter().map(|e| e.frames).sum::<u64>();
    let mut work = 0u64;
    let callback = |base: u64, count: u64| {
        let notify = progress.clone();
        move |p: video_engine::Progress| {
            notify(
                ((base as f64 + (p.seconds * f64::from(fps)).clamp(0.0, count as f64))
                    / total_work as f64
                    * 0.75)
                    .clamp(0.0, 0.75),
            )
        }
    };
    let mut manifest = String::from("ffconcat version 1.0\n");
    let mut segment_pins = vec![];
    let mut retained = 0u64;
    let mut info = None;
    let mut outgoing: Option<(PathBuf, crate::crossfade_windows_guard::PinnedMedia)> = None;
    for (index, prepared) in contexts.iter_mut().enumerate() {
        if cancel() {
            return Err("Crossfade rendering cancelled".into());
        }
        let clip = &timing.clips[index];
        let frames = allocations[index];
        let incoming = index
            .checked_sub(1)
            .and_then(|i| {
                binding
                    .edges
                    .iter()
                    .find(|e| e.from_clip_id == snapshot.project.clips[i].id)
            })
            .map_or(0, |e| e.frames);
        let next = binding
            .edges
            .iter()
            .find(|e| e.from_clip_id == clip.id)
            .map_or(0, |e| e.frames);
        let photo = prepare_sequence_photo(prepared, snapshot, index, &directory)?;
        let photo_pin = photo
            .as_ref()
            .map(|path| pin_encoded_piece(path, &cancel))
            .transpose()?;
        let mut dips = String::new();
        if clip.dip_in_frames > 0 {
            dips.push_str(&format!(
                ",fade=t=in:start_frame=0:nb_frames={}",
                clip.dip_in_frames
            ));
        }
        if clip.dip_out_frames > 0 {
            dips.push_str(&format!(
                ",fade=t=out:start_frame={}:nb_frames={}",
                frames - clip.dip_out_frames,
                clip.dip_out_frames
            ));
        }
        if incoming > 0 {
            let path = directory.join(format!("incoming-{index:04}.mkv"));
            let (_, pin) = encode_crossfade_piece(
                engine,
                prepared,
                &snapshot.project.clips[index].edits,
                PartOptions {
                    frames,
                    fps,
                    audio,
                    bitrate,
                    output: &path,
                    max_bytes: video_render::MAX_EXPORT_BYTES,
                    titles_filter: dips.clone(),
                    segment: Some(PartSegment {
                        start: 0,
                        end: incoming,
                        rgb: true,
                    }),
                },
                &directory,
                &cancel,
                callback(work, incoming),
            )?;
            work += incoming;
            let (previous, previous_pin) =
                outgoing.take().ok_or("Missing preceding overlap handle")?;
            let name = format!("blend-{index:04}.mkv");
            let blend = directory.join(&name);
            let titles = sequence_title_filter(
                snapshot,
                &directory,
                &format!("blend-title-{index:04}"),
                clip.frames.start,
                incoming,
                &title_spans,
            )?;
            let caption_pins = pin_caption_files(
                snapshot,
                &directory,
                &format!("blend-title-{index:04}"),
                clip.frames.start,
                incoming,
                &title_spans,
                &cancel,
            )?;
            let (_, guard) = encode_crossfade_blend(
                engine,
                BlendPiece {
                    a: &previous,
                    b: &path,
                    output: &blend,
                    directory: &directory,
                    frames: incoming,
                    fps,
                    audio,
                    bitrate,
                    max_bytes,
                    titles,
                    canvas,
                },
                &cancel,
                callback(work, incoming),
            )?;
            drop(caption_pins);
            work += incoming;
            segment_pins.push(guard);
            retained += fs::metadata(&blend).map_err(|e| e.to_string())?.len();
            manifest.push_str(&format!(
                "file {name}\nduration {:.9}\n",
                incoming as f64 / f64::from(fps)
            ));
            drop(pin);
            drop(previous_pin);
            fs::remove_file(&path).map_err(|e| e.to_string())?;
            fs::remove_file(&previous).map_err(|e| e.to_string())?;
        } else if outgoing.is_some() {
            return Err("Unexpected overlap handle".into());
        }
        let body_count = frames - incoming - next;
        let name = format!("body-{index:04}.mkv");
        let path = directory.join(&name);
        let mut filters = dips.clone();
        filters.push_str(&sequence_title_filter(
            snapshot,
            &directory,
            &format!("body-title-{index:04}"),
            clip.frames.start,
            frames,
            &title_spans,
        )?);
        let caption_pins = pin_caption_files(
            snapshot,
            &directory,
            &format!("body-title-{index:04}"),
            clip.frames.start,
            frames,
            &title_spans,
            &cancel,
        )?;
        let (result, pin) = encode_crossfade_piece(
            engine,
            prepared,
            &snapshot.project.clips[index].edits,
            PartOptions {
                frames,
                fps,
                audio,
                bitrate,
                output: &path,
                max_bytes,
                titles_filter: filters,
                segment: Some(PartSegment {
                    start: incoming,
                    end: frames - next,
                    rgb: false,
                }),
            },
            &directory,
            &cancel,
            callback(work, body_count),
        )?;
        drop(caption_pins);
        work += body_count;
        info = Some(result);
        segment_pins.push(pin);
        retained += fs::metadata(&path).map_err(|e| e.to_string())?.len();
        manifest.push_str(&format!(
            "file {name}\nduration {:.9}\n",
            body_count as f64 / f64::from(fps)
        ));
        if retained >= max_bytes + (duration * 384000.0) as u64 {
            return Err("Crossfade segments exceed the output working budget".into());
        }
        if next > 0 {
            let path = directory.join(format!("outgoing-{index:04}.mkv"));
            let (_, pin) = encode_crossfade_piece(
                engine,
                prepared,
                &snapshot.project.clips[index].edits,
                PartOptions {
                    frames,
                    fps,
                    audio,
                    bitrate,
                    output: &path,
                    max_bytes: video_render::MAX_EXPORT_BYTES,
                    titles_filter: dips,
                    segment: Some(PartSegment {
                        start: frames - next,
                        end: frames,
                        rgb: true,
                    }),
                },
                &directory,
                &cancel,
                callback(work, next),
            )?;
            work += next;
            outgoing = Some((path, pin));
        }
        drop(photo_pin);
        if let Some(path) = photo {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }
    if outgoing.is_some() || work != total_work {
        return Err("Crossfade work clock disagrees with its manifest".into());
    }
    fs::write(directory.join("sequence.ffconcat"), manifest).map_err(|e| e.to_string())?;
    snapshot.revalidate(&cancel)?;
    Ok(Compiled {
        plan: mux_plan(
            engine,
            SequenceMux {
                snapshot,
                directory: &directory,
                output,
                total_frames: timing.total_frames,
                width: canvas.0,
                height: canvas.1,
                info,
                audio,
                max_bytes,
                crossfade: true,
                verified_sources: Some(&sources.paths),
            },
            &cancel,
        )?,
        encoded_fraction: 0.75,
        authorization: snapshot.transition.clone(),
        segment_pins,
        scratch: Some(scratch),
        source_pins: Some(sources.pins),
    })
}
