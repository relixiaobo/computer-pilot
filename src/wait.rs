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
    /// Window count strictly greater than the baseline captured at start.
    NewWindow,
    /// `snapshot.modal` is `Some(_)` (sheet / dialog).
    Modal,
    /// `snapshot.focused.ref` differs from the baseline captured at start.
    FocusedChanged,
}

pub struct WaitResult {
    pub met: bool,
    pub elapsed_ms: u64,
    pub snapshot: ax::SnapshotResult,
}

/// Returns the focused element's ref id if present, else None.
fn focused_ref(snap: &ax::SnapshotResult) -> Option<usize> {
    snap.focused.as_ref().and_then(|f| f.ref_id)
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

    // Baselines captured on the first iteration for relative conditions.
    // Window count uses AX directly (not the elements list, which only walks
    // the focused window).
    let mut baseline_window_count: Option<usize> = None;
    let mut baseline_focused: Option<Option<usize>> = None;

    loop {
        let snap = ax::snapshot(pid, &name, effective_limit);

        if !snap.ok {
            return Err(snap.error.unwrap_or_else(|| "snapshot failed".into()));
        }

        // Capture baselines once so the first poll defines "before".
        if baseline_window_count.is_none() {
            baseline_window_count = Some(ax::window_count(pid));
        }
        if baseline_focused.is_none() {
            baseline_focused = Some(focused_ref(&snap));
        }

        let met = match condition {
            Condition::Text(text) => snap.elements.iter().any(|el| {
                el.title
                    .as_deref()
                    .is_some_and(|t| t.contains(text.as_str()))
                    || el
                        .value
                        .as_deref()
                        .is_some_and(|v| v.contains(text.as_str()))
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
            Condition::NewWindow => ax::window_count(pid) > baseline_window_count.unwrap_or(0),
            Condition::Modal => snap.modal.is_some(),
            Condition::FocusedChanged => focused_ref(&snap) != baseline_focused.unwrap_or(None),
        };

        let elapsed = start.elapsed().as_millis() as u64;

        if met {
            return Ok(WaitResult {
                met: true,
                elapsed_ms: elapsed,
                snapshot: snap,
            });
        }

        if Instant::now() >= deadline {
            return Ok(WaitResult {
                met: false,
                elapsed_ms: elapsed,
                snapshot: snap,
            });
        }

        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }
}
