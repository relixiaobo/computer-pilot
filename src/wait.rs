//! Wait for UI conditions by polling the AX tree.

use crate::ax;
use crate::system;
use std::time::{Duration, Instant};

const POLL_INTERVAL_MS: u64 = 500;

pub enum Condition {
    /// Wait until any element contains this text (in title or value).
    Text(String),
    /// Wait until an element with this ref exists.
    Ref(usize),
    /// Wait until an element with this ref no longer exists.
    Gone(usize),
}

pub struct WaitResult {
    pub met: bool,
    pub elapsed_ms: u64,
    pub snapshot: ax::SnapshotResult,
}

pub fn wait_for(
    condition: &Condition,
    app: &Option<String>,
    timeout_ms: u64,
    limit: usize,
) -> Result<WaitResult, String> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);

    loop {
        let (pid, name) = system::resolve_target_app(app)?;
        let snap = ax::snapshot(pid, &name, limit);

        if !snap.ok {
            return Err(snap.error.unwrap_or_else(|| "snapshot failed".into()));
        }

        let met = match condition {
            Condition::Text(text) => snap.elements.iter().any(|el| {
                el.title.as_deref().is_some_and(|t| t.contains(text.as_str()))
                    || el.value.as_deref().is_some_and(|v| v.contains(text.as_str()))
            }),
            Condition::Ref(ref_id) => snap.elements.iter().any(|el| el.ref_id == *ref_id),
            Condition::Gone(ref_id) => !snap.elements.iter().any(|el| el.ref_id == *ref_id),
        };

        let elapsed = Instant::now().duration_since(deadline - Duration::from_millis(timeout_ms));

        if met {
            return Ok(WaitResult {
                met: true,
                elapsed_ms: elapsed.as_millis() as u64,
                snapshot: snap,
            });
        }

        if Instant::now() >= deadline {
            return Ok(WaitResult {
                met: false,
                elapsed_ms: timeout_ms,
                snapshot: snap,
            });
        }

        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }
}
