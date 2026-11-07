// Phase 1 visual-only frame. Titles/audio are deliberately separate phase 2 work.
struct ComposedVisualPreview {
    inputs: DecodedPreviewInputs,
    frame: crate::video_sequence_render::PreviewRgbPart,
}
impl ComposedVisualPreview {
    fn validate(&self) -> Result<(), String> {
        self.inputs.validate()?;
        let first = &self.inputs.parts[0].frame;
        if self.frame.width != first.width
            || self.frame.height != first.height
            || self.frame.rgb.len() != first.rgb.len()
        {
            return Err("Composed frame geometry mismatch".into());
        }
        Ok(())
    }
}
fn compose_visual_inputs(
    db: &mut Connection,
    s: &EditorSession,
    inputs: DecodedPreviewInputs,
    mut step: impl FnMut(&str) -> Result<(), String>,
) -> Result<ComposedVisualPreview, String> {
    inputs.validate()?;
    inputs.request.verify(db, s, &s.receipt)?;
    let manifest = load_layer_manifest(db, &s.root, &s.project_id, &s.receipt.current_id)?;
    let mut buffers = Vec::new();
    for (part, seek) in inputs.parts.iter().zip(&inputs.request.position.video) {
        if inputs.request.cancelled() {
            return Err("Visual composition cancelled".into());
        }
        let clip = manifest
            .timing
            .clips
            .iter()
            .find(|clip| clip.id == seek.clip_id)
            .ok_or("Missing composition clip")?;
        let duration = clip.frames.end - clip.frames.start;
        let mut gain = 1.0_f64;
        if seek.dip_in_frames > 0 {
            gain = gain.min(seek.part_frame as f64 / seek.dip_in_frames as f64);
        }
        if seek.dip_out_frames > 0 {
            let start = duration
                .checked_sub(seek.dip_out_frames)
                .ok_or("Invalid dip interval")?;
            if seek.part_frame >= start {
                gain = gain.min((duration - seek.part_frame) as f64 / seek.dip_out_frames as f64);
            }
        }
        if !(0.0..=1.0).contains(&gain) {
            return Err("Invalid dip gain".into());
        }
        buffers.push(
            part.frame
                .rgb
                .iter()
                .map(|value| (f64::from(*value) * gain).round() as u8)
                .collect::<Vec<_>>(),
        );
        step("after_visual_part")?;
    }
    let first = &inputs.parts[0].frame;
    let rgb = if buffers.len() == 1 {
        buffers.remove(0)
    } else {
        let a = &inputs.request.position.video[0].weight;
        let b = &inputs.request.position.video[1].weight;
        if a.denominator == 0
            || a.denominator != b.denominator
            || a.numerator.checked_add(b.numerator) != Some(a.denominator)
        {
            return Err("Invalid complementary visual weights".into());
        }
        crate::crossfade_visual_compositor::blend_rgb(
            &buffers[0],
            &buffers[1],
            first.width,
            first.height,
            b.numerator as f64 / b.denominator as f64,
        )?
    };
    step("after_visual_composition")?;
    if inputs.request.cancelled() {
        return Err("Visual composition cancelled".into());
    }
    inputs.request.verify(db, s, &s.receipt)?;
    let result = ComposedVisualPreview {
        frame: crate::video_sequence_render::PreviewRgbPart {
            width: first.width,
            height: first.height,
            rgb,
        },
        inputs,
    };
    result.validate()?;
    Ok(result)
}
impl PreviewRequests {
    fn compose_next(
        &self,
        db: &mut Connection,
        s: &EditorSession,
        receipt: &SessionReceipt,
        engine: &Path,
        mut step: impl FnMut(&str) -> Result<(), String>,
    ) -> Result<Option<ComposedVisualPreview>, String> {
        let Some(request) = self.start_next(db, s, receipt)? else {
            return Ok(None);
        };
        let result = decode_preview_inputs(db, s, &request, engine, &mut step)
            .and_then(|inputs| compose_visual_inputs(db, s, inputs, &mut step));
        self.complete(
            db,
            s,
            receipt,
            &request,
            result,
            ComposedVisualPreview::validate,
        )
    }
    fn visual_current(
        &self,
        db: &mut Connection,
        s: &EditorSession,
        receipt: &SessionReceipt,
        visual: &ComposedVisualPreview,
    ) -> Result<bool, String> {
        visual.validate()?;
        self.decoded_current(db, s, receipt, &visual.inputs)
    }
}
