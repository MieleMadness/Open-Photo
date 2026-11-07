fn open_seek_project(f: &mut Fixture, revision: u64) -> EditorSession {
    store(&f.db, f.root.to_str().unwrap(), &f.project, Some(revision)).unwrap();
    let from = f.project.clips[0].id.clone();
    let to = f.project.clips[1].id.clone();
    let data = prepare_layer_manifest(
        &mut f.db,
        f.root.to_str().unwrap(),
        &f.project.id,
        &[Boundary {
            from_clip_id: &from,
            to_clip_id: &to,
            frames: 15,
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
fn seek_fixture(fps: u32, audio: bool) -> (Fixture, EditorSession) {
    let (mut f, _) = manifest_fixture();
    f.project.frame_rate = fps;
    if audio {
        if let Media::Video { info } = &mut f.project.assets[0].media {
            info.audio.push(video_probe::AudioInfo {
                stream: 1,
                codec: "aac".into(),
                sample_rate: Some("48000".into()),
                channels: Some(2),
                channel_layout: Some("stereo".into()),
                start_time: Some("0".into()),
                duration_seconds: Some(2.0),
            });
        }
    }
    let s = open_seek_project(&mut f, 2);
    (f, s)
}
fn at(f: &mut Fixture, s: &EditorSession, frame: u64) -> SessionSeek {
    seek_session(&mut f.db, s, &s.receipt, SeekPoint::Frame(frame)).unwrap()
}
#[test]
fn seek_crossfade_centers_and_title_boundaries_use_one_project_clock() {
    let (mut f, s) = seek_fixture(30, false);
    let start = at(&mut f, &s, 45);
    assert_eq!(start.video.len(), 2);
    assert_eq!(start.video[0].part_frame, 45);
    assert_eq!(start.video[1].part_frame, 0);
    assert_eq!(
        start.video[0].weight,
        SeekWeight {
            numerator: 29,
            denominator: 30
        }
    );
    assert_eq!(
        start.video[1].weight,
        SeekWeight {
            numerator: 1,
            denominator: 30
        }
    );
    assert_eq!(start.titles, vec!["caption"]);
    assert_eq!(
        start.samples,
        SeekSpan {
            start: 72000,
            end: 73600
        }
    );
    let last = at(&mut f, &s, 59);
    assert_eq!(last.video[0].weight.numerator, 1);
    assert_eq!(last.video[1].weight.numerator, 29);
    let next = at(&mut f, &s, 60);
    assert_eq!(next.video.len(), 1);
    assert_eq!(next.video[0].part_frame, 15);
    assert_eq!(
        next.video[0].weight,
        SeekWeight {
            numerator: 1,
            denominator: 1
        }
    );
    assert!(at(&mut f, &s, 44).titles.is_empty());
    assert_eq!(at(&mut f, &s, 74).titles.len(), 1);
    assert!(at(&mut f, &s, 75).titles.is_empty());
}
#[test]
fn seek_audio_linear_envelopes_preserve_independent_gain_and_silence() {
    let (mut f, s) = seek_fixture(30, true);
    let position = at(&mut f, &s, 45);
    let linked = position
        .audio
        .iter()
        .filter(|a| a.origin == SeekAudioOrigin::NormalizedClip)
        .collect::<Vec<_>>();
    assert_eq!(linked.len(), 2);
    assert_eq!(linked[0].source_sample_start, 72000);
    assert_eq!(linked[1].source_sample_start, 0);
    assert_eq!(
        linked[0].fade_out,
        Some(SeekSpan {
            start: 72000,
            end: 96000
        })
    );
    assert_eq!(
        linked[1].fade_in,
        Some(SeekSpan {
            start: 72000,
            end: 96000
        })
    );
    for sample in [72000, 72800, 73599] {
        assert!(
            (linked[0].gain_at(sample).unwrap() + linked[1].gain_at(sample).unwrap() - 1.0).abs()
                < 1e-12
        );
    }
    assert!((linked[1].gain_at(72000).unwrap() - 1.0 / 48000.0).abs() < 1e-12);
    let music = position
        .audio
        .iter()
        .find(|a| a.origin == SeekAudioOrigin::AddedTrack)
        .unwrap();
    assert_eq!(music.source_sample_start, 96000);
    assert!((music.gain_at(72000).unwrap() - 0.75).abs() < 1e-12);
    assert_eq!(
        music.fade_in,
        Some(SeekSpan {
            start: 24000,
            end: 36000
        })
    );
    assert_eq!(
        music.fade_out,
        Some(SeekSpan {
            start: 96000,
            end: 120000
        })
    );
    assert!(music.gain_at(position.samples.end).is_err());
    f.project.clips[1].edits.muted = true;
    let muted = open_seek_project(&mut f, 3);
    let p = at(&mut f, &muted, 45);
    assert!(p.audio[1].silence);
    assert_eq!(p.audio[1].gain_at(72000).unwrap(), 0.0);
    assert!((p.audio[0].gain_at(72000).unwrap() - linked[0].gain_at(72000).unwrap()).abs() < 1e-12);
}
#[test]
fn subframe_audio_starts_and_ends_are_preserved_inside_seek_blocks() {
    let (mut f, _) = seek_fixture(30, false);
    let track = &mut f.project.audio_tracks[0];
    track.start_seconds = 24001.0 / 48000.0;
    track.duration_seconds = 0.04;
    track.fade_in_seconds = 0.0;
    track.fade_out_seconds = 0.0;
    let s = open_seek_project(&mut f, 3);
    assert!(at(&mut f, &s, 14)
        .audio
        .iter()
        .all(|a| a.origin != SeekAudioOrigin::AddedTrack));
    let first = at(&mut f, &s, 15);
    let audio = first
        .audio
        .iter()
        .find(|a| a.origin == SeekAudioOrigin::AddedTrack)
        .unwrap();
    assert_eq!(
        audio.samples,
        SeekSpan {
            start: 24001,
            end: 25600
        }
    );
    assert_eq!(audio.source_sample_start, 48000);
    assert!(audio.gain_at(24000).is_err());
    let next = at(&mut f, &s, 16);
    let audio = next
        .audio
        .iter()
        .find(|a| a.origin == SeekAudioOrigin::AddedTrack)
        .unwrap();
    assert_eq!(
        audio.samples,
        SeekSpan {
            start: 25600,
            end: 25921
        }
    );
    assert_eq!(audio.source_sample_start, 49599);
    assert!(at(&mut f, &s, 17)
        .audio
        .iter()
        .all(|a| a.origin != SeekAudioOrigin::AddedTrack));
}
#[test]
fn photo_crossfades_and_dip_cuts_keep_distinct_video_and_audio_state() {
    let (mut f, s) = seek_fixture(30, true);
    let before = at(&mut f, &s, 104);
    assert_eq!(before.video[0].dip_out_frames, 6);
    assert_eq!(before.video[0].part_frame, 59);
    let after = at(&mut f, &s, 105);
    assert_eq!(after.video.len(), 1);
    assert!(after.video[0].photo);
    assert_eq!(after.video[0].part_frame, 0);
    assert_eq!(after.video[0].dip_in_frames, 6);
    assert!(after
        .audio
        .iter()
        .filter(|a| a.origin == SeekAudioOrigin::NormalizedClip)
        .all(|a| a.silence));
    // Dips reserve local video fades but do not fade linked audio.
    assert!(before
        .audio
        .iter()
        .filter(|a| a.origin == SeekAudioOrigin::NormalizedClip)
        .all(|a| a.fade_out.is_none()));
    f.project.transitions.clear();
    let mut mixed = open_seek_project(&mut f, 3);
    mixed
        .apply(
            &mut f.db,
            &mixed.receipt.clone(),
            SessionAction::Crossfade {
                from: &f.project.clips[1].id,
                to: "photo-clip",
                frames: Some(12),
            },
            |_| Ok(()),
        )
        .unwrap();
    let overlap = at(&mut f, &mixed, 93);
    assert_eq!(overlap.video.len(), 2);
    assert!(overlap.video[1].photo);
    assert_eq!(
        overlap.video[1].weight,
        SeekWeight {
            numerator: 1,
            denominator: 24
        }
    );
    assert_eq!(overlap.video[1].part_frame, 0);
    let photo = overlap.audio.iter().find(|a| a.id == "photo-clip").unwrap();
    assert!(photo.silence);
    assert_eq!(photo.source_sample_start, 0);
    assert_eq!(photo.gain_at(148800).unwrap(), 0.0);
}
#[test]
fn whole_timeline_seeks_cover_every_frame_without_duplicate_endpoints_at_both_rates() {
    for fps in [30, 60] {
        let (mut f, s) = seek_fixture(fps, true);
        let m =
            load_layer_manifest(&mut f.db, &s.root, &s.project_id, &s.receipt.current_id).unwrap();
        let mut sample_end = 0;
        for frame in 0..m.timing.total_frames {
            let p = seek_verified_manifest(&s, &m, SeekPoint::Frame(frame)).unwrap();
            assert_eq!(p.samples.start, sample_end);
            sample_end = p.samples.end;
            assert!((1..=2).contains(&p.video.len()));
            let sum = p
                .video
                .iter()
                .map(|v| v.weight.numerator as f64 / v.weight.denominator as f64)
                .sum::<f64>();
            assert!((sum - 1.0).abs() < 1e-12);
            assert_eq!(
                p.audio
                    .iter()
                    .filter(|a| a.origin == SeekAudioOrigin::NormalizedClip)
                    .count(),
                p.video.len()
            );
            for a in &p.audio {
                assert!(a.samples.start >= p.samples.start && a.samples.end <= p.samples.end);
                assert!(a.samples.start < a.samples.end);
            }
        }
        assert_eq!(sample_end, m.timing.total_samples);
        assert!(seek_verified_manifest(&s, &m, SeekPoint::Frame(m.timing.total_frames)).is_err());
    }
}
#[test]
fn seek_times_match_frame_quantization_and_reject_invalid_or_exclusive_end() {
    let (mut f, s) = seek_fixture(30, false);
    for (seconds, frame) in [
        (0.0, 0),
        (1.5, 45),
        (1.5 - 1e-8, 44),
        (1.51, 45),
        (2.0, 60),
        (5.5 - 0.5 / 30.0, 164),
    ] {
        let p = seek_session(&mut f.db, &s, &s.receipt, SeekPoint::Seconds(seconds)).unwrap();
        assert_eq!(p.project_frame, frame);
    }
    for seconds in [-1.0, f64::NAN, f64::INFINITY, 5.5, 6.0] {
        assert!(seek_session(&mut f.db, &s, &s.receipt, SeekPoint::Seconds(seconds)).is_err());
    }
    assert!(seek_session(&mut f.db, &s, &s.receipt, SeekPoint::Frame(u64::MAX)).is_err());
    assert!(linear_seek_weight(&SeekSpan { start: 1, end: 1 }, 1, true).is_err());
    assert!(linear_seek_weight(
        &SeekSpan {
            start: 0,
            end: u64::MAX
        },
        0,
        true
    )
    .is_err());
}
#[test]
fn resumed_seek_is_read_only_receipt_bound_and_revalidates_native_state() {
    let (mut f, original) = seek_fixture(30, false);
    initialize_checkpoint_store(&f.db).unwrap();
    let receipt = save_checkpoint(&mut f.db, &original, None, |_| Ok(())).unwrap();
    f.reopen();
    f.reopen();
    let mut s =
        resume_checkpoint(&mut f.db, &original.root, &original.project_id, &receipt).unwrap();
    let raw = f.raw_project();
    let manifest_rows = manifest_count(&f);
    let first = at(&mut f, &s, 45);
    assert_eq!(first.session, s.receipt);
    assert_eq!(first.document_hash, payload_hash(&raw));
    assert_eq!(first.project_revision, 3);
    assert_eq!(manifest_count(&f), manifest_rows);
    assert_eq!(checkpoint_count(&f), 1);
    assert_eq!(f.raw_project(), raw);
    let old = s.receipt.clone();
    session_edit(&mut f, &mut s, Some(18)).unwrap();
    assert!(seek_session(&mut f.db, &s, &old, SeekPoint::Frame(45)).is_err());
    assert_ne!(first.session, at(&mut f, &s, 45).session);
    let mut changed: Value = serde_json::from_str(&raw).unwrap();
    changed["titles"][0]["text"] = json!("changed after resume");
    f.db.execute(
        "UPDATE video_sequences SET data=?1 WHERE id=?2",
        params![changed.to_string(), s.project_id],
    )
    .unwrap();
    assert!(seek_session(&mut f.db, &s, &s.receipt, SeekPoint::Frame(45)).is_err());
    f.db.execute(
        "UPDATE video_sequences SET data=?1 WHERE id=?2",
        params![raw, s.project_id],
    )
    .unwrap();
    f.db.execute(
        "UPDATE phase_j_layer_manifests_test SET data='corrupt' WHERE id=?1",
        [&s.receipt.current_id],
    )
    .unwrap();
    assert!(seek_session(&mut f.db, &s, &s.receipt, SeekPoint::Frame(45)).is_err());
}
