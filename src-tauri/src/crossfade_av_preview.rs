// Test-only phase 2: exact 48 kHz stereo windows and post-transition global titles.
struct WorkspacePreviewAccess<'a> {
    settings: &'a Connection,
    projects: &'a Connection,
    workspace: &'a str,
    revision: u64,
}
struct AudioVisualPreview {
    visual: ComposedVisualPreview,
    pcm: Vec<f32>,
    clipped_samples: usize,
    audio_pins: Option<crate::crossfade_windows_guard::PinnedMedia>,
    workspace: Option<crate::video_workspace::PhaseJPreviewSources>,
}
impl AudioVisualPreview {
    fn validate(&self) -> Result<(), String> {
        self.visual.validate()?;
        let seek = &self.visual.inputs.request.position;
        if !matches!(seek.fps, 30 | 60) {
            return Err("Invalid AV frame rate".into());
        }
        let samples = seek
            .samples
            .end
            .checked_sub(seek.samples.start)
            .ok_or("Invalid PCM window")?;
        if samples != 48000 / u64::from(seek.fps)
            || self.pcm.len() as u64 != samples * 2
            || self.pcm.iter().any(|v| !v.is_finite() || v.abs() > 1.0)
            || self.clipped_samples > self.pcm.len()
        {
            return Err("Invalid composed PCM payload".into());
        }
        if seek.audio.iter().any(|slice| !slice.silence)
            && self
                .audio_pins
                .as_ref()
                .is_none_or(|pins| pins.count() == 0)
        {
            return Err("Missing PCM source ownership".into());
        }
        Ok(())
    }
}
fn audio_visual_inputs(
    db: &mut Connection,
    s: &EditorSession,
    inputs: DecodedPreviewInputs,
    engine: &Path,
    mut step: impl FnMut(&str) -> Result<(), String>,
) -> Result<AudioVisualPreview, String> {
    let mut visual = compose_visual_inputs(db, s, inputs, &mut step)?;
    let request = visual.inputs.request.clone();
    request.verify(db, s, &s.receipt)?;
    let manifest = load_layer_manifest(db, &s.root, &s.project_id, &s.receipt.current_id)?;
    let project = parse(manifest.project_json.as_bytes())?;
    let seek = &request.position;
    if seek.audio.len() > 32 {
        return Err("Too many preview audio layers".into());
    }
    let mut sources = Vec::new();
    for slice in seek.audio.iter().filter(|slice| !slice.silence) {
        let root = visual
            .inputs
            .source_roots
            .get(&slice.asset_id)
            .ok_or("Missing PCM source binding")?;
        let (path, bytes, hash, stream, base) =
            if let Some(asset) = project.assets.iter().find(|a| a.id == slice.asset_id) {
                let Media::Video { info } = &asset.media else {
                    return Err("Photo cannot supply PCM".into());
                };
                let stream = info.audio.first().ok_or("Missing audio stream")?.stream;
                let base = if slice.origin == SeekAudioOrigin::NormalizedClip {
                    let clip = project
                        .clips
                        .iter()
                        .find(|c| c.id == slice.id)
                        .ok_or("Missing linked audio clip")?;
                    (clip.source_in.seconds()? * 48000.0).round() as u64
                } else {
                    0
                };
                (
                    crate::sequence_media::source(root, asset, || request.cancelled())?,
                    asset.bytes,
                    asset.sha256.clone(),
                    stream,
                    base,
                )
            } else {
                let asset = project
                    .audio_sources
                    .iter()
                    .find(|a| a.id == slice.asset_id)
                    .ok_or("Missing independent audio")?;
                (
                    video_probe::safe_audio_path(
                        root,
                        &Path::new(root).join(&asset.relative_path).to_string_lossy(),
                    )?,
                    asset.bytes,
                    asset.sha256.clone(),
                    asset.info.stream,
                    0,
                )
            };
        let start = base
            .checked_add(slice.source_sample_start)
            .ok_or("PCM seek overflow")?;
        let count = slice.samples.end - slice.samples.start;
        if bytes > 128 * 1024 * 1024 || start.checked_add(count).is_none_or(|end| end > 480000) {
            return Err("Gated PCM source exceeds ten-second/file budget".into());
        }
        sources.push((path, bytes, hash, stream, start));
    }
    let mut unique = std::collections::BTreeMap::new();
    for (path, bytes, hash, _, _) in &sources {
        if let Some((old_bytes, old_hash)) = unique.insert(path, (*bytes, hash)) {
            if old_bytes != *bytes || old_hash != hash {
                return Err("Conflicting audio source bindings".into());
            }
        }
    }
    let bindings = unique
        .iter()
        .map(|(path, (bytes, hash))| (path.as_path(), *bytes, hash.as_str()))
        .collect::<Vec<_>>();
    // Silent windows need no new sources; visual ownership remains live.
    let audio_pins = if bindings.is_empty() {
        None
    } else {
        Some(crate::crossfade_windows_guard::PinnedMedia::acquire(
            &bindings,
            || request.cancelled(),
        )?)
    };
    let mut mixed = vec![0.0_f64; ((seek.samples.end - seek.samples.start) * 2) as usize];
    for (slice, source) in seek
        .audio
        .iter()
        .filter(|slice| !slice.silence)
        .zip(&sources)
    {
        if request.cancelled() {
            return Err("PCM preview cancelled".into());
        }
        let count = slice.samples.end - slice.samples.start;
        let filter=format!("aresample=48000:async=1:first_pts=0,aformat=sample_fmts=flt:channel_layouts=stereo,atrim=start_sample={}:end_sample={},asetpts=PTS-STARTPTS,apad=whole_len={count},atrim=end_sample={count}",source.4,source.4+count);
        let args: Vec<std::ffi::OsString> = vec![
            "-nostdin".into(),
            "-v".into(),
            "error".into(),
            "-xerror".into(),
            "-threads".into(),
            "1".into(),
            "-filter_threads".into(),
            "1".into(),
            "-max_alloc".into(),
            "268435456".into(),
            "-protocol_whitelist".into(),
            "file,pipe".into(),
            "-format_whitelist".into(),
            "mov,matroska,webm,avi,mp3,wav,flac".into(),
            "-i".into(),
            source.0.clone().into(),
            "-map".into(),
            format!("0:{}", source.3).into(),
            "-vn".into(),
            "-af".into(),
            filter.into(),
            "-ac".into(),
            "2".into(),
            "-ar".into(),
            "48000".into(),
            "-f".into(),
            "f32le".into(),
            "pipe:1".into(),
        ];
        let raw = video_engine::run_until(
            &video_engine::tool(engine, "ffmpeg")?,
            &args.iter().map(|a| a.as_os_str()).collect::<Vec<_>>(),
            std::time::Duration::from_secs(30),
            || request.cancelled(),
        )?;
        if raw.len() as u64 != count * 8 {
            return Err("Incomplete PCM source window".into());
        }
        for (index, chunk) in raw.as_chunks::<4>().0.iter().enumerate() {
            let value = f32::from_le_bytes(*chunk);
            if !value.is_finite() {
                return Err("Nonfinite decoded PCM".into());
            }
            let sample = slice.samples.start + index as u64 / 2;
            let target = ((slice.samples.start - seek.samples.start) * 2) as usize + index;
            mixed[target] += f64::from(value) * slice.gain_at(sample)?;
        }
        step("after_pcm_layer")?;
    }
    let clipped_samples = mixed.iter().filter(|v| v.abs() > 1.0).count();
    let pcm = mixed
        .into_iter()
        .map(|v| v.clamp(-1.0, 1.0) as f32)
        .collect();
    let titles = project
        .titles
        .iter()
        .filter(|title| seek.titles.contains(&title.id))
        .cloned()
        .collect::<Vec<_>>();
    crate::video_sequence_render::overlay_preview_titles(
        engine,
        &mut visual.frame,
        &titles,
        || request.cancelled(),
    )?;
    step("after_av_composition")?;
    if request.cancelled() {
        return Err("AV preview cancelled".into());
    }
    request.verify(db, s, &s.receipt)?;
    let result = AudioVisualPreview {
        visual,
        pcm,
        clipped_samples,
        audio_pins,
        workspace: None,
    };
    result.validate()?;
    Ok(result)
}
impl PreviewRequests {
    fn workspace_av_next(
        &self,
        db: &mut Connection,
        s: &EditorSession,
        engine: &Path,
        access: WorkspacePreviewAccess<'_>,
        mut step: impl FnMut(&str) -> Result<(), String>,
    ) -> Result<Option<AudioVisualPreview>, String> {
        let Some(request) = self.start_next(db, s, &s.receipt)? else {
            return Ok(None);
        };
        // Admission failures also complete the worker: never strand its slot.
        let result = (|| {
            let sources = crate::video_workspace::PhaseJPreviewSources::admit(
                access.settings,
                access.projects,
                access.workspace,
                &s.project_id,
                access.revision,
            )?;
            let inputs =
                decode_preview_inputs_from(db, s, &request, engine, Some(&sources), &mut step)?;
            let mut output = audio_visual_inputs(db, s, inputs, engine, &mut step)?;
            if !sources.current(access.settings, access.projects)? {
                return Err("Workspace preview changed during composition".into());
            }
            output.workspace = Some(sources);
            Ok(output)
        })();
        self.complete(
            db,
            s,
            &s.receipt,
            &request,
            result,
            AudioVisualPreview::validate,
        )
    }
    fn workspace_av_current(
        &self,
        db: &mut Connection,
        s: &EditorSession,
        result: &AudioVisualPreview,
        settings: &Connection,
        projects: &Connection,
    ) -> Result<bool, String> {
        result.validate()?;
        if !self.visual_current(db, s, &s.receipt, &result.visual)? {
            return Ok(false);
        }
        result
            .workspace
            .as_ref()
            .ok_or("Missing workspace preview authorization")?
            .current(settings, projects)
    }
    fn av_next(
        &self,
        db: &mut Connection,
        s: &EditorSession,
        receipt: &SessionReceipt,
        engine: &Path,
        mut step: impl FnMut(&str) -> Result<(), String>,
    ) -> Result<Option<AudioVisualPreview>, String> {
        let Some(request) = self.start_next(db, s, receipt)? else {
            return Ok(None);
        };
        let result = decode_preview_inputs(db, s, &request, engine, &mut step)
            .and_then(|inputs| audio_visual_inputs(db, s, inputs, engine, &mut step));
        self.complete(
            db,
            s,
            receipt,
            &request,
            result,
            AudioVisualPreview::validate,
        )
    }
    fn av_current(
        &self,
        db: &mut Connection,
        s: &EditorSession,
        receipt: &SessionReceipt,
        result: &AudioVisualPreview,
    ) -> Result<bool, String> {
        if result.workspace.is_some() {
            return Err("Workspace preview requires authoritative workspace delivery check".into());
        }
        result.validate()?;
        self.visual_current(db, s, receipt, &result.visual)
    }
}
