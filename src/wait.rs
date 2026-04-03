//! Wait for UI conditions by polling the AX tree.
//!
//! Note: Ref-based waits (--ref, --gone) use DFS traversal-order ref numbers
//! which can shift when the UI changes. For the most reliable waits, prefer
//! --text which matches by content rather than position.

use crate::ax;
use crate::system;
use std::time::{Duration, Instant};

const POLL_INTERVAL_MS: u64 = 500;

pub enum Condition {
    Text(String),
    Ref(usize),
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
    let start = Instant::now();
    let deadline = start + Duration::from_millis(timeout_ms);

    // Resolve target once to prevent drift on focus changes
    let (pid, name) = system::resolve_target_app(app)?;

    // For --gone with low limits, auto-increase to reduce false positives
    let effective_limit = match condition {
        Condition::Gone(ref_id) if *ref_id > limit => *ref_id + 50,
        _ => limit,
    };

    loop {
        let snap = ax::snapshot(pid, &name, effective_limit);

        if !snap.ok {
            return Err(snap.error.unwrap_or_else(|| "snapshot failed".into()));
        }

        let met = match condition {
            Condition::Text(text) => snap.elements.iter().any(|el| {
                el.title.as_deref().is_some_and(|t| t.contains(text.as_str()))
                    || el.value.as_deref().is_some_and(|v| v.contains(text.as_str()))
            }),
            Condition::Ref(ref_id) => snap.elements.iter().any(|el| el.ref_id == *ref_id),
            Condition::Gone(ref_id) => {
                if snap.truncated {
                    // Can't confirm element is gone when snapshot is truncated
                    false
                } else {
                    !snap.elements.iter().any(|el| el.ref_id == *ref_id)
                }
            }
        };

        let elapsed = start.elapsed().as_millis() as u64;

        if met {
            return Ok(WaitResult { met: true, elapsed_ms: elapsed, snapshot: snap });
        }

        if Instant::now() >= deadline {
            return Ok(WaitResult { met: false, elapsed_ms: elapsed, snapshot: snap });
        }

        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }
}
