use crate::ax::Element;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

const CACHE_TTL: Duration = Duration::from_secs(5 * 60);

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
    let client_key = std::env::var("COMPUTER_PILOT_INTERNAL_CLIENT_KEY")
        .or_else(|_| std::env::var("COMPUTER_PILOT_CLIENT_KEY"))
        .unwrap_or_else(|_| "computer-pilot-cli".into());
    let mut hasher = DefaultHasher::new();
    client_key.hash(&mut hasher);
    let root = crate::broker::runtime_home();
    let mut p = root
        .join("snapshot-cache")
        .join(format!("{:016x}", hasher.finish()));
    p.push(format!("{pid}.json"));
    p
}

pub fn load_previous(pid: i32) -> Option<Vec<Element>> {
    let path = cache_path(pid);
    let metadata = fs::metadata(&path).ok()?;
    let modified = metadata.modified().ok()?;
    if SystemTime::now().duration_since(modified).ok()? > CACHE_TTL {
        let _ = fs::remove_file(path);
        return None;
    }
    let data = fs::read(&path).ok()?;
    let entry: CacheEntry = serde_json::from_slice(&data).ok()?;
    Some(entry.elements)
}

pub fn save_current(pid: i32, elements: &[Element]) -> std::io::Result<()> {
    let path = cache_path(pid);
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("snapshot cache path has no parent"))?;
    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    let entry = CacheEntry {
        elements: elements.to_vec(),
    };
    let json = serde_json::to_vec(&entry)?;
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)?;
    file.write_all(&json)?;
    file.sync_all()?;
    fs::rename(temp, path)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn element(ref_id: usize, role: &str, x: f64, y: f64) -> Element {
        Element {
            ref_id,
            role: role.into(),
            title: Some(format!("title-{ref_id}")),
            value: Some(format!("value-{ref_id}")),
            x,
            y,
            width: 100.0,
            height: 20.0,
            ax_path: Some(format!("window/{role}:{ref_id}")),
        }
    }

    #[test]
    fn identity_rounds_position_but_ignores_ref_and_content() {
        let base = element(1, "button", 10.49, -10.49);
        let mut same_identity = element(99, "button", 10.1, -10.1);
        same_identity.title = Some("different".into());
        assert_eq!(id_of(&base), id_of(&same_identity));

        let x_boundary = element(1, "button", 10.5, -10.49);
        assert_ne!(id_of(&base), id_of(&x_boundary));
        let y_boundary = element(1, "button", 10.49, -10.5);
        assert_ne!(id_of(&base), id_of(&y_boundary));
        let role_change = element(1, "link", 10.49, -10.49);
        assert_ne!(id_of(&base), id_of(&role_change));
    }

    #[test]
    fn content_change_threshold_is_strictly_greater_than_half_a_point() {
        let base = element(1, "button", 10.0, 20.0);
        assert!(!content_changed(&base, &base));

        let mut changed = base.clone();
        changed.width += 0.5;
        assert!(!content_changed(&base, &changed));
        changed.width += 0.001;
        assert!(content_changed(&base, &changed));

        let mut changed = base.clone();
        changed.height -= 0.5;
        assert!(!content_changed(&base, &changed));
        changed.height -= 0.001;
        assert!(content_changed(&base, &changed));

        let mut changed = base.clone();
        changed.title = Some("renamed".into());
        assert!(content_changed(&base, &changed));
        let mut changed = base.clone();
        changed.value = None;
        assert!(content_changed(&base, &changed));

        let mut unchanged = base.clone();
        unchanged.ref_id = 42;
        unchanged.ax_path = Some("different/path".into());
        assert!(!content_changed(&base, &unchanged));
    }

    #[test]
    fn diff_classifies_added_changed_removed_and_unchanged_elements() {
        let unchanged = element(1, "button", 10.0, 10.0);
        let changed_before = element(2, "textfield", 20.0, 20.0);
        let removed = element(3, "link", 30.0, 30.0);

        let mut unchanged_after = unchanged.clone();
        unchanged_after.ref_id = 91;
        let mut changed_after = changed_before.clone();
        changed_after.ref_id = 92;
        changed_after.value = Some("edited".into());
        let added = element(4, "checkbox", 40.0, 40.0);

        let result = diff(
            &[unchanged, changed_before, removed],
            &[unchanged_after, changed_after, added],
        );
        assert_eq!(result.total, 3);
        assert_eq!(result.unchanged_count, 1);
        assert_eq!(result.added.len(), 1);
        assert_eq!(result.added[0].ref_id, 4);
        assert_eq!(result.changed.len(), 1);
        assert_eq!(result.changed[0].ref_id, 92);
        assert_eq!(result.removed, [3]);
    }

    #[test]
    fn position_changes_are_remove_plus_add_not_content_changes() {
        let before = element(7, "button", 10.0, 10.0);
        let mut after = before.clone();
        after.x = 11.0;
        after.ref_id = 8;

        let result = diff(&[before], &[after]);
        assert_eq!(result.added.len(), 1);
        assert!(result.changed.is_empty());
        assert_eq!(result.removed, [7]);
        assert_eq!(result.unchanged_count, 0);
    }
}
