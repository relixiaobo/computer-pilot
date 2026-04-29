use crate::ax::Element;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

const CACHE_DIR: &str = "/tmp/cu-snapshot-cache";

/// Identity of a UI element for diffing across snapshots.
/// (role, round(x), round(y)) — robust to ref re-numbering, sensitive to re-layout.
type ElementId = (String, i64, i64);

fn id_of(el: &Element) -> ElementId {
    (el.role.clone(), el.x.round() as i64, el.y.round() as i64)
}

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    elements: Vec<Element>,
}

fn cache_path(pid: i32) -> PathBuf {
    let mut p = PathBuf::from(CACHE_DIR);
    p.push(format!("{pid}.json"));
    p
}

pub fn load_previous(pid: i32) -> Option<Vec<Element>> {
    let path = cache_path(pid);
    let data = std::fs::read(&path).ok()?;
    let entry: CacheEntry = serde_json::from_slice(&data).ok()?;
    Some(entry.elements)
}

pub fn save_current(pid: i32, elements: &[Element]) -> std::io::Result<()> {
    std::fs::create_dir_all(CACHE_DIR)?;
    let entry = CacheEntry {
        elements: elements.to_vec(),
    };
    let json = serde_json::to_vec(&entry)?;
    std::fs::write(cache_path(pid), json)
}

#[derive(Serialize)]
pub struct Diff {
    /// Elements that exist in `curr` but not in `prev` (by identity).
    pub added: Vec<Element>,
    /// Elements with the same identity as before but different title/value/size.
    pub changed: Vec<Element>,
    /// Refs (from the previous snapshot) of elements that no longer exist.
    pub removed: Vec<usize>,
    pub unchanged_count: usize,
    pub total: usize,
}

pub fn diff(prev: &[Element], curr: &[Element]) -> Diff {
    let prev_map: HashMap<ElementId, &Element> = prev.iter().map(|e| (id_of(e), e)).collect();
    let curr_ids: std::collections::HashSet<ElementId> = curr.iter().map(id_of).collect();

    let mut added = Vec::new();
    let mut changed = Vec::new();
    let mut unchanged = 0usize;

    for el in curr {
        match prev_map.get(&id_of(el)) {
            None => added.push(el.clone()),
            Some(prev_el) => {
                if content_changed(prev_el, el) {
                    changed.push(el.clone());
                } else {
                    unchanged += 1;
                }
            }
        }
    }

    let removed: Vec<usize> = prev
        .iter()
        .filter(|e| !curr_ids.contains(&id_of(e)))
        .map(|e| e.ref_id)
        .collect();

    Diff {
        added,
        changed,
        removed,
        unchanged_count: unchanged,
        total: curr.len(),
    }
}

fn content_changed(a: &Element, b: &Element) -> bool {
    a.title != b.title
        || a.value != b.value
        || (a.width - b.width).abs() > 0.5
        || (a.height - b.height).abs() > 0.5
}

/// Compare what was at `ref_id` in the previous snapshot against what is at
/// `ref_id` in the current AX walk. Returns an advice string when the identity
/// has changed, indicating the UI shifted between snapshots and the agent's
/// ref likely points to a different element than it expected. Soft signal —
/// the action still runs; the agent reads the advice and chooses to recover.
pub fn detect_ref_drift(
    prev: &[Element],
    curr: &[Element],
    ref_id: usize,
) -> Option<String> {
    let prev_el = prev.iter().find(|e| e.ref_id == ref_id)?;
    let curr_el = curr.iter().find(|e| e.ref_id == ref_id)?;
    if id_of(prev_el) == id_of(curr_el) {
        return None;
    }
    let prev_label = prev_el
        .title
        .as_deref()
        .or(prev_el.value.as_deref())
        .unwrap_or("");
    let curr_label = curr_el
        .title
        .as_deref()
        .or(curr_el.value.as_deref())
        .unwrap_or("");
    Some(format!(
        "ref [{ref_id}] now points to a different element than the previous snapshot — was {} \"{}\" at ({:.0},{:.0}), now {} \"{}\" at ({:.0},{:.0}). UI shifted between snapshots; re-snapshot before relying on this ref.",
        prev_el.role, prev_label, prev_el.x, prev_el.y,
        curr_el.role, curr_label, curr_el.x, curr_el.y,
    ))
}
