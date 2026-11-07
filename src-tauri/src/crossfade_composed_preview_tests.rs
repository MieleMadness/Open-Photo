#[test]
fn composed_real_frames_follow_clock_weights_and_preserve_source_ownership() {
    let (mut f, s) = decode_fixture();
    let q = PreviewRequests::new();
    let original = fs::read(f.root.join("decoded-source.mp4")).unwrap();
    for frame in [0, 24, 25, 26, 27, 28, 29, 30, 54] {
        preview_submit(&q, &mut f, &s, frame);
        let visual = q
            .compose_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
            .unwrap()
            .unwrap();
        let weights = &visual.inputs.request.position.video;
        for channel in 0..3 {
            let expected = visual
                .inputs
                .parts
                .iter()
                .zip(weights)
                .map(|(part, seek)| {
                    f64::from(pixel(&part.frame, 48, 32)[channel]) * seek.weight.numerator as f64
                        / seek.weight.denominator as f64
                })
                .sum::<f64>()
                .round() as u8;
            assert_eq!(pixel(&visual.frame, 48, 32)[channel], expected);
        }
        assert!(q
            .visual_current(&mut f.db, &s, &s.receipt, &visual)
            .unwrap());
        assert!(
            fs::OpenOptions::new()
                .write(true)
                .open(f.root.join("decoded-source.mp4"))
                .is_err()
                || frame == 54
        );
    }
    assert_eq!(
        fs::read(f.root.join("decoded-source.mp4")).unwrap(),
        original
    );
}
#[test]
fn composition_supersession_keeps_worker_owned_and_rejects_final_stale_delivery() {
    let (mut f, s) = decode_fixture();
    let q = PreviewRequests::new();
    let mut other = Connection::open(f.root.join("prototype.db")).unwrap();
    preview_submit(&q, &mut f, &s, 24);
    assert!(q
        .compose_next(&mut f.db, &s, &s.receipt, &decode_engine(), |stage| {
            if stage == "after_visual_composition" {
                q.submit(&mut other, &s, &s.receipt, SeekPoint::Frame(25))?;
                assert!(q.start_next(&mut other, &s, &s.receipt)?.is_none());
            }
            Ok(())
        })
        .unwrap()
        .is_none());
    let visual = q
        .compose_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .unwrap()
        .unwrap();
    preview_submit(&q, &mut f, &s, 26);
    assert!(!q
        .visual_current(&mut f.db, &s, &s.receipt, &visual)
        .unwrap());
}

#[test]
fn real_dip_frames_follow_existing_discrete_fade_boundaries() {
    let (mut f, _) = decode_fixture();
    f.project.transitions.push(CutTransition {
        from_clip_id: f.project.clips[1].id.clone(),
        to_clip_id: f.project.clips[2].id.clone(),
        kind: CutTransitionKind::DipToBlack,
        duration_seconds: 0.4,
    });
    let s = open_decode_session(&mut f, 2);
    let q = PreviewRequests::new();
    for (frame, gain) in [
        (48, 1.0),
        (51, 0.5),
        (53, 1.0 / 6.0),
        (54, 0.0),
        (57, 0.5),
        (60, 1.0),
    ] {
        preview_submit(&q, &mut f, &s, frame);
        let visual = q
            .compose_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
            .unwrap()
            .unwrap();
        let raw = pixel(&visual.inputs.parts[0].frame, 80, 32);
        let actual = pixel(&visual.frame, 80, 32);
        for channel in 0..3 {
            assert_eq!(
                actual[channel],
                (f64::from(raw[channel]) * gain).round() as u8,
                "frame {frame}"
            );
        }
    }
}

#[test]
fn composed_frame_matches_bundled_ffmpeg_rgb_reference() {
    let (mut f, s) = decode_fixture();
    let q = PreviewRequests::new();
    preview_submit(&q, &mut f, &s, 26);
    let visual = q
        .compose_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .unwrap()
        .unwrap();
    let a = f.root.join("reference-a.rgb");
    let b = f.root.join("reference-b.rgb");
    fs::write(&a, &visual.inputs.parts[0].frame.rgb).unwrap();
    fs::write(&b, &visual.inputs.parts[1].frame.rgb).unwrap();
    let w = &visual.inputs.request.position.video[1].weight;
    let weight = w.numerator as f64 / w.denominator as f64;
    let filter = format!(
        "[0:v]format=gbrp[a];[1:v]format=gbrp[b];[a][b]blend=all_expr='A*{}+B*{}',format=rgb24",
        1.0 - weight,
        weight
    );
    let args: Vec<std::ffi::OsString> = [
        "-v",
        "error",
        "-filter_complex_threads",
        "1",
        "-f",
        "rawvideo",
        "-pixel_format",
        "rgb24",
        "-video_size",
        "96x64",
        "-i",
    ]
    .into_iter()
    .map(Into::into)
    .chain([
        a.into(),
        "-f".into(),
        "rawvideo".into(),
        "-pixel_format".into(),
        "rgb24".into(),
        "-video_size".into(),
        "96x64".into(),
        "-i".into(),
        b.into(),
        "-filter_complex".into(),
        filter.into(),
        "-frames:v".into(),
        "1".into(),
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        "rgb24".into(),
        "pipe:1".into(),
    ])
    .collect();
    let reference = video_engine::run(
        &video_engine::tool(&decode_engine(), "ffmpeg").unwrap(),
        &args.iter().map(|a| a.as_os_str()).collect::<Vec<_>>(),
        std::time::Duration::from_secs(30),
    )
    .unwrap();
    assert_eq!(reference.len(), visual.frame.rgb.len());
    assert!(reference
        .iter()
        .zip(&visual.frame.rgb)
        .all(|(a, b)| a.abs_diff(*b) <= 1));
}
