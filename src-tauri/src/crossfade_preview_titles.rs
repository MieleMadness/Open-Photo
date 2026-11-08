// Render global titles once, after visual transitions, using the native title style.
pub(crate) fn overlay_preview_titles(
    engine: &Path,
    frame: &mut PreviewRgbPart,
    titles: &[crate::video_sequence::TitleOverlay],
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    if titles.is_empty() {
        return Ok(());
    }
    if titles.len() > 32
        || frame.width == 0
        || frame.height == 0
        || frame.width > 960
        || frame.height > 540
        || frame.rgb.len() as u64 != u64::from(frame.width) * u64::from(frame.height) * 3
    {
        return Err("Invalid title preview bounds".into());
    }
    let mut scratch = Scratch::new(&std::env::temp_dir())?;
    let input = scratch.path().join("frame.rgb");
    fs::write(&input, &frame.rgb).map_err(|e| e.to_string())?;
    let mut paths = vec![input];
    let mut filter = String::from("format=rgb24");
    for (index, title) in titles.iter().enumerate() {
        let name = format!("title-{index}.txt");
        let path = scratch.path().join(&name);
        fs::write(&path, &title.text).map_err(|e| e.to_string())?;
        paths.push(path);
        filter.push_str(&format!(",drawtext=fontfile='C\\:/Windows/Fonts/arial.ttf':textfile={name}:expansion=none:fontcolor=white:fontsize=h/18:x=(w-text_w)*{:.3}:y=(h-text_h)*{:.3}:borderw=2:bordercolor=black",f64::from(title.x_percent)/100.0,f64::from(title.y_percent)/100.0));
    }
    let hashes = paths
        .iter()
        .map(|p| video_project::fingerprint(p, &cancel))
        .collect::<Result<Vec<_>, _>>()?;
    let bindings = paths
        .iter()
        .zip(&hashes)
        .map(|(p, (size, hash))| (p.as_path(), *size, hash.as_str()))
        .collect::<Vec<_>>();
    let pins = crate::crossfade_windows_guard::PinnedMedia::acquire(&bindings, &cancel)?;
    let args: Vec<OsString> = vec![
        "-v".into(),
        "error".into(),
        "-threads".into(),
        "1".into(),
        "-filter_threads".into(),
        "1".into(),
        "-f".into(),
        "rawvideo".into(),
        "-pixel_format".into(),
        "rgb24".into(),
        "-video_size".into(),
        format!("{}x{}", frame.width, frame.height).into(),
        "-i".into(),
        "frame.rgb".into(),
        "-vf".into(),
        filter.into(),
        "-frames:v".into(),
        "1".into(),
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        "rgb24".into(),
        "pipe:1".into(),
    ];
    let rgb = video_engine::run_preview_in(
        &video_engine::tool(engine, "ffmpeg")?,
        &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
        scratch.path(),
        &cancel,
    )?;
    if rgb.len() != frame.rgb.len() || cancel() {
        return Err("Incomplete or cancelled title frame".into());
    }
    frame.rgb = rgb;
    drop(pins);
    scratch.close()?;
    Ok(())
}
