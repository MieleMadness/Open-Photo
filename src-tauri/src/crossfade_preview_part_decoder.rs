// Included only under cfg(all(test, windows)) in the shared sequence renderer.
// Produces one normalized RGB input frame, not a composed/display-ready preview.
pub(crate) struct PreviewRgbPart {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}
pub(crate) fn decode_preview_part(
    engine: &Path,
    project: &Sequence,
    root: &str,
    index: usize,
    frame: u64,
    cancel: impl Fn() -> bool,
) -> Result<PreviewRgbPart, String> {
    if cancel() {
        return Err("Preview part decode cancelled".into());
    }
    let clip = project.clips.get(index).ok_or("Missing preview clip")?;
    let asset = project
        .assets
        .iter()
        .find(|a| a.id == clip.asset_id)
        .ok_or("Missing preview asset")?;
    let frames = *frame_allocation(project)?
        .get(index)
        .ok_or("Missing preview frame allocation")?;
    if frame >= frames || frames > u64::from(project.frame_rate) * 10 {
        return Err(
            "Gated preview decoder requires an in-range frame in a clip of at most ten seconds"
                .into(),
        );
    }
    let mut scratch = Scratch::new(&std::env::temp_dir())?;
    let mut derivative_pin = None;
    let prepared = if asset.is_photo() {
        let source = crate::sequence_media::source(root, asset, &cancel)?;
        let image = crate::sequence_media::decode_photo(&source)?;
        if asset.media
            != (crate::sequence_media::Media::Photo {
                width: image.width(),
                height: image.height(),
            })
        {
            return Err("Photo dimensions differ from saved metadata".into());
        }
        let flattened = crate::sequence_media::flatten_photo(image);
        let mut encoded = std::io::Cursor::new(Vec::new());
        flattened
            .write_to(&mut encoded, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        let bytes = encoded.into_inner();
        let path = scratch.path().join("still.png");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        drop(file);
        let hash = {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(&bytes))
        };
        derivative_pin = Some(crate::crossfade_windows_guard::PinnedMedia::acquire(
            &[(&path, bytes.len() as u64, &hash)],
            &cancel,
        )?);
        let (filter, width, height) =
            video_project::edit_filters(&clip.edits, &project.output, true)?;
        PartSource {
            source: path,
            photo: true,
            video_stream: 0,
            audio_stream: None,
            start: 0.0,
            duration: frames as f64 / f64::from(project.frame_rate),
            filter,
            width,
            height,
        }
    } else {
        let single = crate::video_sequence::single(project, asset, clip.clone())?;
        let prepared = video_render::prepare(
            engine,
            root,
            &single,
            RenderOptions {
                preview: true,
                size: ExportSize::Project,
                acknowledge_first_audio: false,
            },
            &cancel,
        )?;
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
            return Err("Preview decoder requires SDR Rec.709 source color".into());
        }
        PartSource::video(prepared)
    };
    if prepared.width > 960 || prepared.height > 540 {
        return Err("Preview canvas exceeds bounded decoder dimensions".into());
    }
    let mut args: Vec<OsString> = [
        "-nostdin",
        "-v",
        "error",
        "-xerror",
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
            project.frame_rate.to_string().into(),
        ]);
    } else {
        args.extend([
            "-format_whitelist".into(),
            "mov,matroska,webm,avi".into(),
            "-ss".into(),
            format!("{:.9}", prepared.start).into(),
        ]);
    }
    let filter = format!(
        "{},select=eq(n\\,{frame}),format=rgb24",
        normalized_video_filter(&prepared, frames, project.frame_rate, "")
    );
    args.extend([
        "-i".into(),
        prepared.source.clone().into(),
        "-map".into(),
        format!("0:{}", prepared.video_stream).into(),
        "-an".into(),
        "-sn".into(),
        "-dn".into(),
        "-vf".into(),
        filter.into(),
        "-frames:v".into(),
        "1".into(),
        "-fps_mode".into(),
        "vfr".into(),
        "-c:v".into(),
        "rawvideo".into(),
        "-threads:v".into(),
        "1".into(),
        "-pix_fmt".into(),
        "rgb24".into(),
        "-f".into(),
        "rawvideo".into(),
        "pipe:1".into(),
    ]);
    if cancel() {
        return Err("Preview part decode cancelled".into());
    }
    let rgb = video_engine::run_until(
        &video_engine::tool(engine, "ffmpeg")?,
        &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
        Duration::from_secs(30),
        &cancel,
    )?;
    let expected = u64::from(prepared.width) * u64::from(prepared.height) * 3;
    if rgb.len() as u64 != expected || cancel() {
        return Err("Preview decoder returned an incomplete frame or was cancelled".into());
    }
    drop(derivative_pin);
    scratch.close()?;
    Ok(PreviewRgbPart {
        width: prepared.width,
        height: prepared.height,
        rgb,
    })
}
