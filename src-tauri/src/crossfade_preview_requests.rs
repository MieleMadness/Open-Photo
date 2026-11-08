// cfg(test) only. Bounded request lifecycle, not a decoder/executor or UI route.
use std::sync::{
    atomic::{AtomicBool as PreviewCancel, Ordering as PreviewOrder},
    Arc as PreviewArc, Mutex as PreviewMutex,
};
const PREVIEW_RESULT_LIMIT: usize = 8 * 1024 * 1024;
#[derive(Debug)]
struct PreviewRequest {
    owner: String,
    generation: u64,
    root: String,
    project: String,
    position: PreviewArc<SessionSeek>,
    cancel: PreviewCancel,
}
impl PreviewRequest {
    fn cancelled(&self) -> bool {
        self.cancel.load(PreviewOrder::SeqCst)
    }
    fn stop(&self) {
        self.cancel.store(true, PreviewOrder::SeqCst);
    }
    fn verify(
        &self,
        db: &mut Connection,
        s: &EditorSession,
        receipt: &SessionReceipt,
    ) -> Result<(), String> {
        if s.root != self.root || s.project_id != self.project || receipt != &self.position.session
        {
            return Err("Preview request belongs to another session selection".into());
        }
        let fresh = seek_session(
            db,
            s,
            receipt,
            SeekPoint::Frame(self.position.project_frame),
        )?;
        if fresh != *self.position {
            return Err("Preview project/seek changed; discard the result".into());
        }
        Ok(())
    }
}
#[derive(Debug)]
struct PreviewRequestState {
    generation: u64,
    closed: bool,
    active: Option<PreviewArc<PreviewRequest>>,
    pending: Option<PreviewArc<PreviewRequest>>,
}
impl PreviewRequestState {
    fn invalidate(&mut self) {
        if let Some(active) = &self.active {
            active.stop();
        }
        if let Some(pending) = self.pending.take() {
            pending.stop();
        }
    }
    fn advance(&mut self) -> Result<u64, String> {
        self.invalidate();
        match self.generation.checked_add(1).filter(|n| *n <= JS_SAFE) {
            Some(next) => {
                self.generation = next;
                Ok(next)
            }
            None => {
                self.closed = true;
                Err("Preview request generation exhausted; close this coordinator".into())
            }
        }
    }
}
#[derive(Debug)]
struct PreviewRequests {
    id: String,
    state: PreviewMutex<PreviewRequestState>,
}
#[derive(Debug)]
struct DeliveredPreview {
    owner: String,
    generation: u64,
    root: String,
    project: String,
    position: PreviewArc<SessionSeek>,
    bytes: Vec<u8>,
}
impl PreviewRequests {
    fn new() -> Self {
        Self {
            id: video_project::id(),
            state: PreviewMutex::new(PreviewRequestState {
                generation: 0,
                closed: false,
                active: None,
                pending: None,
            }),
        }
    }
    fn submit(
        &self,
        db: &mut Connection,
        s: &EditorSession,
        receipt: &SessionReceipt,
        point: SeekPoint,
    ) -> Result<PreviewArc<PreviewRequest>, String> {
        // Serialize admission and completion while validating metadata. Workers
        // poll their atomic cancellation token without needing this mutex.
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        if state.closed {
            return Err("Preview coordinator is closed".into());
        }
        let position = match seek_session(db, s, receipt, point) {
            Ok(position) => position,
            Err(error) => {
                state.advance()?;
                return Err(error);
            }
        };
        for request in [state.pending.as_ref(), state.active.as_ref()]
            .into_iter()
            .flatten()
        {
            if !request.cancelled()
                && request.root == s.root
                && request.project == s.project_id
                && *request.position == position
            {
                return Ok(request.clone());
            }
        }
        let generation = state.advance()?;
        let request = PreviewArc::new(PreviewRequest {
            owner: self.id.clone(),
            generation,
            root: s.root.clone(),
            project: s.project_id.clone(),
            position: PreviewArc::new(position),
            cancel: PreviewCancel::new(false),
        });
        state.pending = Some(request.clone());
        Ok(request)
    }
    fn start_next(
        &self,
        db: &mut Connection,
        s: &EditorSession,
        receipt: &SessionReceipt,
    ) -> Result<Option<PreviewArc<PreviewRequest>>, String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        if state.closed || state.active.is_some() {
            return Ok(None);
        }
        let Some(request) = state.pending.take() else {
            return Ok(None);
        };
        if let Err(error) = request.verify(db, s, receipt) {
            request.stop();
            state.advance()?;
            return Err(error);
        }
        state.active = Some(request.clone());
        Ok(Some(request))
    }
    fn cancel(&self, request: &PreviewArc<PreviewRequest>) -> Result<bool, String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        if request.owner != self.id {
            return Ok(false);
        }
        if state
            .pending
            .as_ref()
            .is_some_and(|p| PreviewArc::ptr_eq(p, request))
        {
            request.stop();
            state.pending = None;
            return Ok(true);
        }
        if state
            .active
            .as_ref()
            .is_some_and(|p| PreviewArc::ptr_eq(p, request))
        {
            request.stop();
            return Ok(true);
        }
        Ok(false)
    }
    fn finish(
        &self,
        db: &mut Connection,
        s: &EditorSession,
        receipt: &SessionReceipt,
        request: &PreviewArc<PreviewRequest>,
        result: Result<Vec<u8>, String>,
    ) -> Result<Option<DeliveredPreview>, String> {
        let result = self.complete(db, s, receipt, request, result, |bytes| {
            if bytes.is_empty() || bytes.len() > PREVIEW_RESULT_LIMIT {
                Err("Preview result payload is empty or exceeds its limit".into())
            } else {
                Ok(())
            }
        })?;
        Ok(result.map(|bytes| DeliveredPreview {
            owner: self.id.clone(),
            generation: request.generation,
            root: request.root.clone(),
            project: request.project.clone(),
            position: request.position.clone(),
            bytes,
        }))
    }
    fn complete<T>(
        &self,
        db: &mut Connection,
        s: &EditorSession,
        receipt: &SessionReceipt,
        request: &PreviewArc<PreviewRequest>,
        result: Result<T, String>,
        validate: impl FnOnce(&T) -> Result<(), String>,
    ) -> Result<Option<T>, String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        if request.owner != self.id
            || !state
                .active
                .as_ref()
                .is_some_and(|p| PreviewArc::ptr_eq(p, request))
        {
            return Err("Completion is not the active preview worker".into());
        }
        // This is the worker's acknowledgement, including cancellation/error.
        // Never release a still-running worker's slot merely on cancellation.
        state.active = None;
        let obsolete =
            state.closed || request.cancelled() || request.generation != state.generation;
        request.stop();
        if obsolete {
            return Ok(None);
        }
        request.verify(db, s, receipt)?;
        let result = result?;
        validate(&result)?;
        Ok(Some(result))
    }
    fn close(&self) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        state.closed = true;
        state.invalidate();
        Ok(())
    }
    // A consumer may run after finish returns (for example on another event
    // loop). Recheck immediately before handing its opaque payload to a view.
    fn still_current(
        &self,
        db: &mut Connection,
        s: &EditorSession,
        receipt: &SessionReceipt,
        delivered: &DeliveredPreview,
    ) -> Result<bool, String> {
        let state = self.state.lock().map_err(|e| e.to_string())?;
        if state.closed
            || delivered.owner != self.id
            || delivered.generation != state.generation
            || delivered.root != s.root
            || delivered.project != s.project_id
        {
            return Ok(false);
        }
        if receipt != &delivered.position.session {
            return Ok(false);
        }
        let fresh = seek_session(
            db,
            s,
            receipt,
            SeekPoint::Frame(delivered.position.project_frame),
        )?;
        Ok(fresh == *delivered.position)
    }
}
impl Drop for PreviewRequests {
    fn drop(&mut self) {
        // A poisoned state still owns live cancellation tokens. Drop must stop
        // them as well; it does not claim to join or reap a real media process.
        match self.state.get_mut() {
            Ok(state) => state.invalidate(),
            Err(poisoned) => poisoned.into_inner().invalidate(),
        }
    }
}
