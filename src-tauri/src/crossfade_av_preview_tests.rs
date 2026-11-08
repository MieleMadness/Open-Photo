fn workspace_av_fixture(f: &Fixture) -> (Connection, Connection) {
    let second = f.root.join("second-root");
    fs::create_dir(&second).unwrap();
    let mut bindings = std::collections::BTreeMap::new();
    for asset in &f.project.assets {
        let root = if matches!(asset.media, Media::Photo { .. }) {
            fs::rename(
                f.root.join(&asset.relative_path),
                second.join(&asset.relative_path),
            )
            .unwrap();
            &second
        } else {
            &f.root
        };
        bindings.insert(asset.id.clone(), root.to_string_lossy().into_owned());
    }
    for asset in &f.project.audio_sources {
        fs::rename(
            f.root.join(&asset.relative_path),
            second.join(&asset.relative_path),
        )
        .unwrap();
        bindings.insert(asset.id.clone(), second.to_string_lossy().into_owned());
    }
    let settings = Connection::open_in_memory().unwrap();
    settings
        .execute_batch("CREATE TABLE user_data(key TEXT PRIMARY KEY,value TEXT NOT NULL)")
        .unwrap();
    settings
        .execute(
            "INSERT INTO user_data VALUES('openphoto.workspaces',?1)",
            [json!([{"id":"workspace","root":f.root,"roots":[f.root,second]}]).to_string()],
        )
        .unwrap();
    let projects = Connection::open_in_memory().unwrap();
    projects.execute_batch("CREATE TABLE workspace_video_projects(workspace TEXT,id TEXT,revision INTEGER,data TEXT,bindings TEXT)").unwrap();
    projects
        .execute(
            "INSERT INTO workspace_video_projects VALUES('workspace',?1,1,?2,?3)",
            params![
                f.project.id,
                serde_json::to_string(&f.project).unwrap(),
                serde_json::to_string(&bindings).unwrap()
            ],
        )
        .unwrap();
    (settings, projects)
}

#[test]
fn workspace_av_resolves_multiple_roots_retains_pins_and_rechecks_delivery_access() {
    let (mut f, s) = audio_preview_fixture();
    let (settings, projects) = workspace_av_fixture(&f);
    let q = PreviewRequests::new();
    // The final photo is in the second root, as is the independent WAV source.
    preview_submit(&q, &mut f, &s, 56);
    let result = q
        .workspace_av_next(
            &mut f.db,
            &s,
            &decode_engine(),
            WorkspacePreviewAccess {
                settings: &settings,
                projects: &projects,
                workspace: "workspace",
                revision: 1,
            },
            |_| Ok(()),
        )
        .unwrap()
        .unwrap();
    assert!(result.pcm.iter().any(|sample| *sample > 0.0));
    assert!(q
        .workspace_av_current(&mut f.db, &s, &result, &settings, &projects)
        .unwrap());
    assert!(q.av_current(&mut f.db, &s, &s.receipt, &result).is_err());
    for file in ["constant.wav", "decoded-still.png"] {
        assert!(fs::OpenOptions::new()
            .write(true)
            .open(f.root.join("second-root").join(file))
            .is_err());
    }
    settings
        .execute("UPDATE user_data SET value='[]'", [])
        .unwrap();
    assert!(q
        .workspace_av_current(&mut f.db, &s, &result, &settings, &projects)
        .is_err());
    drop(result);
    assert!(fs::OpenOptions::new()
        .write(true)
        .open(f.root.join("second-root/constant.wav"))
        .is_ok());
}

#[test]
fn workspace_av_rejects_mid_composition_revocation_and_document_mismatch_without_stranding_worker()
{
    let (mut f, s) = audio_preview_fixture();
    let (settings, projects) = workspace_av_fixture(&f);
    let q = PreviewRequests::new();
    preview_submit(&q, &mut f, &s, 24);
    let access = || WorkspacePreviewAccess {
        settings: &settings,
        projects: &projects,
        workspace: "workspace",
        revision: 1,
    };
    let saved: String = settings
        .query_row("SELECT value FROM user_data", [], |r| r.get(0))
        .unwrap();
    assert!(q
        .workspace_av_next(&mut f.db, &s, &decode_engine(), access(), |stage| {
            if stage == "after_av_composition" {
                settings
                    .execute("UPDATE user_data SET value='[]'", [])
                    .unwrap();
            }
            Ok(())
        })
        .is_err());
    assert!(q.state.lock().unwrap().active.is_none());
    settings
        .execute("UPDATE user_data SET value=?1", [saved])
        .unwrap();
    let mut changed = f.project.clone();
    changed.title = "Different native document".into();
    projects
        .execute(
            "UPDATE workspace_video_projects SET data=?1",
            [serde_json::to_string(&changed).unwrap()],
        )
        .unwrap();
    preview_submit(&q, &mut f, &s, 24);
    assert!(q
        .workspace_av_next(&mut f.db, &s, &decode_engine(), access(), |_| Ok(()))
        .is_err());
    assert!(q.state.lock().unwrap().active.is_none());
}

#[test]
fn av_silent_frame_has_exact_stereo_window_and_global_titles_change_pixels() {
    let (mut f, s) = decode_fixture();
    let q = PreviewRequests::new();
    preview_submit(&q, &mut f, &s, 24);
    let plain = q
        .compose_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .unwrap()
        .unwrap();
    let plain_rgb = plain.frame.rgb.clone();
    drop(plain);
    preview_submit(&q, &mut f, &s, 24);
    let av = q
        .av_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(av.pcm.len(), 3200);
    assert!(av.pcm.iter().all(|v| *v == 0.0));
    assert_ne!(av.visual.frame.rgb, plain_rgb);
    assert!(q.av_current(&mut f.db, &s, &s.receipt, &av).unwrap());
    preview_submit(&q, &mut f, &s, 25);
    assert!(!q.av_current(&mut f.db, &s, &s.receipt, &av).unwrap());
}

#[test]
fn av_supersession_after_mixing_discards_result_and_releases_worker() {
    let (mut f, s) = decode_fixture();
    let q = PreviewRequests::new();
    let mut other = Connection::open(f.root.join("prototype.db")).unwrap();
    preview_submit(&q, &mut f, &s, 24);
    assert!(q
        .av_next(&mut f.db, &s, &s.receipt, &decode_engine(), |stage| {
            if stage == "after_av_composition" {
                q.submit(&mut other, &s, &s.receipt, SeekPoint::Frame(25))?;
            }
            Ok(())
        })
        .unwrap()
        .is_none());
    assert!(q.state.lock().unwrap().active.is_none());
}

fn audio_preview_fixture() -> (Fixture, EditorSession) {
    let (mut f, _) = decode_fixture();
    let source = f.root.join("constant.wav");
    let args: Vec<std::ffi::OsString> = [
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "aevalsrc=0.8|0.8:s=48000:d=2",
        "-ac",
        "2",
        "-c:a",
        "pcm_f32le",
    ]
    .into_iter()
    .map(Into::into)
    .chain([source.clone().into()])
    .collect();
    video_engine::run(
        &video_engine::tool(&decode_engine(), "ffmpeg").unwrap(),
        &args.iter().map(|a| a.as_os_str()).collect::<Vec<_>>(),
        std::time::Duration::from_secs(30),
    )
    .unwrap();
    let info = video_probe::probe_audio_only(&decode_engine(), &source, || false).unwrap();
    let (bytes, sha256) = video_project::fingerprint(&source, || false).unwrap();
    f.project.audio_sources.push(AudioSource {
        id: "constant".into(),
        relative_path: "constant.wav".into(),
        bytes,
        sha256,
        info,
    });
    f.project.audio_tracks.push(AudioTrack {
        id: "music".into(),
        asset_id: "constant".into(),
        start_seconds: 0.0,
        source_in_seconds: 0.0,
        duration_seconds: 2.0,
        gain_percent: 200,
        fade_in_seconds: 1.0,
        fade_out_seconds: 0.0,
    });
    let s = open_decode_session(&mut f, 2);
    (f, s)
}
#[test]
fn real_pcm_uses_sample_centered_gain_and_reports_clipping_with_source_pins() {
    let (mut f, s) = audio_preview_fixture();
    let q = PreviewRequests::new();
    for frame in [0, 15, 30] {
        preview_submit(&q, &mut f, &s, frame);
        let result = q
            .av_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
            .unwrap()
            .unwrap();
        for n in [0, 799, 1599] {
            let sample = u64::from(frame as u32) * 1600 + n;
            let gain = if sample < 48000 {
                (sample * 2 + 1) as f64 / 96000.0
            } else {
                1.0
            };
            let expected = (0.8 * 2.0 * gain).min(1.0) as f32;
            assert!(
                (result.pcm[n as usize * 2] - expected).abs() < 0.000001,
                "frame {frame} sample {n}"
            );
        }
        assert_eq!(result.clipped_samples > 0, frame == 30);
        assert!(fs::OpenOptions::new()
            .write(true)
            .open(f.root.join("constant.wav"))
            .is_err());
    }
}

#[test]
fn phase_two_linked_audio_stays_continuous_across_linear_crossfade() {
    let (mut f, _) = decode_fixture();
    let original = f.root.join("decoded-source.mp4");
    let linked = f.root.join("linked.mp4");
    let args: Vec<std::ffi::OsString> = vec![
        "-v".into(),
        "error".into(),
        "-i".into(),
        original.into(),
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        "aevalsrc=0.25|0.25:s=48000:d=2".into(),
        "-map".into(),
        "0:v:0".into(),
        "-map".into(),
        "1:a:0".into(),
        "-c:v".into(),
        "copy".into(),
        "-c:a".into(),
        "aac".into(),
        linked.clone().into(),
    ];
    video_engine::run(
        &video_engine::tool(&decode_engine(), "ffmpeg").unwrap(),
        &args.iter().map(|a| a.as_os_str()).collect::<Vec<_>>(),
        std::time::Duration::from_secs(30),
    )
    .unwrap();
    let info = video_probe::probe(&decode_engine(), &linked, || false).unwrap();
    let legacy = video_project::new_project(f.root.to_str().unwrap(), &linked, info).unwrap();
    let id = f.project.assets[0].id.clone();
    f.project.assets[0] = legacy.asset.into();
    f.project.assets[0].id = id;
    let original_bytes = fs::read(&linked).unwrap();
    let s = open_decode_session(&mut f, 2);
    let q = PreviewRequests::new();
    for frame in [15, 24, 25, 27, 29, 30] {
        preview_submit(&q, &mut f, &s, frame);
        let result = q
            .av_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
            .unwrap()
            .unwrap();
        let mean = result.pcm.iter().map(|v| f64::from(*v)).sum::<f64>() / result.pcm.len() as f64;
        assert!((mean - 0.25).abs() < 0.025, "frame {frame}: {mean}");
        assert_eq!(result.clipped_samples, 0);
    }
    assert_eq!(fs::read(linked).unwrap(), original_bytes);
}

#[test]
fn phase_two_added_layers_respect_fade_out_mute_and_exact_end() {
    let (mut f, _) = audio_preview_fixture();
    f.project.audio_tracks[0].gain_percent = 100;
    f.project.audio_tracks[0].fade_in_seconds = 0.0;
    f.project.audio_tracks[0].fade_out_seconds = 0.5;
    let mut muted = f.project.audio_tracks[0].clone();
    muted.id = "muted".into();
    muted.gain_percent = 0;
    f.project.audio_tracks.push(muted);
    let s = open_decode_session(&mut f, 3);
    let q = PreviewRequests::new();
    preview_submit(&q, &mut f, &s, 57);
    let result = q
        .av_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .unwrap()
        .unwrap();
    let expected = 0.8 * ((96000_u64 - 91200) * 2 - 1) as f64 / 48000.0;
    assert!((f64::from(result.pcm[0]) - expected).abs() < 0.001);
    drop(result);
    preview_submit(&q, &mut f, &s, 60);
    let result = q
        .av_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .unwrap()
        .unwrap();
    assert!(result.pcm.iter().all(|v| *v == 0.0));
}

#[test]
fn phase_two_rejects_changed_audio_and_releases_visual_ownership() {
    let (mut f, s) = audio_preview_fixture();
    let path = f.root.join("constant.wav");
    let mut bytes = fs::read(&path).unwrap();
    let index = bytes.len() - 1;
    bytes[index] ^= 1;
    fs::write(&path, &bytes).unwrap();
    let q = PreviewRequests::new();
    preview_submit(&q, &mut f, &s, 15);
    assert!(q
        .av_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .is_err());
    assert!(q.state.lock().unwrap().active.is_none());
    assert_eq!(fs::read(path).unwrap(), bytes);
    assert!(fs::OpenOptions::new()
        .write(true)
        .open(f.root.join("decoded-source.mp4"))
        .is_ok());
}

#[test]
fn phase_two_title_end_is_exclusive_and_positions_follow_canvas_percentages() {
    let (mut f, s) = decode_fixture();
    let q = PreviewRequests::new();
    preview_submit(&q, &mut f, &s, 27);
    let plain = q
        .compose_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .unwrap()
        .unwrap();
    let rgb = plain.frame.rgb.clone();
    drop(plain);
    preview_submit(&q, &mut f, &s, 27);
    let av = q
        .av_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(av.visual.frame.rgb, rgb);
    drop(av);
    let title = TitleOverlay {
        id: "position".into(),
        text: "XX".into(),
        start_seconds: 0.0,
        end_seconds: 1.0,
        x_percent: 0,
        y_percent: 0,
    };
    let mut left = crate::video_sequence_render::PreviewRgbPart {
        width: 320,
        height: 180,
        rgb: vec![0; 320 * 180 * 3],
    };
    crate::video_sequence_render::overlay_preview_titles(
        &decode_engine(),
        &mut left,
        std::slice::from_ref(&title),
        || false,
    )
    .unwrap();
    let mut right = crate::video_sequence_render::PreviewRgbPart {
        width: 320,
        height: 180,
        rgb: vec![0; 320 * 180 * 3],
    };
    let mut title = title;
    title.x_percent = 100;
    title.y_percent = 100;
    crate::video_sequence_render::overlay_preview_titles(
        &decode_engine(),
        &mut right,
        &[title],
        || false,
    )
    .unwrap();
    let white = |frame: &crate::video_sequence_render::PreviewRgbPart| {
        frame
            .rgb
            .as_chunks::<3>()
            .0
            .iter()
            .enumerate()
            .filter(|(_, rgb)| rgb.iter().all(|v| *v > 200))
            .map(|(i, _)| (i % 320, i / 320))
            .collect::<Vec<_>>()
    };
    let a = white(&left);
    let b = white(&right);
    assert!(!a.is_empty() && !b.is_empty());
    assert!(a.iter().all(|(x, y)| *x < 160 && *y < 90));
    assert!(b.iter().all(|(x, y)| *x >= 160 && *y >= 90));
}

#[test]
fn phase_two_repeated_audio_layers_sum_once_each_and_cancel_before_handoff() {
    let (mut f, _) = audio_preview_fixture();
    f.project.audio_tracks[0].gain_percent = 100;
    f.project.audio_tracks[0].fade_in_seconds = 0.0;
    let mut other_track = f.project.audio_tracks[0].clone();
    other_track.id = "second".into();
    f.project.audio_tracks.push(other_track);
    let s = open_decode_session(&mut f, 3);
    let q = PreviewRequests::new();
    preview_submit(&q, &mut f, &s, 15);
    let mixed = q
        .av_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .unwrap()
        .unwrap();
    assert!(mixed.pcm.iter().all(|v| *v == 1.0));
    assert_eq!(mixed.clipped_samples, mixed.pcm.len());
    assert_eq!(mixed.audio_pins.as_ref().unwrap().count(), 1);
    drop(mixed);
    let mut other = Connection::open(f.root.join("prototype.db")).unwrap();
    preview_submit(&q, &mut f, &s, 15);
    assert!(q
        .av_next(&mut f.db, &s, &s.receipt, &decode_engine(), |stage| {
            if stage == "after_pcm_layer" {
                q.submit(&mut other, &s, &s.receipt, SeekPoint::Frame(16))?;
            }
            Ok(())
        })
        .unwrap()
        .is_none());
    assert!(q.state.lock().unwrap().active.is_none());
}

#[test]
fn phase_two_sixty_fps_uses_exact_eight_hundred_sample_windows() {
    let (mut f, _) = audio_preview_fixture();
    f.project.frame_rate = 60;
    let s = open_decode_session(&mut f, 3);
    let q = PreviewRequests::new();
    preview_submit(&q, &mut f, &s, 30);
    let result = q
        .av_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(result.pcm.len(), 1600);
    for n in [0, 399, 799] {
        let expected = (0.8 * 2.0 * ((24000 + n) * 2 + 1) as f64 / 96000.0) as f32;
        assert!((result.pcm[n as usize * 2] - expected).abs() < 0.000001);
    }
}

#[test]
fn phase_two_subframe_track_start_keeps_leading_silence_and_channel_alignment() {
    let (mut f, _) = audio_preview_fixture();
    f.project.audio_tracks[0].start_seconds = 5.0 / 48000.0;
    f.project.audio_tracks[0].gain_percent = 100;
    f.project.audio_tracks[0].fade_in_seconds = 0.0;
    let s = open_decode_session(&mut f, 3);
    let q = PreviewRequests::new();
    preview_submit(&q, &mut f, &s, 0);
    let result = q
        .av_next(&mut f.db, &s, &s.receipt, &decode_engine(), |_| Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(result.pcm.len(), 3200);
    assert!(result.pcm[..10].iter().all(|v| *v == 0.0));
    assert!(result.pcm[10..].iter().all(|v| (*v - 0.8).abs() < 0.000001));
}
