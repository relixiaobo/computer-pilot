//! Optional presentation for agents. Full observations stay in the Broker;
//! ordinary JSON and error/recovery details keep their existing contract.
use serde_json::Value;

pub fn apply(value: &mut Value) {
    // Observations have the same presentation on success and recovery. Keep
    // the error envelope and other diagnostics intact.
    if let Some(snapshot) = value.get_mut("snapshot") {
        compact_snapshot(snapshot);
    }
    if let Some(snapshot) = value
        .get_mut("diagnostics")
        .and_then(|diagnostics| diagnostics.get_mut("snapshot"))
    {
        compact_snapshot(snapshot);
    }
    if value.get("ok") != Some(&Value::Bool(true)) {
        return;
    }
    // Recovery and non-UI command output are already purpose-specific.
    if value.get("elements").is_none()
        && value.get("snapshot").is_none()
        && value.get("method").is_none()
    {
        return;
    }
    let Some(object) = value.as_object_mut() else {
        return;
    };
    for key in [
        "schema_version",
        "client_key",
        "bundle_id",
        "window_id",
        "command_id",
        "command_status",
        "settle_ms",
    ] {
        object.remove(key);
    }
    // Receipts echo arguments already supplied by the caller. Keep the route
    // and observed effect; full field contents remain in observations.
    if object.contains_key("method") {
        object.remove("app");
        if object.remove("ref").is_some() {
            object.remove("x");
            object.remove("y");
        }
    }
    match object.get("method").and_then(Value::as_str) {
        Some("ax-set-value") => {
            object.remove("value");
        }
        Some("ax-perform") => {
            object.remove("action");
        }
        Some("unicode-pid" | "unicode-global" | "paste-pid" | "paste-global") => {
            object.remove("text");
        }
        Some("key-pid" | "key-global") => {
            object.remove("combo");
        }
        _ => {}
    }
    compact_snapshot(value);
}

fn compact_snapshot(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    for key in ["displays", "window_frame", "limit"] {
        object.remove(key);
    }
    for key in ["truncated", "depth_limited"] {
        if object.get(key) == Some(&Value::Bool(false)) {
            object.remove(key);
        }
    }
    if let Some(elements) = object.get_mut("elements").and_then(Value::as_array_mut) {
        for element in elements {
            compact_element(element);
        }
    }
    for key in ["focused", "modal"] {
        if let Some(element) = object.get_mut(key) {
            compact_element(element);
        }
    }
}

fn compact_element(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        for key in ["axPath", "x", "y", "width", "height"] {
            object.remove(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_receipts_keep_dispatch_confidence_and_effect_advice() {
        for (mut result, echo) in [
            (
                serde_json::json!({
                    "ok": true, "method": "ax-set-value", "dispatched": true,
                    "ax_value_written": true, "confidence": "low",
                    "controlled_editor_risk": true, "value": "Draft",
                    "effect_advice": "AXValue does not prove input handlers ran"
                }),
                "value",
            ),
            (
                serde_json::json!({
                    "ok": true, "method": "ax-perform", "dispatched": true,
                    "effect_verified": null, "confidence": "medium", "action": "AXPress",
                    "effect_advice": "AX accepted the action; observe its effect"
                }),
                "action",
            ),
            (
                serde_json::json!({
                    "ok": true, "method": "key-pid", "dispatched": true,
                    "effect_verified": null, "confidence": "medium", "combo": "escape",
                    "effect_advice": "No generic observable effect; inspect before retry"
                }),
                "combo",
            ),
        ] {
            let mut expected = result.clone();
            expected.as_object_mut().unwrap().remove(echo);
            apply(&mut result);
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn partial_and_unverified_results_keep_their_recovery_evidence() {
        let output = serde_json::json!({
            "ok": true, "method": "unicode-pid", "effect_verified": false,
            "clipboard_hint": "clipboard changed while pasting",
            "snapshot_error": "AX read failed", "verify_advice": "inspect before retry",
            "snapshot": {"truncated": true, "truncation_hint": "use --limit 200",
                         "elements": [{"ref": 1, "role": "textfield", "value": "中文😀"}]}
        });
        let mut result = output.clone();
        apply(&mut result);
        assert_eq!(result, output);
        let error = r#"{"ok":false,"code":"unknown_outcome","command_id":"command:1","diagnostics":{"dispatched":true}}"#;
        let mut result: Value = serde_json::from_str(error).unwrap();
        let original = result.clone();
        apply(&mut result);
        assert_eq!(result, original);
    }
}
