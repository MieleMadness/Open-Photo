// Windows test-only decoder. Source pins live with returned raw inputs through
// later composition; these are not a finished preview image or playable audio.
struct DecodedPreviewPart {
    clip_id: String,
    frame: crate::video_sequence_render::PreviewRgbPart,
}
struct DecodedPreviewInputs {
    request: PreviewArc<PreviewRequest>,
    parts: Vec<DecodedPreviewPart>,
    originals: crate::crossfade_windows_guard::PinnedMedia,
    source_roots: std::collections::BTreeMap<String, String>,
}
impl DecodedPreviewInputs {
    fn validate(&self) -> Result<(), String> {
        if self.parts.len() != self.request.position.video.len()
            || self.parts.is_empty()
            || self.parts.len() > 2
            || self.originals.count() == 0
        {
            return Err("Decoded input ownership/count mismatch".into());
        }
        let mut bytes = 0usize;
        let first = &self.parts[0].frame;
        for (part, seek) in self.parts.iter().zip(&self.request.position.video) {
            if part.clip_id != seek.clip_id
                || part.frame.width != first.width
                || part.frame.height != first.height
                || part.frame.width == 0
                || part.frame.height == 0
                || part.frame.width > 960
                || part.frame.height > 540
                || part.frame.rgb.len() as u64
                    != u64::from(part.frame.width) * u64::from(part.frame.height) * 3
            {
                return Err("Decoded input frame geometry/clock mismatch".into());
            }
            bytes = bytes
                .checked_add(part.frame.rgb.len())
                .ok_or("Decoded preview byte overflow")?;
        }
        if bytes > PREVIEW_RESULT_LIMIT {
            return Err("Decoded preview inputs exceed byte budget".into());
        }
        Ok(())
    }
}
fn decode_preview_inputs(
    db: &mut Connection,
    s: &EditorSession,
    request: &PreviewArc<PreviewRequest>,
    engine: &Path,
    step: impl FnMut(&str) -> Result<(), String>,
) -> Result<DecodedPreviewInputs, String> {
    decode_preview_inputs_from(db, s, request, engine, None, step)
}
fn decode_preview_inputs_from(
    db: &mut Connection,
    s: &EditorSession,
    request: &PreviewArc<PreviewRequest>,
    engine: &Path,
    workspace: Option<&crate::video_workspace::PhaseJPreviewSources>,
    mut step: impl FnMut(&str) -> Result<(), String>,
) -> Result<DecodedPreviewInputs, String> {
    request.verify(db, s, &s.receipt)?;
    if request.cancelled() {
        return Err("Preview decode cancelled".into());
    }
    let manifest = load_layer_manifest(db, &s.root, &s.project_id, &s.receipt.current_id)?;
    let project = parse(manifest.project_json.as_bytes())?;
    let source_roots = match workspace {
        Some(workspace) => workspace.roots_for(&project)?.clone(),
        None => project
            .assets
            .iter()
            .map(|asset| asset.id.clone())
            .chain(project.audio_sources.iter().map(|asset| asset.id.clone()))
            .map(|id| (id, s.root.clone()))
            .collect(),
    };
    let frames = crate::video_sequence_render::frame_allocation(&project)?;
    let mut sources = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for seek in &request.position.video {
        let index = project
            .clips
            .iter()
            .position(|c| c.id == seek.clip_id)
            .ok_or("Missing decoded clip")?;
        let clip = &project.clips[index];
        let asset = project
            .assets
            .iter()
            .find(|a| a.id == clip.asset_id)
            .ok_or("Missing decoded source")?;
        let (w, h) = match &asset.media {
            Media::Video { info } => (info.width, info.height),
            Media::Photo { width, height } => (*width, *height),
        };
        if asset.bytes > 128 * 1024 * 1024
            || u64::from(w) * u64::from(h) > 8_388_608
            || frames[index] > u64::from(project.frame_rate) * 10
        {
            return Err("Gated preview source exceeds file/pixel/clip-duration budget".into());
        }
        let root = source_roots
            .get(&asset.id)
            .ok_or("Missing preview source binding")?;
        if fs::metadata(Path::new(root).join(&asset.relative_path))
            .map_err(|e| e.to_string())?
            .len()
            != asset.bytes
        {
            return Err("Preview source size changed".into());
        }
        let source = crate::sequence_media::source(root, asset, || request.cancelled())?;
        if seen.insert(asset.id.clone()) {
            sources.push((source, asset.bytes, asset.sha256.clone()));
        }
    }
    let expected = sources
        .iter()
        .map(|(path, bytes, hash)| (path.as_path(), *bytes, hash.as_str()))
        .collect::<Vec<_>>();
    let originals =
        crate::crossfade_windows_guard::PinnedMedia::acquire(&expected, || request.cancelled())?;
    step("after_sources_pinned")?;
    let mut parts = Vec::new();
    for seek in &request.position.video {
        step("before_part_decode")?;
        if request.cancelled() {
            return Err("Preview decode cancelled".into());
        }
        let index = project
            .clips
            .iter()
            .position(|c| c.id == seek.clip_id)
            .ok_or("Missing preview clip")?;
        let frame = crate::video_sequence_render::decode_preview_part(
            engine,
            &project,
            source_roots
                .get(&seek.asset_id)
                .ok_or("Missing preview decoder root")?,
            index,
            seek.part_frame,
            || request.cancelled(),
        )?;
        parts.push(DecodedPreviewPart {
            clip_id: seek.clip_id.clone(),
            frame,
        });
        step("after_part_decode")?;
    }
    let decoded = DecodedPreviewInputs {
        request: request.clone(),
        parts,
        originals,
        source_roots,
    };
    decoded.validate()?;
    Ok(decoded)
}
impl PreviewRequests {
    fn decode_next(
        &self,
        db: &mut Connection,
        s: &EditorSession,
        receipt: &SessionReceipt,
        engine: &Path,
        step: impl FnMut(&str) -> Result<(), String>,
    ) -> Result<Option<DecodedPreviewInputs>, String> {
        let Some(request) = self.start_next(db, s, receipt)? else {
            return Ok(None);
        };
        let result = decode_preview_inputs(db, s, &request, engine, step);
        self.complete(
            db,
            s,
            receipt,
            &request,
            result,
            DecodedPreviewInputs::validate,
        )
    }
    fn decoded_current(
        &self,
        db: &mut Connection,
        s: &EditorSession,
        receipt: &SessionReceipt,
        decoded: &DecodedPreviewInputs,
    ) -> Result<bool, String> {
        let state = self.state.lock().map_err(|e| e.to_string())?;
        if state.closed
            || decoded.request.owner != self.id
            || decoded.request.generation != state.generation
        {
            return Ok(false);
        }
        decoded.validate()?;
        decoded.request.verify(db, s, receipt)?;
        Ok(true)
    }
}
