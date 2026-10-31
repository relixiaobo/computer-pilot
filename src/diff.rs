//! In-memory change counts for action-effect verification. No snapshot cache.
use crate::ax::Element;
use std::collections::{HashMap, HashSet};

// Ignore ref renumbering, but preserve the existing position/role semantics.
fn id_of(el: &Element) -> (&str, i64, i64) {
    (&el.role, el.x.round() as i64, el.y.round() as i64)
}

pub struct Diff {
    pub added: usize,
    pub changed: usize,
    pub removed: usize,
}

pub fn diff(prev: &[Element], curr: &[Element]) -> Diff {
    let prev_map: HashMap<_, _> = prev.iter().map(|e| (id_of(e), e)).collect();
    let curr_ids: HashSet<_> = curr.iter().map(id_of).collect();
    let mut added = 0;
    let mut changed = 0;
    for el in curr {
        match prev_map.get(&id_of(el)) {
            None => added += 1,
            Some(before) if content_changed(before, el) => changed += 1,
            _ => {}
        }
    }
    let removed = prev
        .iter()
        .filter(|e| !curr_ids.contains(&id_of(e)))
        .count();
    Diff {
        added,
        changed,
        removed,
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
        assert_eq!(result.added, 1);
        assert_eq!(result.changed, 1);
        assert_eq!(result.removed, 1);
    }

    #[test]
    fn position_changes_are_remove_plus_add_not_content_changes() {
        let before = element(7, "button", 10.0, 10.0);
        let mut after = before.clone();
        after.x = 11.0;
        after.ref_id = 8;

        let result = diff(&[before], &[after]);
        assert_eq!(result.added, 1);
        assert_eq!(result.changed, 0);
        assert_eq!(result.removed, 1);
    }
}
