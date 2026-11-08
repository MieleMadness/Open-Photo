fn phase3_encode(snapshot: &Snapshot, directory: &Path, name: &str, preview: bool) -> PathBuf {
    let output = directory.join(name);
    let mut compiled = compile(
        &engine(),
        snapshot,
        &output,
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
    video_render::validate_output(&engine(), &compiled.plan, &output, || false).unwrap();
    compiled.close().unwrap();
    output
}
fn phase3_frame(path: &Path, frame: u64) -> Vec<u8> {
    let filter = format!("select=eq(n\\,{frame})");
    video_engine::run(
        &video_engine::tool(&engine(), "ffmpeg").unwrap(),
        &[
            "-v".as_ref(),
            "error".as_ref(),
            "-i".as_ref(),
            path.as_os_str(),
            "-vf".as_ref(),
            filter.as_ref(),
            "-frames:v".as_ref(),
            "1".as_ref(),
            "-pix_fmt".as_ref(),
            "rgb24".as_ref(),
            "-f".as_ref(),
            "rawvideo".as_ref(),
            "pipe:1".as_ref(),
        ],
        Duration::from_secs(20),
    )
    .unwrap()
}
#[test]
fn phase3_segmented_crossfade_matches_sample_centered_rgb_preview_at_every_overlap_frame() {
    let (fixture, mut snapshot) = fixture();
    snapshot.transition = Some(crate::video_workspace::transitions::RenderBinding::fixture(
        vec![crate::video_workspace::transitions::Edge {
            from_clip_id: snapshot.project.clips[0].id.clone(),
            to_clip_id: snapshot.project.clips[1].id.clone(),
            frames: 3,
        }],
    ));
    for (name, preview) in [
        ("crossfade-preview.mp4", true),
        ("crossfade-export.mp4", false),
    ] {
        let output = phase3_encode(&snapshot, &fixture.0, name, preview);
        let info = video_probe::probe(&engine(), &output, || false).unwrap();
        assert_eq!(info.frame_count, Some(12));
        assert_eq!(info.audio.len(), 1);
        for frame in 3..6 {
            let a = decode_preview_part(
                &engine(),
                &snapshot.project,
                snapshot
                    .roots
                    .get(&snapshot.project.clips[0].asset_id)
                    .unwrap(),
                0,
                frame,
                || false,
            )
            .unwrap();
            let b = decode_preview_part(
                &engine(),
                &snapshot.project,
                snapshot
                    .roots
                    .get(&snapshot.project.clips[1].asset_id)
                    .unwrap(),
                1,
                frame - 3,
                || false,
            )
            .unwrap();
            let expected = crate::crossfade_visual_compositor::blend_rgb(
                &a.rgb,
                &b.rgb,
                a.width,
                a.height,
                (2 * (frame - 3) + 1) as f64 / 6.0,
            )
            .unwrap();
            let actual = phase3_frame(&output, frame);
            assert_eq!(actual.len(), expected.len());
            let offset = (32 * 96 + 48) * 3;
            for channel in 0..3 {
                assert!(
                    (i16::from(actual[offset + channel]) - i16::from(expected[offset + channel]))
                        .abs()
                        <= 8,
                    "{name} frame {frame}, channel {channel}: {} versus {}",
                    actual[offset + channel],
                    expected[offset + channel]
                );
            }
        }
    }
}
#[test]
fn phase3_consecutive_sixty_fps_video_photo_overlaps_keep_clock_alpha_and_global_titles() {
    let (fixture, mut snapshot) = fixture();
    snapshot.project.frame_rate = 60;
    snapshot.project.output.width = 384;
    snapshot.project.output.height = 256;
    let photo = fixture.0.join("crossfade-alpha.png");
    image::RgbaImage::from_fn(96, 64, |x, _| {
        image::Rgba([0, 255, 0, if x < 48 { 0 } else { 255 }])
    })
    .save(&photo)
    .unwrap();
    let asset = crate::sequence_media::new_photo(&fixture.0, &photo).unwrap();
    let mut clip = crate::sequence_media::default_clip(&asset).unwrap();
    clip.source_out = crate::video_project::VideoTime::from_seconds(0.2, "1/1000000").unwrap();
    snapshot
        .roots
        .insert(asset.id.clone(), fixture.0.to_string_lossy().into_owned());
    snapshot.project.assets.push(asset);
    snapshot.project.clips.push(clip);
    snapshot
        .project
        .titles
        .push(crate::video_sequence::TitleOverlay {
            id: "global-title".into(),
            text: "MMMM".into(),
            start_seconds: 10.0 / 60.0,
            end_seconds: 27.0 / 60.0,
            x_percent: 50,
            y_percent: 50,
        });
    snapshot.transition = Some(crate::video_workspace::transitions::RenderBinding::fixture(
        vec![
            crate::video_workspace::transitions::Edge {
                from_clip_id: snapshot.project.clips[0].id.clone(),
                to_clip_id: snapshot.project.clips[1].id.clone(),
                frames: 5,
            },
            crate::video_workspace::transitions::Edge {
                from_clip_id: snapshot.project.clips[1].id.clone(),
                to_clip_id: snapshot.project.clips[2].id.clone(),
                frames: 4,
            },
        ],
    ));
    let output = phase3_encode(&snapshot, &fixture.0, "sixty-mixed-crossfade.mp4", false);
    assert_eq!(
        video_probe::probe(&engine(), &output, || false)
            .unwrap()
            .frame_count,
        Some(33)
    );
    for (frame, visible) in [
        (9, false),
        (10, true),
        (11, true),
        (21, true),
        (24, true),
        (26, true),
        (27, false),
    ] {
        let raw = phase3_frame(&output, frame);
        assert_eq!(raw.len(), 384 * 256 * 3);
        let white = raw
            .as_chunks::<3>()
            .0
            .iter()
            .filter(|p| p.iter().all(|v| *v > 180))
            .count();
        assert_eq!(white > 20, visible, "frame {frame}, {white} white pixels");
    }
    let a = decode_preview_part(
        &engine(),
        &snapshot.project,
        snapshot
            .roots
            .get(&snapshot.project.clips[1].asset_id)
            .unwrap(),
        1,
        15,
        || false,
    )
    .unwrap();
    let b = decode_preview_part(
        &engine(),
        &snapshot.project,
        snapshot
            .roots
            .get(&snapshot.project.clips[2].asset_id)
            .unwrap(),
        2,
        1,
        || false,
    )
    .unwrap();
    let expected =
        crate::crossfade_visual_compositor::blend_rgb(&a.rgb, &b.rgb, a.width, a.height, 3.0 / 8.0)
            .unwrap();
    let actual = phase3_frame(&output, 22);
    for x in [96usize, 288] {
        let offset = (128 * 384 + x) * 3;
        for ch in 0..3 {
            assert!((i16::from(expected[offset + ch]) - i16::from(actual[offset + ch])).abs() <= 8);
        }
    }
}
#[test]
fn phase3_transition_pcm_has_exact_sample_centered_complements_without_premature_clipping() {
    let (fixture, _) = fixture();
    let frames = 4;
    let fps = 30;
    let samples = 6400usize;
    let mut inputs = vec![];
    let mut pins = vec![];
    for (index, color, level) in [(0, "red", 1.5), (1, "blue", 0.5)] {
        let path = fixture.0.join(format!("pcm-handle-{index}.mkv"));
        let color = format!("color=c={color}:s=96x64:r=30:d=0.133333333");
        let audio = format!("aevalsrc={level}|-{level}:s=48000:d=0.133333333");
        let args: Vec<OsString> = [
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            &color,
            "-f",
            "lavfi",
            "-i",
            &audio,
            "-map",
            "0:v:0",
            "-map",
            "1:a:0",
            "-c:v",
            "ffv1",
            "-pix_fmt",
            "bgr0",
            "-c:a",
            "pcm_f32le",
            "-t",
            "0.133333333",
        ]
        .into_iter()
        .map(Into::into)
        .chain([path.clone().into()])
        .collect();
        video_engine::run(
            &video_engine::tool(&engine(), "ffmpeg").unwrap(),
            &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
            Duration::from_secs(20),
        )
        .unwrap();
        pins.push(pin_encoded_piece(&path, &|| false).unwrap());
        inputs.push(path);
    }
    let output = fixture.0.join("pcm-blend.mkv");
    let (_, guard) = encode_crossfade_blend(
        &engine(),
        BlendPiece {
            a: &inputs[0],
            b: &inputs[1],
            output: &output,
            directory: &fixture.0,
            frames,
            fps,
            audio: true,
            bitrate: 2_000_000,
            max_bytes: 16 * 1024 * 1024,
            titles: String::new(),
            canvas: (96, 64),
        },
        &|| false,
        |_| {},
    )
    .unwrap();
    let raw = video_engine::run(
        &video_engine::tool(&engine(), "ffmpeg").unwrap(),
        &[
            "-v".as_ref(),
            "error".as_ref(),
            "-i".as_ref(),
            output.as_os_str(),
            "-map".as_ref(),
            "0:a:0".as_ref(),
            "-f".as_ref(),
            "f32le".as_ref(),
            "pipe:1".as_ref(),
        ],
        Duration::from_secs(20),
    )
    .unwrap();
    assert_eq!(raw.len(), samples * 8);
    for (index, value) in raw.as_chunks::<4>().0.iter().enumerate() {
        let expected = (1.5 - (index / 2) as f64 / 6400.0 - 0.5 / 6400.0)
            * if index % 2 == 0 { 1.0 } else { -1.0 };
        assert!(
            (f64::from(f32::from_le_bytes(*value)) - expected).abs() < 1e-6,
            "sample {}, channel {}",
            index / 2,
            index % 2
        );
    }
    drop(guard);
    drop(pins);
}
#[test]
fn phase3_segment_guards_survive_cleanup_until_publication_and_cancelled_work_leaves_no_output() {
    let (fixture, mut snapshot) = fixture();
    snapshot.transition = Some(crate::video_workspace::transitions::RenderBinding::fixture(
        vec![crate::video_workspace::transitions::Edge {
            from_clip_id: snapshot.project.clips[0].id.clone(),
            to_clip_id: snapshot.project.clips[1].id.clone(),
            frames: 3,
        }],
    ));
    let output = fixture.0.join("pinned-crossfade.mp4");
    let mut compiled = compile(
        &engine(),
        &snapshot,
        &output,
        RenderOptions {
            preview: true,
            size: ExportSize::Project,
            acknowledge_first_audio: false,
        },
        || false,
        |_| {},
    )
    .unwrap();
    let original = fixture.0.join(&snapshot.project.assets[0].relative_path);
    let part = compiled
        .scratch
        .as_ref()
        .unwrap()
        .path()
        .join("body-0000.mkv");
    assert!(fs::OpenOptions::new().write(true).open(&original).is_err());
    assert!(fs::OpenOptions::new().write(true).open(&part).is_err());
    compiled.close().unwrap();
    assert!(!part.exists());
    assert!(fs::OpenOptions::new().write(true).open(&original).is_err());
    drop(compiled);
    assert!(fs::OpenOptions::new().write(true).open(&original).is_ok());
    let stopped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let signal = stopped.clone();
    assert!(compile(
        &engine(),
        &snapshot,
        &output,
        RenderOptions {
            preview: true,
            size: ExportSize::Project,
            acknowledge_first_audio: false
        },
        || stopped.load(std::sync::atomic::Ordering::SeqCst),
        move |fraction| {
            if fraction > 0.0 {
                signal.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
    )
    .is_err());
    assert!(!output.exists());
    assert!(fs::OpenOptions::new().write(true).open(&original).is_ok());
    assert!(fs::read_dir(&fixture.0).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".openphoto-parts-")));
}
#[test]
fn phase3_single_video_added_audio_is_present_in_actual_preview_and_export() {
    let (fixture, mut snapshot) = fixture();
    snapshot.project.clips.truncate(1);
    snapshot.project.clips[0].edits.muted = true;
    snapshot
        .project
        .audio_tracks
        .push(crate::video_sequence::AudioTrack {
            id: "added".into(),
            asset_id: snapshot.project.assets[0].id.clone(),
            start_seconds: 0.0,
            source_in_seconds: 0.0,
            duration_seconds: 0.15,
            gain_percent: 100,
            fade_in_seconds: 0.0,
            fade_out_seconds: 0.0,
        });
    for (name, preview) in [("preview-added.mp4", true), ("export-added.mp4", false)] {
        let output = phase3_encode(&snapshot, &fixture.0, name, preview);
        let info = video_probe::probe(&engine(), &output, || false).unwrap();
        assert_eq!(info.frame_count, Some(6));
        assert_eq!(info.audio.len(), 1);
        assert!(rms(&output, "0.04") > 0.02);
    }
    snapshot.revalidate(|| false).unwrap();
}
#[test]
fn phase3_titles_cross_cuts_once_use_exclusive_frame_ends_and_survive_video_dips() {
    let (fixture, mut snapshot) = fixture();
    snapshot.project.output.width = 384;
    snapshot.project.output.height = 256;
    snapshot
        .project
        .titles
        .push(crate::video_sequence::TitleOverlay {
            id: "boundary-title".into(),
            text: "MMMM".into(),
            start_seconds: 5.0 / 30.0,
            end_seconds: 8.0 / 30.0,
            x_percent: 50,
            y_percent: 50,
        });
    snapshot
        .project
        .transitions
        .push(crate::video_sequence::CutTransition {
            from_clip_id: snapshot.project.clips[0].id.clone(),
            to_clip_id: snapshot.project.clips[1].id.clone(),
            kind: crate::video_sequence::CutTransitionKind::DipToBlack,
            duration_seconds: 0.08,
        });
    for (name, preview) in [("preview-title.mp4", true), ("export-title.mp4", false)] {
        let output = phase3_encode(&snapshot, &fixture.0, name, preview);
        for (frame, visible) in [(4, false), (5, true), (6, true), (7, true), (8, false)] {
            let raw = phase3_frame(&output, frame);
            assert_eq!(raw.len(), 384 * 256 * 3);
            let white = raw
                .as_chunks::<3>()
                .0
                .iter()
                .filter(|pixel| pixel.iter().all(|v| *v > 180))
                .count();
            assert_eq!(
                white > 20,
                visible,
                "frame {frame}: {white} white pixels ({name})"
            );
        }
    }
}
#[test]
fn phase3_added_audio_uses_exact_sample_delay_trim_and_sample_centered_fades() {
    use crate::crossfade_layer_clock::{AudioPlan, Span};
    let plan = AudioPlan {
        id: "track".into(),
        asset_id: "source".into(),
        samples: Span { start: 5, end: 965 },
        source_samples: Span { start: 7, end: 967 },
        fade_in_samples: 100,
        fade_out_samples: 100,
        gain_percent: 200,
    };
    let filter = added_audio_filter(0, 0, &plan, "track").unwrap();
    let raw = video_engine::run(
        &video_engine::tool(&engine(), "ffmpeg").unwrap(),
        &[
            "-v".as_ref(),
            "error".as_ref(),
            "-f".as_ref(),
            "lavfi".as_ref(),
            "-i".as_ref(),
            "aevalsrc=n/10000|n/20000:s=48000:d=0.1".as_ref(),
            "-filter_complex".as_ref(),
            filter.as_ref(),
            "-map".as_ref(),
            "[track]".as_ref(),
            "-f".as_ref(),
            "f32le".as_ref(),
            "pipe:1".as_ref(),
        ],
        Duration::from_secs(20),
    )
    .unwrap();
    assert_eq!(raw.len(), 965 * 8);
    for (index, bytes) in raw.as_chunks::<4>().0.iter().enumerate() {
        let sample = index / 2;
        let expected = if sample < 5 {
            0.0
        } else {
            let n = (sample - 5) as f64;
            let source = (n + 7.0) / if index % 2 == 0 { 10000.0 } else { 20000.0 };
            source
                * 2.0
                * ((n + 0.5) / 100.0).clamp(0.0, 1.0)
                * ((960.0 - n - 0.5) / 100.0).clamp(0.0, 1.0)
        };
        assert!(
            (f64::from(f32::from_le_bytes(*bytes)) - expected).abs() < 1e-6,
            "sample {sample}, channel {}",
            index % 2
        );
    }
    let clipped = video_engine::run(
        &video_engine::tool(&engine(), "ffmpeg").unwrap(),
        &[
            "-v".as_ref(),
            "error".as_ref(),
            "-f".as_ref(),
            "lavfi".as_ref(),
            "-i".as_ref(),
            "aevalsrc=2|-2:s=48000:d=0.04".as_ref(),
            "-af".as_ref(),
            FINAL_PCM_CLIP.as_ref(),
            "-f".as_ref(),
            "f32le".as_ref(),
            "pipe:1".as_ref(),
        ],
        Duration::from_secs(20),
    )
    .unwrap();
    for (index, bytes) in clipped.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            f32::from_le_bytes(*bytes),
            if index % 2 == 0 { 1.0 } else { -1.0 }
        );
    }
}
#[test]
fn phase3_video_used_only_as_added_audio_still_requires_final_source_verification() {
    let (fixture, mut snapshot) = fixture();
    snapshot.project.clips.remove(0);
    snapshot
        .project
        .audio_tracks
        .push(crate::video_sequence::AudioTrack {
            id: "added".into(),
            asset_id: snapshot.project.assets[0].id.clone(),
            start_seconds: 0.0,
            source_in_seconds: 0.0,
            duration_seconds: 0.15,
            gain_percent: 100,
            fade_in_seconds: 0.0,
            fade_out_seconds: 0.0,
        });
    snapshot.revalidate(|| false).unwrap();
    let source = fixture.0.join(&snapshot.project.assets[0].relative_path);
    fs::write(source, b"changed disposable source").unwrap();
    assert!(snapshot.revalidate(|| false).is_err());
}
