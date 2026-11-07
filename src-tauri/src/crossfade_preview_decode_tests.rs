fn decode_engine() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/video-engine")
}
fn open_decode_session(f: &mut Fixture, revision: u64) -> EditorSession {
    store(&f.db, f.root.to_str().unwrap(), &f.project, Some(revision)).unwrap();
    initialize_layer_store(&f.db).unwrap();
    let from = f.project.clips[0].id.clone();
    let to = f.project.clips[1].id.clone();
    let data = prepare_layer_manifest(
        &mut f.db,
        f.root.to_str().unwrap(),
        &f.project.id,
        &[Boundary {
            from_clip_id: &from,
            to_clip_id: &to,
            frames: 6,
        }],
    )
    .unwrap();
    let id = save_layer_manifest(
        &mut f.db,
        f.root.to_str().unwrap(),
        &f.project.id,
        &data,
        |_| Ok(()),
    )
    .unwrap();
    EditorSession::open(&mut f.db, f.root.to_str().unwrap(), &f.project.id, &id).unwrap()
}
fn decode_fixture() -> (Fixture, EditorSession) {
    let mut f = Fixture::new();
    let path = f.root.join("decoded-source.mp4");
    let args: Vec<std::ffi::OsString> = [
        "-nostdin",
        "-v",
        "error",
        "-n",
        "-f",
        "lavfi",
        "-i",
        "color=c=red:s=96x64:r=30:d=1",
        "-f",
        "lavfi",
        "-i",
        "color=c=blue:s=96x64:r=30:d=1",
        "-filter_complex",
        "[0:v][1:v]concat=n=2:v=1:a=0[v]",
        "-map",
        "[v]",
        "-an",
        "-c:v",
        "libopenh264",
        "-b:v",
        "1000000",
        "-pix_fmt",
        "yuv420p",
    ]
    .into_iter()
    .map(Into::into)
    .chain([path.clone().into()])
    .collect();
    video_engine::run(
        &video_engine::tool(&decode_engine(), "ffmpeg").unwrap(),
        &args.iter().map(|a| a.as_os_str()).collect::<Vec<_>>(),
        std::time::Duration::from_secs(30),
    )
    .unwrap();
    let info = video_probe::probe(&decode_engine(), &path, || false).unwrap();
    let legacy = video_project::new_project(f.root.to_str().unwrap(), &path, info).unwrap();
    let mut project = migrate(legacy).unwrap();
    project.id = f.project.id.clone();
    project.output = video_project::Output {
        width: 96,
        height: 64,
        fit: "fit".into(),
    };
    let base = project.assets[0].time_base().to_owned();
    project.clips[0].source_out = video_project::VideoTime::from_seconds(1.0, &base).unwrap();
    let mut second = project.clips[0].clone();
    second.id = "decoded-b".into();
    second.source_in = video_project::VideoTime::from_seconds(1.0, &base).unwrap();
    second.source_out = video_project::VideoTime::from_seconds(2.0, &base).unwrap();
    project.clips.push(second);
    let still = f.root.join("decoded-still.png");
    image::RgbaImage::from_fn(96, 64, |x, _| {
        image::Rgba([0, 255, 0, if x < 48 { 0 } else { 255 }])
    })
    .save(&still)
    .unwrap();
    let (bytes, sha256) = video_project::fingerprint(&still, || false).unwrap();
    let asset = crate::sequence_media::Asset {
        id: "decoded-still".into(),
        relative_path: "decoded-still.png".into(),
        sha256,
        bytes,
        media: Media::Photo {
            width: 96,
            height: 64,
        },
    };
    let mut photo = crate::sequence_media::default_clip(&asset).unwrap();
    photo.id = "decoded-photo".into();
    photo.source_out = video_project::VideoTime::from_seconds(1.0, "1/1000000").unwrap();
    project.assets.push(asset);
    project.clips.push(photo);
    project.titles.push(TitleOverlay {
        id: "decode-title".into(),
        text: "Not composited yet".into(),
        start_seconds: 0.1,
        end_seconds: 0.9,
        x_percent: 50,
        y_percent: 50,
    });
    f.project = project;
    let s = open_decode_session(&mut f, 1);
    (f, s)
}
fn pixel(part: &crate::video_sequence_render::PreviewRgbPart, x: usize, y: usize) -> [u8; 3] {
    let n = (y * part.width as usize + x) * 3;
    part.rgb[n..n + 3].try_into().unwrap()
}
#[test]
fn real_preview_decode_returns_verified_overlap_parts_and_retains_source_pins() {
    let (mut f, s) = decode_fixture();
    let q = PreviewRequests::new();
    preview_submit(&q, &mut f, &s, 24);
    let source = f.root.join("decoded-source.mp4");
    let expected = video_project::fingerprint(&source, || false).unwrap();
    let raw = f.raw_project();
    let result = q
        .decode_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(result.parts.len(), 2);
    assert_eq!(result.originals.count(), 1); // repeated asset, pinned once
    assert_eq!(
        (result.parts[0].frame.width, result.parts[0].frame.height),
        (96, 64)
    );
    let red = pixel(&result.parts[0].frame, 48, 32);
    let blue = pixel(&result.parts[1].frame, 48, 32);
    assert!(red[0] > 240 && red[1] < 15 && red[2] < 15, "{red:?}");
    assert!(blue[0] < 15 && blue[1] < 15 && blue[2] > 240, "{blue:?}");
    assert_eq!(result.request.position.titles, vec!["decode-title"]);
    assert!(q
        .decoded_current(&mut f.db, &s, &s.receipt, &result)
        .unwrap());
    assert!(fs::OpenOptions::new().write(true).open(&source).is_err());
    assert!(fs::remove_file(&source).is_err());
    assert_eq!(f.raw_project(), raw);
    assert!(q.state.lock().unwrap().active.is_none());
    drop(result);
    assert!(fs::OpenOptions::new().write(true).open(&source).is_ok());
    assert_eq!(
        video_project::fingerprint(&source, || false).unwrap(),
        expected
    );
}
#[test]
fn real_photo_decode_flattens_transparency_and_checks_exact_rgb_payloads() {
    let (mut f, s) = decode_fixture();
    let q = PreviewRequests::new();
    preview_submit(&q, &mut f, &s, 54);
    let mut result = q
        .decode_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(result.parts.len(), 1);
    assert!(result.request.position.video[0].photo);
    let black = pixel(&result.parts[0].frame, 16, 32);
    let green = pixel(&result.parts[0].frame, 80, 32);
    assert!(black.into_iter().all(|v| v < 8));
    assert!(
        green[0] < 15 && green[1] > 240 && green[2] < 15,
        "{green:?}"
    );
    result.parts[0].frame.rgb.pop();
    assert!(q
        .decoded_current(&mut f.db, &s, &s.receipt, &result)
        .is_err());
}
#[test]
fn real_normalized_frame_selection_crosses_a_source_color_change_exactly() {
    let (mut f, _) = decode_fixture();
    let base = f.project.assets[0].time_base().to_owned();
    f.project.clips[0].source_out = video_project::VideoTime::from_seconds(2.0, &base).unwrap();
    let s = open_decode_session(&mut f, 2);
    let q = PreviewRequests::new();
    for (frame, red_expected) in [(29, true), (30, false), (53, false)] {
        preview_submit(&q, &mut f, &s, frame);
        let result = q
            .decode_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
            .unwrap()
            .unwrap();
        let rgb = pixel(&result.parts[0].frame, 48, 32);
        assert_eq!(rgb[0] > 240, red_expected, "frame {frame}: {rgb:?}");
        assert_eq!(rgb[2] > 240, !red_expected, "frame {frame}: {rgb:?}");
    }
}
#[test]
fn decoded_results_are_discarded_after_native_project_changes_during_work() {
    let (mut f, s) = decode_fixture();
    let q = PreviewRequests::new();
    preview_submit(&q, &mut f, &s, 24);
    let other = Connection::open(f.root.join("prototype.db")).unwrap();
    let mut changed: Value = serde_json::from_str(&f.raw_project()).unwrap();
    changed["titles"][0]["text"] = json!("changed during decode");
    assert!(q
        .decode_next(&mut f.db, &s, &s.receipt, &decode_engine(), |stage| {
            if stage == "after_part_decode" {
                other
                    .execute(
                        "UPDATE video_sequences SET data=?1 WHERE id=?2",
                        params![changed.to_string(), s.project_id],
                    )
                    .unwrap();
            }
            Ok(())
        })
        .is_err());
    assert!(q.state.lock().unwrap().active.is_none());
    assert!(fs::OpenOptions::new()
        .write(true)
        .open(f.root.join("decoded-source.mp4"))
        .is_ok());
}
#[test]
fn superseding_a_real_decode_cancels_its_remaining_parts_and_preserves_latest_work() {
    let (mut f, s) = decode_fixture();
    let q = PreviewRequests::new();
    preview_submit(&q, &mut f, &s, 24);
    let mut other = Connection::open(f.root.join("prototype.db")).unwrap();
    let mut decoded = 0;
    let obsolete = q
        .decode_next(&mut f.db, &s, &s.receipt, &decode_engine(), |stage| {
            if stage == "after_part_decode" {
                decoded += 1;
                q.submit(&mut other, &s, &s.receipt, SeekPoint::Frame(25))
                    .unwrap();
                assert!(q.start_next(&mut other, &s, &s.receipt).unwrap().is_none());
            }
            Ok(())
        })
        .unwrap();
    assert!(obsolete.is_none());
    assert_eq!(decoded, 1);
    let latest = q
        .decode_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(latest.request.position.project_frame, 25);
    assert_eq!(latest.parts.len(), 2);
}
#[test]
fn changed_or_hardlinked_originals_are_refused_without_replacing_source_bytes() {
    let (mut f, s) = decode_fixture();
    let q = PreviewRequests::new();
    let source = f.root.join("decoded-source.mp4");
    let raw = fs::read(&source).unwrap();
    let mut changed = raw.clone();
    let last = changed.len() - 1;
    changed[last] ^= 1;
    fs::write(&source, &changed).unwrap();
    preview_submit(&q, &mut f, &s, 24);
    assert!(q
        .decode_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .is_err());
    assert_eq!(fs::read(&source).unwrap(), changed);
    fs::write(&source, &raw).unwrap();
    let alias = f.root.join("source-alias.mp4");
    fs::hard_link(&source, &alias).unwrap();
    preview_submit(&q, &mut f, &s, 24);
    assert!(q
        .decode_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .is_err());
    assert_eq!(fs::read(&source).unwrap(), raw);
    fs::remove_file(alias).unwrap();
}
#[test]
fn decoder_rejects_long_parts_and_false_photo_geometry_before_delivery() {
    let (mut f, _) = decode_fixture();
    f.project.clips[2].source_out =
        video_project::VideoTime::from_seconds(11.0, "1/1000000").unwrap();
    let s = open_decode_session(&mut f, 2);
    let q = PreviewRequests::new();
    preview_submit(&q, &mut f, &s, 54);
    assert!(q
        .decode_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .is_err());
    assert!(q.state.lock().unwrap().active.is_none());
    f.project.clips[2].source_out =
        video_project::VideoTime::from_seconds(1.0, "1/1000000").unwrap();
    f.project.assets[1].media = Media::Photo {
        width: 128,
        height: 64,
    };
    let s = open_decode_session(&mut f, 3);
    preview_submit(&q, &mut f, &s, 54);
    assert!(q
        .decode_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .is_err());
}
