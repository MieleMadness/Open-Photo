use crate::crossfade_clock::Boundary;
use crate::crossfade_layer_clock::{plan as layer_plan, Span};
fn edge(f: &Fixture, frames: u64) -> Boundary<'_> {
    Boundary {
        from_clip_id: &f.project.clips[0].id,
        to_clip_id: &f.project.clips[1].id,
        frames,
    }
}
fn add_photo(f: &mut Fixture) {
    let mut asset = f.project.assets[0].clone();
    asset.id = "photo-source".into();
    asset.relative_path = "still.png".into();
    asset.sha256 = "c".repeat(64);
    asset.media = Media::Photo {
        width: 96,
        height: 64,
    };
    let mut clip = crate::sequence_media::default_clip(&asset).unwrap();
    clip.id = "photo-clip".into();
    // Two seconds matches the fixture videos while preserving a photo's real
    // microsecond clock and geometry (no fabricated video/audio metadata).
    clip.source_out = video_project::VideoTime {
        ticks: "2000000".into(),
        time_base: "1/1000000".into(),
    };
    f.project.assets.push(asset);
    f.project.clips.push(clip);
}
fn add_title(f: &mut Fixture, start: f64, end: f64) {
    f.project.titles.push(TitleOverlay {
        id: "caption".into(),
        text: "One overlay".into(),
        start_seconds: start,
        end_seconds: end,
        x_percent: 50,
        y_percent: 80,
    });
}
fn add_audio(f: &mut Fixture) {
    f.project.audio_sources.push(AudioSource {
        id: "music".into(),
        relative_path: "music.wav".into(),
        sha256: "b".repeat(64),
        bytes: 123,
        info: video_probe::AudioOnlyInfo {
            duration_seconds: 10.0,
            stream: 0,
            codec: "pcm_s16le".into(),
            channels: 2,
            sample_rate: 48000,
        },
    });
    f.project.audio_tracks.push(AudioTrack {
        id: "track".into(),
        asset_id: "music".into(),
        start_seconds: 0.5,
        source_in_seconds: 1.0,
        duration_seconds: 2.0,
        gain_percent: 75,
        fade_in_seconds: 0.25,
        fade_out_seconds: 0.5,
    });
}
#[test]
fn mixed_photos_and_videos_share_native_frame_and_sample_clock() {
    let mut f = Fixture::new();
    add_photo(&mut f);
    let original = f.project.clone();
    let edges = [
        edge(&f, 12),
        Boundary {
            from_clip_id: &f.project.clips[1].id,
            to_clip_id: "photo-clip",
            frames: 18,
        },
    ];
    let p = layer_plan(&f.project, &edges).unwrap();
    assert_eq!(p.total_frames, 150);
    assert_eq!(p.total_samples, 240000);
    assert_eq!(
        p.clips
            .iter()
            .map(|c| (&c.frames.start, &c.frames.end))
            .collect::<Vec<_>>(),
        vec![(&0, &60), (&48, &108), (&90, &150)]
    );
    assert!(p.clips[2].photo);
    assert!(!p.clips[2].linked_audio);
    assert_eq!(
        p.clips[2].samples,
        Span {
            start: 144000,
            end: 240000
        }
    );
    assert_eq!(f.project, original);
    // The version-one persisted snapshot adapter still rejects mixed media.
    store(&f.db, f.root.to_str().unwrap(), &f.project, Some(1)).unwrap();
    assert!(capture(&mut f.db, f.root.to_str().unwrap(), &f.project.id).is_err());
}
#[test]
fn still_only_overlap_and_cumulative_rounding_work_at_both_frame_rates() {
    let mut f = Fixture::new();
    add_photo(&mut f);
    let photo = f.project.clips[2].clone();
    f.project.clips = (0..3)
        .map(|index| {
            let mut c = photo.clone();
            c.id = format!("still-{index}");
            c.source_out.ticks = "250000".into();
            c
        })
        .collect();
    for fps in [30, 60] {
        f.project.frame_rate = fps;
        let edges = [
            Boundary {
                from_clip_id: "still-0",
                to_clip_id: "still-1",
                frames: 2,
            },
            Boundary {
                from_clip_id: "still-1",
                to_clip_id: "still-2",
                frames: 2,
            },
        ];
        let p = layer_plan(&f.project, &edges).unwrap();
        let native = crate::video_sequence_render::frame_allocation(&f.project).unwrap();
        assert_eq!(
            p.clips
                .iter()
                .map(|c| c.frames.end - c.frames.start)
                .collect::<Vec<_>>(),
            native
        );
        assert_eq!(p.total_frames, if fps == 30 { 19 } else { 41 });
        assert_eq!(p.total_samples, p.total_frames * 48000 / u64::from(fps));
        assert!(p.clips.iter().all(|c| c.photo && !c.linked_audio));
    }
}
#[test]
fn titles_use_one_absolute_overlay_without_rippling_or_overlap_duplication() {
    let mut f = Fixture::new();
    add_title(&mut f, 1.5, 2.5);
    let p = layer_plan(&f.project, &[edge(&f, 15)]).unwrap();
    assert_eq!(p.total_frames, 105);
    assert_eq!(p.titles.len(), 1);
    assert_eq!(p.titles[0].frames, Span { start: 45, end: 75 });
    assert_eq!(p.titles[0].id, "caption");
    let hard = layer_plan(&f.project, &[]).unwrap();
    assert_eq!(p.titles, hard.titles);
    f.project.titles[0].end_seconds = 4.0;
    assert!(layer_plan(&f.project, &[edge(&f, 15)]).is_err());
    assert_eq!(f.project.titles[0].end_seconds, 4.0);
    f.project.titles[0].start_seconds = 0.001;
    f.project.titles[0].end_seconds = 0.002;
    assert!(layer_plan(&f.project, &[]).is_err());
}
#[test]
fn independent_audio_stays_absolute_and_uses_one_sample_exact_mix_span() {
    let mut f = Fixture::new();
    add_audio(&mut f);
    let p = layer_plan(&f.project, &[edge(&f, 15)]).unwrap();
    assert_eq!(p.audio.len(), 1);
    assert_eq!(
        p.audio[0].samples,
        Span {
            start: 24000,
            end: 120000
        }
    );
    assert_eq!(
        p.audio[0].source_samples,
        Span {
            start: 48000,
            end: 144000
        }
    );
    assert_eq!(
        (
            p.audio[0].fade_in_samples,
            p.audio[0].fade_out_samples,
            p.audio[0].gain_percent
        ),
        (12000, 24000, 75)
    );
    assert_eq!(p.audio, layer_plan(&f.project, &[]).unwrap().audio);
    f.project.audio_tracks[0].start_seconds = 2.0;
    assert!(layer_plan(&f.project, &[edge(&f, 15)]).is_err());
    assert_eq!(f.project.audio_tracks[0].duration_seconds, 2.0);
}
#[test]
fn linked_audio_respects_saved_stream_mute_and_gain_without_inventing_photo_audio() {
    let mut f = Fixture::new();
    add_photo(&mut f);
    let info=video_probe::parse_probe(br#"{"streams":[{"index":0,"codec_type":"video","codec_name":"h264","width":96,"height":64,"time_base":"1/90000"},{"index":1,"codec_type":"audio","codec_name":"aac","channels":2,"sample_rate":"48000"}],"format":{"duration":"2"}}"#).unwrap();
    f.project.assets[0].media = Media::Video {
        info: Box::new(info),
    };
    f.project.clips[1].edits.muted = true;
    let p = layer_plan(&f.project, &[edge(&f, 15)]).unwrap();
    assert!(p.clips[0].linked_audio);
    assert!(!p.clips[1].linked_audio);
    assert!(!p.clips[2].linked_audio);
    assert_eq!(p.clips[0].samples.end - p.clips[1].samples.start, 24000);
    f.project.clips[0].edits.gain_percent = 0;
    assert!(!layer_plan(&f.project, &[]).unwrap().clips[0].linked_audio);
    // A separately added track referencing the same video uses its own gain;
    // muting linked clip audio must not mute this explicit project layer.
    f.project.audio_tracks.push(AudioTrack {
        id: "video-track".into(),
        asset_id: f.project.assets[0].id.clone(),
        start_seconds: 0.0,
        source_in_seconds: 0.5,
        duration_seconds: 1.0,
        gain_percent: 80,
        fade_in_seconds: 0.0,
        fade_out_seconds: 0.0,
    });
    let p = layer_plan(&f.project, &[]).unwrap();
    assert_eq!(
        p.audio[0].source_samples,
        Span {
            start: 24000,
            end: 72000
        }
    );
    assert_eq!(p.audio[0].gain_percent, 80);
}
#[test]
fn dips_and_crossfades_share_a_clean_frame_budget_and_never_share_a_boundary() {
    let mut f = Fixture::new();
    add_photo(&mut f);
    f.project.transitions.push(CutTransition {
        from_clip_id: f.project.clips[1].id.clone(),
        to_clip_id: "photo-clip".into(),
        kind: CutTransitionKind::DipToBlack,
        duration_seconds: 0.4,
    });
    let p = layer_plan(&f.project, &[edge(&f, 15)]).unwrap();
    assert_eq!(p.total_frames, 165); // A dip does not shorten its cut.
    assert_eq!(p.clips[1].dip_out_frames, 6);
    assert_eq!(p.clips[2].dip_in_frames, 6);
    let hard = layer_plan(&f.project, &[]).unwrap();
    assert_eq!(hard.total_frames, 180);
    assert!(layer_plan(&f.project, &[edge(&f, 53)]).is_err()); // 53+6+2 > 60.
    let conflict = Boundary {
        from_clip_id: &f.project.clips[1].id,
        to_clip_id: "photo-clip",
        frames: 5,
    };
    assert!(layer_plan(&f.project, &[conflict]).is_err());
    // A nonintegral half-dip reserves all touched frames, rather than rounding
    // down and allowing its fade to collide with the overlap envelope.
    f.project.transitions[0].duration_seconds = 0.09;
    assert_eq!(
        layer_plan(&f.project, &[]).unwrap().clips[1].dip_out_frames,
        2
    );
}
#[test]
fn saved_layer_plan_binds_all_layer_edits_to_real_document_and_revision() {
    let mut f = Fixture::new();
    add_photo(&mut f);
    add_title(&mut f, 1.5, 2.5);
    add_audio(&mut f);
    store(&f.db, f.root.to_str().unwrap(), &f.project, Some(1)).unwrap();
    let raw = f.raw_project();
    let edges = [edge(&f, 15)];
    // End the immutable edge borrow before borrowing the fixture connection.
    let from = edges[0].from_clip_id.to_owned();
    let to = edges[0].to_clip_id.to_owned();
    let edges = [Boundary {
        from_clip_id: &from,
        to_clip_id: &to,
        frames: 15,
    }];
    let first =
        capture_layer_plan(&mut f.db, f.root.to_str().unwrap(), &f.project.id, &edges).unwrap();
    assert_eq!(first.revision, 2);
    assert_eq!(first.project_id, f.project.id);
    assert_eq!(first.root, root_key(f.root.to_str().unwrap()).unwrap());
    assert_eq!(first.timing.total_frames, 165);
    assert_eq!(f.raw_project(), raw);
    let mut changed = serde_json::to_value(&f.project).unwrap();
    changed["titles"][0]["text"] = json!("Edited title");
    f.db.execute(
        "UPDATE video_sequences SET data=?1 WHERE id=?2",
        params![changed.to_string(), f.project.id],
    )
    .unwrap();
    f.reopen();
    let next =
        capture_layer_plan(&mut f.db, f.root.to_str().unwrap(), &f.project.id, &edges).unwrap();
    assert_eq!(first.revision, next.revision);
    assert_eq!(first.timing, next.timing);
    assert_ne!(first.document_hash, next.document_hash);
    let other = Fixture::new();
    assert!(capture_layer_plan(
        &mut f.db,
        other.root.to_str().unwrap(),
        &f.project.id,
        &edges
    )
    .is_err());
    f.db.execute("UPDATE video_sequences SET revision=?1", [JS_SAFE + 1])
        .unwrap();
    assert!(
        capture_layer_plan(&mut f.db, f.root.to_str().unwrap(), &f.project.id, &edges).is_err()
    );
}
#[test]
fn invalid_project_edges_layer_sources_and_quantized_fades_are_refused() {
    let mut f = Fixture::new();
    add_audio(&mut f);
    let mut invalid = f.project.clone();
    invalid.frame_rate = 24;
    assert!(layer_plan(&invalid, &[]).is_err());
    invalid = f.project.clone();
    invalid.audio_tracks[0].asset_id = "missing".into();
    assert!(layer_plan(&invalid, &[]).is_err());
    invalid = f.project.clone();
    invalid.audio_sources[0].id = invalid.assets[0].id.clone();
    assert!(layer_plan(&invalid, &[]).is_err());
    let e = edge(&f, 15);
    assert!(layer_plan(&f.project, &[e.clone(), e]).is_err());
    // Duration rounds to 1,920 samples, while two independently rounded fades
    // total 1,921. Refuse rather than emit inconsistent audio envelopes.
    f.project.audio_tracks[0].start_seconds = 0.0;
    f.project.audio_tracks[0].duration_seconds = 0.04001;
    f.project.audio_tracks[0].fade_in_seconds = 0.020012;
    f.project.audio_tracks[0].fade_out_seconds = 0.019998;
    assert!(layer_plan(&f.project, &[]).is_err());
}
