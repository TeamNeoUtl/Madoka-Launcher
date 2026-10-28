use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

use crate::InstanceId;

/// One async mutex per instance. A write task holds the guard until it finishes.
#[derive(Debug, Clone, Default)]
pub struct InstanceLocks {
    slots: Arc<Mutex<HashMap<InstanceId, Arc<AsyncMutex<()>>>>>,
}

impl InstanceLocks {
    pub async fn lock(&self, id: InstanceId) -> OwnedMutexGuard<()> {
        let slot = {
            let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
            Arc::clone(slots.entry(id).or_default())
        };
        slot.lock_owned().await
    }
}
