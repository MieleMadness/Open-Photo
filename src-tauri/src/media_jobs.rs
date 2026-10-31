//! A single background image lane bounds simultaneous editor allocations.
use std::sync::{atomic::AtomicU64, Arc};
use tokio::sync::Semaphore;

pub struct ImageJobs {
    pub gate: Arc<Semaphore>,
    pub preview_generation: Arc<AtomicU64>,
}

impl Default for ImageJobs {
    fn default() -> Self {
        Self {
            gate: Arc::new(Semaphore::new(1)),
            preview_generation: Arc::new(AtomicU64::new(0)),
        }
    }
}
