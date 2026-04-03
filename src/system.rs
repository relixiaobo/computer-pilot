//! macOS system integration — app resolution, System Events, permissions.

use crate::key;
use std::process::{Command, Stdio};

// ── Permissions ─────────────────────────────────────────────────────────────

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> u8;
}

pub fn check_accessibility() -> bool {
    unsafe { AXIsProcessTrusted() != 0 }
}

pub fn check_screen_recording() -> bool {
    unsafe { CGPreflightScreenCaptureAccess() != 0 }
}

// ── App resolution ──────────────────────────────────────────────────────────

pub fn resolve_target_app(name: &Option<String>) -> Result<(i32, String), String> {
    let script = match name {
        Some(n) => {
            let escaped = serde_json::to_string(n)
                .map_err(|e| format!("failed to encode app name: {e}"))?;
            format!(
                r#"
ObjC.import("AppKit");
var nameToFind = {escaped};
var apps = ObjC.unwrap($.NSWorkspace.sharedWorkspace.runningApplications);
var target = apps.find(function(a) {{ return a.localizedName && ObjC.unwrap(a.localizedName) === nameToFind; }});
if (target) {{
    JSON.stringify({{pid: Number(target.processIdentifier), name: ObjC.unwrap(target.localizedName)}});
}} else {{
    JSON.stringify({{error: "app not found: " + nameToFind}});
}}
"#
            )
        }
        None => r#"
ObjC.import("AppKit");
var app = $.NSWorkspace.sharedWorkspace.frontmostApplication;
JSON.stringify({pid: Number(app.processIdentifier), name: ObjC.unwrap(app.localizedName)});
"#
        .to_string(),
    };

    let stdout = run_jxa(&script)?;
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("failed to parse app info: {e}"))?;

    if let Some(err) = parsed.get("error").and_then(|v| v.as_str()) {
        return Err(err.to_string());
    }

    let pid = parsed.get("pid").and_then(|v| v.as_i64()).ok_or("missing pid")? as i32;
    let app = parsed
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();
    Ok((pid, app))
}

pub fn list_apps() -> Result<String, String> {
    let script = r#"
ObjC.import("AppKit");
ObjC.import("Foundation");
function hasSdef(bundlePath) {
  var fileManager = $.NSFileManager.defaultManager;
  var resourcesPath = $(bundlePath + "/Contents/Resources");
  var entries = fileManager.contentsOfDirectoryAtPathError(resourcesPath, null);
  if (!entries) return false;
  var items = ObjC.unwrap(entries);
  return items.some(function(entry) { return String(ObjC.unwrap(entry)).endsWith(".sdef"); });
}
var ws = $.NSWorkspace.sharedWorkspace;
var frontPid = Number(ws.frontmostApplication.processIdentifier);
var apps = ObjC.unwrap(ws.runningApplications)
  .filter(function(app) { return Number(app.activationPolicy) === 0 && app.localizedName; })
  .map(function(app) {
    var bundlePath = app.bundleURL ? ObjC.unwrap(app.bundleURL.path) : null;
    return {
      name: ObjC.unwrap(app.localizedName),
      pid: Number(app.processIdentifier),
      active: Number(app.processIdentifier) === frontPid,
      scriptable: bundlePath ? hasSdef(bundlePath) : false
    };
  })
  .sort(function(left, right) { return left.name.localeCompare(right.name); });
JSON.stringify({ apps: apps });
"#;
    run_jxa(script)
}

// ── AppleScript string escaping ──────────────────────────────────────────────

/// Escape a string for safe embedding in an AppleScript double-quoted literal.
/// Handles backslash, double-quote, and control characters.
fn applescript_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {} // strip other control chars
            c => out.push(c),
        }
    }
    out
}

// ── System Events (type text / send key to specific app) ────────────────────

pub fn type_text(text: &str, app: Option<&str>) -> Result<(), String> {
    let escaped = applescript_escape(text);
    let script = if let Some(app_name) = app {
        let escaped_app = applescript_escape(app_name);
        format!(
            "tell application \"{escaped_app}\" to activate\ndelay 0.3\n\
             tell application \"System Events\" to keystroke \"{escaped}\""
        )
    } else {
        format!("tell application \"System Events\" to keystroke \"{escaped}\"")
    };
    run_applescript(&script)
}

pub fn send_key(combo: &str, app: &str) -> Result<(), String> {
    let parts: Vec<&str> = combo.split('+').collect();
    if parts.is_empty() {
        return Err("empty key combo".into());
    }

    let key_name = parts.last().unwrap();
    let modifier_names = &parts[..parts.len() - 1];

    // Character-based `keystroke` for printable chars (keyboard-layout safe).
    // `key code` only for special keys (enter, escape, arrows, etc.).
    let lower = key_name.to_lowercase();
    let is_printable = lower.len() == 1
        && lower
            .chars()
            .next()
            .map(|c| c.is_ascii_alphanumeric() || ".,;'/\\[]=-`".contains(c))
            .unwrap_or(false);

    let key_clause = if is_printable {
        let escaped = lower.replace('\\', "\\\\").replace('"', "\\\"");
        format!("keystroke \"{escaped}\"")
    } else {
        let keycode = key::resolve_keycode(key_name)?;
        format!("key code {keycode}")
    };

    let mut modifiers = Vec::new();
    for name in modifier_names {
        modifiers.push(match name.to_lowercase().as_str() {
            "cmd" | "command" => "command down",
            "shift" => "shift down",
            "ctrl" | "control" => "control down",
            "alt" | "option" | "opt" => "option down",
            other => return Err(format!("unknown modifier: {other}")),
        });
    }

    let using_clause = if modifiers.is_empty() {
        String::new()
    } else if modifiers.len() == 1 {
        format!(" using {}", modifiers[0])
    } else {
        format!(" using {{{}}}", modifiers.join(", "))
    };

    let escaped_app = applescript_escape(app);
    let script = format!(
        "tell application \"{escaped_app}\" to activate\ndelay 0.3\n\
         tell application \"System Events\" to {key_clause}{using_clause}"
    );
    run_applescript(&script)
}

// ── Shell helpers ───────────────────────────────────────────────────────────

fn run_jxa(script: &str) -> Result<String, String> {
    let output = Command::new("osascript")
        .args(["-l", "JavaScript", "-e", script])
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to run osascript: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "osascript failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    String::from_utf8(output.stdout)
        .map(|s| s.trim().to_string())
        .map_err(|e| format!("osascript returned non-utf8: {e}"))
}

fn run_applescript(script: &str) -> Result<(), String> {
    let output = Command::new("osascript")
        .args(["-e", script])
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .map_err(|e| format!("osascript failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "osascript failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}
