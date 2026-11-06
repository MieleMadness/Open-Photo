//! Test-only ordered normalized-clip receipt checks. A receipt must come from
//! the trusted normalizer, never from caller-controlled project metadata.
use crate::video_sequence_render::Snapshot;
use std::{collections::HashSet, path::PathBuf};

pub(crate) struct NormalizedReceipt {
    pub clip_id: String,
    pub asset_id: String,
    pub saved_source_sha256: String,
    pub part: PathBuf,
    pub part_sha256: String,
    pub frames: u64,
}

/// Reject swapped, changed or unaccounted normalized parts before invoking
/// the experimental graph. This is an integrity check for trusted receipts,
/// not proof that an untrusted caller really normalized the saved originals.
pub(crate) fn verify_ordered_receipts(
    snapshot: &Snapshot,
    receipts: &[NormalizedReceipt],
    inputs: &[PathBuf],
    expected_frames: &[u64],
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    if receipts.len() != snapshot.project.clips.len()
        || receipts.len() != inputs.len()
        || receipts.len() != expected_frames.len()
        || receipts.len() < 2
        || receipts.len() > 200
    {
        return Err(
            "Crossfade receipts must match the saved timeline and normalized inputs.".into(),
        );
    }
    let mut unique_clip_ids = HashSet::new();
    let mut unique_parts = HashSet::new();
    for (index, receipt) in receipts.iter().enumerate() {
        if cancel() {
            return Err("Crossfade normalized receipt verification cancelled.".into());
        }
        let clip = &snapshot.project.clips[index];
        let asset = snapshot
            .project
            .assets
            .iter()
            .find(|asset| asset.id == clip.asset_id)
            .ok_or("Crossfade receipt references a missing saved source.")?;
        if receipt.clip_id != clip.id
            || receipt.asset_id != asset.id
            || receipt.saved_source_sha256 != asset.sha256
            || receipt.frames != expected_frames[index]
            || receipt.part != inputs[index]
            || !receipt.part.is_absolute()
            || !unique_clip_ids.insert(&receipt.clip_id)
            || !unique_parts.insert(&receipt.part)
            || receipt.part_sha256.len() != 64
            || !receipt.part_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(
                "Crossfade normalized receipt differs from saved clip order or source.".into(),
            );
        }
        let (_, actual_hash) = crate::video_project::fingerprint(&receipt.part, &cancel)?;
        if actual_hash != receipt.part_sha256 {
            return Err("Crossfade normalized clip changed after receipt creation.".into());
        }
    }
    Ok(())
}
