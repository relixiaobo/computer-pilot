//! macOS system integration — app resolution, System Events, permissions.
//! All scripting uses AppleScript (no JXA). Sdef parsing is in sdef.rs (Rust native).

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

pub fn check_automation() -> bool {
    // Probe: try a benign System Events query (short timeout)
    run_applescript_capture(
        r#"tell application "System Events" to get name"#, 5, false
    ).is_ok()
}

// ── App resolution ──────────────────────────────────────────────────────────

pub fn resolve_target_app(name: &Option<String>) -> Result<(i32, String), String> {
    // Use tab as delimiter — no app name contains a tab character
    let script = match name {
        Some(n) => {
            let escaped = applescript_escape(n);
            format!(
                "tell application \"System Events\"\n\
                    set p to first process whose name is \"{escaped}\"\n\
                    return ((unix id of p) as text) & tab & (name of p)\n\
                end tell"
            )
        }
        None => {
            "tell application \"System Events\"\n\
                set p to first process whose frontmost is true\n\
                return ((unix id of p) as text) & tab & (name of p)\n\
            end tell".to_string()
        }
    };

    let stdout = run_applescript_capture(&script, 10, false)?;
    // Output format: "PID\tName"
    let parts: Vec<&str> = stdout.splitn(2, '\t').collect();
    if parts.len() != 2 {
        return Err(format!("unexpected output from app resolution: {stdout}"));
    }
    let pid: i32 = parts[0].trim().parse()
        .map_err(|_| format!("invalid pid: {}", parts[0]))?;
    let app_name = parts[1].trim().to_string();
    Ok((pid, app_name))
}

// ── List apps ──────────────────────────────────────────────────────────────

pub fn list_apps() -> Result<String, String> {
    // Get running GUI apps via System Events (name, pid, frontmost, bundle path)
    let script = r#"
tell application "System Events"
    set appList to ""
    set frontName to name of first process whose frontmost is true
    repeat with p in (every process whose background only is false)
        set appFile to ""
        try
            set appFile to POSIX path of (file of p as alias)
        end try
        set appList to appList & (name of p) & "	" & (unix id of p) & "	" & ((name of p) is frontName) & "	" & appFile & "
"
    end repeat
    return appList
end tell
"#;

    let raw = run_applescript_capture(script, 30, false)?;

    // Parse tab-separated output, then use Rust sdef::count_classes for scriptable detection
    let mut apps: Vec<serde_json::Value> = Vec::new();
    for line in raw.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 4 { continue; }
        let name = parts[0].trim();
        let pid: i64 = parts[1].trim().parse().unwrap_or(0);
        let active = parts[2].trim() == "true";
        let bundle_path = parts[3].trim();

        if name.is_empty() { continue; }

        // Sdef detection in Rust (no shell, no python)
        let bundle = if bundle_path.ends_with('/') {
            bundle_path.to_string()
        } else {
            format!("{bundle_path}/")
        };
        let sdef_classes = if !bundle_path.is_empty() {
            crate::sdef::count_classes(&bundle)
        } else {
            None
        };

        let mut entry = serde_json::json!({
            "name": name, "pid": pid, "active": active,
            "scriptable": sdef_classes.is_some()
        });
        if let Some(n) = sdef_classes {
            entry["sdef_classes"] = serde_json::json!(n);
        }
        apps.push(entry);
    }

    apps.sort_by(|a, b| {
        a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or(""))
    });

    Ok(serde_json::to_string(&serde_json::json!({"apps": apps}))
        .unwrap_or_else(|_| r#"{"apps":[]}"#.to_string()))
}

/// Resolve app bundle path — running apps first, then filesystem search.
pub fn resolve_app_bundle_path(app: &str) -> Result<String, String> {
    // First try running apps via System Events
    let escaped = applescript_escape(app);
    let script = format!(
        "tell application \"System Events\"\n\
            set p to first process whose name is \"{escaped}\"\n\
            return POSIX path of (file of p as alias)\n\
        end tell"
    );
    if let Ok(path) = run_applescript_capture(&script, 10, false) {
        if !path.is_empty() {
            return Ok(if path.ends_with('/') { path } else { format!("{path}/") });
        }
    }

    // Fallback: search common locations in Rust (no shell injection risk)
    let search_dirs = [
        "/Applications",
        "/System/Applications",
        "/System/Library/CoreServices",
        "/Applications/Utilities",
        "/System/Applications/Utilities",
    ];
    let target = format!("{app}.app");
    for dir in &search_dirs {
        let candidate = format!("{dir}/{target}/");
        if std::path::Path::new(&candidate).is_dir() {
            return Ok(candidate);
        }
    }
    // Also check ~/Applications
    if let Ok(home) = std::env::var("HOME") {
        let candidate = format!("{home}/Applications/{target}/");
        if std::path::Path::new(&candidate).is_dir() {
            return Ok(candidate);
        }
    }

    Err(format!("app not found: {app}"))
}

// ── AppleScript string escaping ──────────────────────────────────────────────

/// Escape a string for safe embedding in an AppleScript double-quoted literal.
fn applescript_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

// ── System Events (type text / send key to specific app) ────────────────────

pub fn type_text(text: &str, app: Option<&str>) -> Result<(), String> {
    // Save current clipboard
    let prev_clip = Command::new("pbpaste")
        .stdin(Stdio::null()).stderr(Stdio::null())
        .output().ok()
        .and_then(|o| if o.status.success() { Some(o.stdout) } else { None });

    // Write text to clipboard via pbcopy (handles any Unicode, newlines, etc.)
    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped()).stderr(Stdio::null())
        .spawn().map_err(|e| format!("pbcopy failed: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(text.as_bytes())
            .map_err(|e| format!("failed to write to pbcopy: {e}"))?;
    }
    child.wait().map_err(|e| format!("pbcopy failed: {e}"))?;

    // Activate target app and paste
    if let Some(app_name) = app {
        let escaped_app = applescript_escape(app_name);
        run_applescript(&format!(
            "tell application \"{escaped_app}\" to activate\ndelay 0.3"
        ))?;
    }
    run_applescript(
        "tell application \"System Events\" to keystroke \"v\" using command down"
    )?;

    // Small delay then restore clipboard
    std::thread::sleep(std::time::Duration::from_millis(100));
    if let Some(prev) = prev_clip {
        let mut child = Command::new("pbcopy")
            .stdin(Stdio::piped()).stderr(Stdio::null())
            .spawn().map_err(|e| format!("pbcopy restore failed: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(&prev);
        }
        let _ = child.wait();
    }

    Ok(())
}

pub fn send_key(combo: &str, app: &str) -> Result<(), String> {
    let parts: Vec<&str> = combo.split('+').collect();
    if parts.is_empty() {
        return Err("empty key combo".into());
    }

    let key_name = parts.last().unwrap();
    let modifier_names = &parts[..parts.len() - 1];

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

// ── Tell (AppleScript execution against an app) ─────────────────────────────

pub fn tell_app(app: &str, expr: &str, timeout_secs: u64) -> Result<String, String> {
    let escaped_app = applescript_escape(app);

    // Wrap in tell application ... end tell if not already wrapped
    let script = if expr.trim_start().starts_with("tell ") {
        expr.to_string()
    } else {
        format!("tell application \"{escaped_app}\"\n{expr}\nend tell")
    };

    // Try once; if app isn't running (-600), launch it and retry
    match run_applescript_capture(&script, timeout_secs, true) {
        Ok(result) => Ok(result),
        Err(ref e) if e.contains("(-600)") || e.contains("not running") => {
            // Launch the app via Launch Services (not AppleScript) and wait for it
            let _ = std::process::Command::new("open")
                .args(["-a", app])
                .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
                .status();
            std::thread::sleep(std::time::Duration::from_secs(2));
            run_applescript_capture(&script, timeout_secs, true)
        }
        Err(e) => Err(e),
    }
}

// ── Shell helpers ───────────────────────────────────────────────────────────

/// Run AppleScript, capture stdout, enforce timeout.
/// `structured` = true adds -ss flag (structured output for `cu tell`).
/// Multi-line scripts are passed via stdin to avoid shell quoting issues.
fn run_applescript_capture(script: &str, timeout_secs: u64, structured: bool) -> Result<String, String> {
    use std::sync::mpsc;

    let mut cmd = Command::new("osascript");
    if structured { cmd.arg("-ss"); }

    // Use stdin for multi-line scripts (avoids -e quoting issues)
    let use_stdin = script.contains('\n');
    if use_stdin {
        cmd.arg("-"); // read from stdin
    } else {
        cmd.args(["-e", script]);
    }

    let child = cmd
        .stdin(if use_stdin { Stdio::piped() } else { Stdio::null() })
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run osascript: {e}"))?;

    // Write script to stdin if multi-line
    let mut child = child;
    if use_stdin {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(script.as_bytes());
            // stdin is dropped here, closing the pipe
        }
    }

    let child_id = child.id();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = child.wait_with_output();
        let _ = tx.send(result);
    });

    let timeout = std::time::Duration::from_secs(timeout_secs);
    match rx.recv_timeout(timeout) {
        Ok(Ok(output)) => {
            if !output.status.success() {
                return Err(
                    String::from_utf8_lossy(&output.stderr).trim().to_string()
                );
            }
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        }
        Ok(Err(e)) => Err(format!("failed to read osascript output: {e}")),
        Err(_) => {
            let _ = Command::new("kill").arg("-9").arg(child_id.to_string())
                .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
                .status();
            Err(format!("osascript timed out after {timeout_secs}s"))
        }
    }
}

/// Run AppleScript, fire-and-forget (no output capture).
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
