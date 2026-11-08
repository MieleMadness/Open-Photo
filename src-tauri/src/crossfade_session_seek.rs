// Session authorization stays separate from the shared production clock.
use crate::crossfade_layer_clock::Span as SeekSpan;
#[cfg(test)]
use crate::sequence_frame_seek::{linear_seek_weight, SeekWeight};
use crate::sequence_frame_seek::{SeekAudioOrigin, SeekPoint};
#[derive(Debug, PartialEq, Eq)]
struct SessionSeek {
    session: SessionReceipt,
    project_revision: u64,
    document_hash: String,
    frame: crate::sequence_frame_seek::FrameSeek,
}
impl std::ops::Deref for SessionSeek {
    type Target = crate::sequence_frame_seek::FrameSeek;
    fn deref(&self) -> &Self::Target {
        &self.frame
    }
}
fn seek_verified_manifest(
    s: &EditorSession,
    m: &LayerManifest,
    point: SeekPoint,
) -> Result<SessionSeek, String> {
    Ok(SessionSeek {
        session: s.receipt.clone(),
        project_revision: m.project_revision,
        document_hash: m.document_hash.clone(),
        frame: crate::sequence_frame_seek::seek(&m.timing, point)?,
    })
}
fn seek_session(
    db: &mut Connection,
    s: &EditorSession,
    expected: &SessionReceipt,
    point: SeekPoint,
) -> Result<SessionSeek, String> {
    if expected != &s.receipt {
        return Err("Session changed before seeking; reload its receipt".into());
    }
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let manifest = load_layer_manifest_in(&tx, &s.root, &s.project_id, &s.receipt.current_id)?;
    let position = seek_verified_manifest(s, &manifest, point)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(position)
}
