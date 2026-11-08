// Shared final mux: concatenated encoded video and one global audio mix.
struct SequenceMux<'a> {
    snapshot: &'a Snapshot,
    directory: &'a Path,
    output: &'a Path,
    total_frames: u64,
    width: u32,
    height: u32,
    info: Option<video_probe::VideoInfo>,
    audio: bool,
    max_bytes: u64,
    crossfade: bool,
    #[cfg(windows)]
    verified_sources: Option<&'a BTreeMap<String, PathBuf>>,
}
fn mux_plan(
    engine: &Path,
    input: SequenceMux<'_>,
    cancel: impl Fn() -> bool,
) -> Result<Plan, String> {
    let SequenceMux {
        snapshot,
        directory,
        output,
        total_frames,
        width,
        height,
        info,
        audio,
        max_bytes,
        crossfade,
        #[cfg(windows)]
        verified_sources,
    } = input;
    let fps = snapshot.project.frame_rate;
    let duration = total_frames as f64 / f64::from(fps);
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
    let mut extra_audio = Vec::new();
    for track in &snapshot.project.audio_tracks {
        let (source, stream_index, format) = if let Some(asset) = snapshot
            .project
            .assets
            .iter()
            .find(|asset| asset.id == track.asset_id)
        {
            let root = snapshot
                .roots
                .get(&asset.id)
                .ok_or("Relink every audio track source before rendering")?;
            let source = crate::sequence_media::source(root, asset, &cancel)?;
            let info = video_probe::probe(engine, &source, &cancel)?;
            let stream = info
                .audio
                .first()
                .ok_or("Audio source contains no audio stream")?;
            if track.source_in_seconds + track.duration_seconds > info.duration_seconds + 0.000001 {
                return Err("Audio trim exceeds source duration".into());
            }
            (source, stream.stream, "mov,matroska,webm,avi")
        } else {
            let asset = snapshot
                .project
                .audio_sources
                .iter()
                .find(|asset| asset.id == track.asset_id)
                .ok_or("Audio track source missing")?;
            let root = snapshot
                .roots
                .get(&asset.id)
                .ok_or("Relink independent audio source before rendering")?;
            let source = video_probe::safe_audio_path(
                root,
                &Path::new(root).join(&asset.relative_path).to_string_lossy(),
            )?;
            let (bytes, hash) = video_project::fingerprint(&source, &cancel)?;
            if bytes != asset.bytes || hash != asset.sha256 {
                return Err("Independent audio source differs from the saved original".into());
            }
            let info = video_probe::probe_audio_only(engine, &source, &cancel)?;
            if info != asset.info
                || track.source_in_seconds + track.duration_seconds
                    > info.duration_seconds + 0.000001
            {
                return Err("Independent audio metadata or trim has changed".into());
            }
            (source, info.stream, "mp3,wav,flac,mov")
        };
        let input_index = extra_audio.len() + 1;
        #[cfg(windows)]
        if let Some(paths) = verified_sources {
            if paths.get(&track.asset_id) != Some(&source) {
                return Err("Pinned audio source path changed".into());
            }
        }
        args.extend([
            "-format_whitelist".into(),
            format.into(),
            "-i".into(),
            source.into(),
        ]);
        extra_audio.push((input_index, stream_index, track));
    }
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
    if !extra_audio.is_empty() {
        let mut filters = vec![String::from(
            "[0:a:0]aformat=sample_fmts=flt:channel_layouts=stereo,asetpts=N/SR/TB[original]",
        )];
        let mut inputs = String::from("[original]");
        for (index, (input, stream, track)) in extra_audio.iter().enumerate() {
            let label = format!("track{index}");
            let track_plan = crate::crossfade_layer_clock::audio_plan(
                &snapshot.project,
                track,
                total_frames * 48000 / u64::from(fps),
            )?;
            filters.push(added_audio_filter(*input, *stream, &track_plan, &label)?);
            inputs.push_str(&format!("[{label}]"));
        }
        filters.push(format!(
                "{inputs}amix=inputs={}:duration=longest:normalize=0,atrim=end_sample={},{FINAL_PCM_CLIP},asetpts=N/SR/TB[mixed]",
                extra_audio.len() + 1,
                total_frames * 48000 / u64::from(fps)
            ));
        args.extend([
            "-filter_complex".into(),
            filters.join(";").into(),
            "-map".into(),
            "[mixed]".into(),
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            "192k".into(),
            "-ar".into(),
            "48000".into(),
            "-ac".into(),
            "2".into(),
        ]);
    } else if audio {
        args.extend([
            "-map".into(),
            "0:a:0".into(),
            "-af".into(),
            format!(
                "atrim=end_sample={},{FINAL_PCM_CLIP},asetpts=N/SR/TB",
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
    let warnings=vec![format!("{} use a shared {fps} fps SDR Rec.709 canvas. Source trims remain exact in the project; output positions use its frame clock.",if crossfade {"Sample-centered RGB crossfades"} else {"Hard cuts"}),"Video segments use one encode; continuous audio is encoded once as AAC stereo. Silent or muted intervals are filled with silence when the project has audio. Source metadata, subtitles and chapters are excluded.".into()];
    Ok(Plan {
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
        working_directory: Some(directory.to_owned()),
        duration,
        max_bytes,
        timeout: Duration::from_secs(21600),
        args,
        warnings,
    })
}
