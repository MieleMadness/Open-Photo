//! Render previews through the shared export/edit pipeline.
use crate::{
    video_preview::PreviewAssets,
    video_project::{self, Project},
    video_render::{self, ExportSize, RenderOptions},
};
use std::path::Path;
pub(crate) fn build(
    engine: &Path,
    root: &str,
    p: &Project,
    directory: &Path,
    cancel: impl Fn() -> bool,
) -> Result<PreviewAssets, String> {
    let output = directory.join("edited-preview.mp4");
    let plan = video_render::plan(
        engine,
        root,
        p,
        &output,
        RenderOptions {
            preview: true,
            size: ExportSize::Project,
            acknowledge_first_audio: true,
        },
        &cancel,
    )?;
    video_render::encode(engine, &plan, &cancel, |_| {})?;
    let info = video_render::validate_output(engine, p, &plan, &output, &cancel)?;
    video_project::source(root, p, &cancel)?;
    Ok(PreviewAssets {
        source: output.to_string_lossy().into_owned(),
        proxy: true,
        info,
        filmstrip: vec![],
        warnings: plan.warnings,
    })
}
