//! Test-only bridge from a validated, saved timeline snapshot to original
//! source fingerprint expectations. Never learn new hashes during export.
use crate::{
    crossfade_integrity::{self, SourceExpectation},
    video_sequence_render::Snapshot,
};
use std::{collections::HashSet, path::PathBuf};

pub(crate) struct SourceRecord {
    path: PathBuf,
    bytes: u64,
    sha256: String,
}

pub(crate) struct SourceManifest {
    records: Vec<SourceRecord>,
}

impl SourceManifest {
    /// Resolve only sources referenced by the saved timeline. The roots are
    /// relinked source locations; hashes and lengths come from the saved asset.
    pub(crate) fn from_snapshot(
        snapshot: &Snapshot,
        cancel: impl Fn() -> bool,
    ) -> Result<Self, String> {
        snapshot.validate()?;
        let mut records = Vec::new();
        let mut used = HashSet::new();
        for clip in &snapshot.project.clips {
            if cancel() {
                return Err("Crossfade source discovery cancelled.".into());
            }
            if !used.insert(&clip.asset_id) {
                continue;
            }
            let asset = snapshot
                .project
                .assets
                .iter()
                .find(|a| a.id == clip.asset_id)
                .ok_or("Crossfade timeline references an unknown asset.")?;
            let root = snapshot
                .roots
                .get(&asset.id)
                .ok_or("Relink this crossfade timeline asset.")?;
            let path = crate::sequence_media::source(root, asset, &cancel)?;
            records.push(SourceRecord {
                path,
                bytes: asset.bytes,
                sha256: asset.sha256.clone(),
            });
        }
        for source in &snapshot.project.audio_sources {
            if !snapshot
                .project
                .audio_tracks
                .iter()
                .any(|track| track.asset_id == source.id)
            {
                continue;
            }
            if cancel() {
                return Err("Crossfade source discovery cancelled.".into());
            }
            if !used.insert(&source.id) {
                return Err("Crossfade source identifiers must be unique.".into());
            }
            let root = snapshot
                .roots
                .get(&source.id)
                .ok_or("Relink this crossfade audio source.")?;
            let candidate = std::path::Path::new(root).join(&source.relative_path);
            let path = crate::video_probe::safe_audio_path(root, &candidate.to_string_lossy())?;
            records.push(SourceRecord {
                path,
                bytes: source.bytes,
                sha256: source.sha256.clone(),
            });
        }
        if records.is_empty() || records.len() > 200 {
            return Err("Crossfade source snapshot exceeds the allowed asset count.".into());
        }
        let manifest = Self { records };
        manifest.verify(&cancel)?;
        Ok(manifest)
    }

    pub(crate) fn verify(&self, cancel: impl Fn() -> bool) -> Result<(), String> {
        let expected = self.expectations();
        crossfade_integrity::verify_source_fingerprints(&expected, cancel)
    }

    pub(crate) fn expectations(&self) -> Vec<SourceExpectation<'_>> {
        self.records
            .iter()
            .map(|record| SourceExpectation {
                path: &record.path,
                bytes: record.bytes,
                sha256: &record.sha256,
            })
            .collect()
    }
}
