//! macOS system integration — app resolution, System Events, permissions.
//! All scripting uses AppleScript (no JXA). Sdef parsing is in sdef.rs (Rust native).

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
    run_applescript_capture(r#"tell application "System Events" to get name"#, 5, false).is_ok()
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
        None => "tell application \"System Events\"\n\
                set p to first process whose frontmost is true\n\
                return ((unix id of p) as text) & tab & (name of p)\n\
            end tell"
            .to_string(),
    };

    let stdout = run_applescript_capture(&script, 10, false)?;
    // Output format: "PID\tName"
    let parts: Vec<&str> = stdout.splitn(2, '\t').collect();
    if parts.len() != 2 {
        return Err(format!("unexpected output from app resolution: {stdout}"));
    }
    let pid: i32 = parts[0]
        .trim()
        .parse()
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

    // 60s timeout: enumerating + resolving bundle paths via System Events scales
    // with the number of running GUI apps. Machines with 20+ apps can hit ~30s
    // under load; 60s gives reliable headroom without masking real hangs.
    let raw = run_applescript_capture(script, 60, false)?;

    // Parse tab-separated output, then use Rust sdef::count_classes for scriptable detection
    let mut apps: Vec<serde_json::Value> = Vec::new();
    for line in raw.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 4 {
            continue;
        }
        let name = parts[0].trim();
        let pid: i64 = parts[1].trim().parse().unwrap_or(0);
        let active = parts[2].trim() == "true";
        let bundle_path = parts[3].trim();

        if name.is_empty() {
            continue;
        }

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
        a["name"]
            .as_str()
            .unwrap_or("")
            .cmp(b["name"].as_str().unwrap_or(""))
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
    if let Ok(path) = run_applescript_capture(&script, 10, false)
        && !path.is_empty()
    {
        return Ok(if path.ends_with('/') {
            path
        } else {
            format!("{path}/")
        });
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

// ── Window management (via System Events) ──────────────────────────────────

#[derive(serde::Serialize)]
pub struct WindowInfo {
    pub app: String,
    pub index: usize,
    pub title: String,
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
    pub minimized: bool,
    pub focused: bool,
}

pub fn list_windows(app: Option<&str>) -> Result<Vec<WindowInfo>, String> {
    // Build process scope. Use a list (single-element when app is given)
    // so the same `repeat with p in procList` body works for both cases.
    let process_setup = match app {
        Some(name) => {
            let escaped = applescript_escape(name);
            format!("set procList to {{process \"{escaped}\"}}")
        }
        None => "set procList to (every process whose background only is false)".to_string(),
    };

    // Use ASCII control chars as delimiters: US (unit, 0x1f) for fields, RS (record, 0x1e) for rows.
    // These cannot appear in macOS UI text — they're designed exactly for this purpose.
    let script = format!(
        r#"set US to character id 31
set RS to character id 30
tell application "System Events"
    {process_setup}
    set output to ""
    repeat with p in procList
        try
            set procName to name of p
            set winIdx to 0
            repeat with w in windows of p
                set winIdx to winIdx + 1
                try
                    set winTitle to name of w
                    if winTitle is missing value then set winTitle to ""
                    set winPos to position of w
                    set winSize to size of w
                    set isMin to false
                    try
                        set isMin to value of attribute "AXMinimized" of w
                    end try
                    set isFoc to false
                    try
                        set isFoc to value of attribute "AXMain" of w
                    end try
                    set output to output & procName & US & winIdx & US & winTitle & US & (item 1 of winPos) & US & (item 2 of winPos) & US & (item 1 of winSize) & US & (item 2 of winSize) & US & isMin & US & isFoc & RS
                end try
            end repeat
        end try
    end repeat
    return output
end tell"#
    );

    let raw = run_applescript_capture(&script, 15, false)?;
    let mut windows = Vec::new();
    for record in raw.split('\u{1e}') {
        let parts: Vec<&str> = record.split('\u{1f}').collect();
        if parts.len() < 9 {
            continue;
        }
        windows.push(WindowInfo {
            app: parts[0].trim().to_string(),
            index: parts[1].trim().parse().unwrap_or(0),
            title: parts[2].to_string(), // don't trim — title may have leading/trailing spaces
            x: parts[3].trim().parse().unwrap_or(0),
            y: parts[4].trim().parse().unwrap_or(0),
            width: parts[5].trim().parse().unwrap_or(0),
            height: parts[6].trim().parse().unwrap_or(0),
            minimized: parts[7].trim() == "true",
            focused: parts[8].trim() == "true",
        });
    }
    Ok(windows)
}

pub fn window_action(
    action: &str,
    app: &str,
    window_idx: usize,
    arg1: Option<i64>,
    arg2: Option<i64>,
) -> Result<(), String> {
    let escaped = applescript_escape(app);
    let target = format!("window {window_idx}");

    let inner = match action {
        "move" => {
            let x = arg1.ok_or("move requires x y")?;
            let y = arg2.ok_or("move requires x y")?;
            format!("set position of {target} to {{{x}, {y}}}")
        }
        "resize" => {
            let w = arg1.ok_or("resize requires width height")?;
            let h = arg2.ok_or("resize requires width height")?;
            format!("set size of {target} to {{{w}, {h}}}")
        }
        "focus" => {
            format!("set frontmost to true\nperform action \"AXRaise\" of {target}")
        }
        "minimize" => {
            format!("set value of attribute \"AXMinimized\" of {target} to true")
        }
        "unminimize" => {
            format!("set value of attribute \"AXMinimized\" of {target} to false")
        }
        "close" => {
            format!("click (first button of {target} whose subrole is \"AXCloseButton\")")
        }
        other => {
            return Err(format!(
                "unknown window action: {other} (use: list, move, resize, focus, minimize, unminimize, close)"
            ));
        }
    };

    let script = format!(
        "tell application \"System Events\"
    tell process \"{escaped}\"
        {inner}
    end tell
end tell"
    );
    run_applescript_capture(&script, 10, false)?;
    Ok(())
}

// ── Menu (enumerate app menu bar via System Events) ─────────────────────────

#[derive(serde::Serialize)]
pub struct MenuItem {
    pub menu: String,
    pub item: String,
    pub enabled: bool,
}

pub fn list_menu(app: &str) -> Result<Vec<MenuItem>, String> {
    let escaped = applescript_escape(app);
    // Use ASCII control chars (US/RS) as delimiters — cannot appear in UI text.
    let script = format!(
        "set US to character id 31
set RS to character id 30
tell application \"System Events\"
    tell process \"{escaped}\"
        set output to \"\"
        repeat with menuBarItem in menu bar items of menu bar 1
            set menuName to name of menuBarItem
            try
                repeat with mi in menu items of menu 1 of menuBarItem
                    set itemName to name of mi
                    if itemName is not missing value then
                        set isEnabled to enabled of mi
                        set output to output & menuName & US & itemName & US & isEnabled & RS
                    end if
                end repeat
            end try
        end repeat
        return output
    end tell
end tell"
    );
    let raw = run_applescript_capture(&script, 10, false)?;
    let mut items = Vec::new();
    for record in raw.split('\u{1e}') {
        let parts: Vec<&str> = record.split('\u{1f}').collect();
        if parts.len() < 3 {
            continue;
        }
        items.push(MenuItem {
            menu: parts[0].to_string(),
            item: parts[1].to_string(),
            enabled: parts[2].trim() == "true",
        });
    }
    Ok(items)
}

// ── Defaults (read/write macOS preferences) ─────────────────────────────────

pub fn defaults_read(domain: &str, key: Option<&str>) -> Result<String, String> {
    let mut args = vec!["defaults", "read", domain];
    if let Some(k) = key {
        args.push(k);
    }
    let output = Command::new(args[0])
        .args(&args[1..])
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .map_err(|e| format!("defaults failed: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn defaults_write(domain: &str, key: &str, value_args: &[String]) -> Result<(), String> {
    let mut cmd = Command::new("defaults");
    cmd.arg("write").arg(domain).arg(key);
    for v in value_args {
        cmd.arg(v);
    }
    let output = cmd
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .map_err(|e| format!("defaults write failed: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(())
}

// ── Launch (D6) ─────────────────────────────────────────────────────────────

/// Resolve a process by bundle identifier (System Events lookup).
/// Returns `(pid, app_name)` once the process exists, error otherwise.
pub fn resolve_by_bundle_id(bundle_id: &str) -> Result<(i32, String), String> {
    let escaped = applescript_escape(bundle_id);
    let script = format!(
        "tell application \"System Events\"\n\
            set p to first process whose bundle identifier is \"{escaped}\"\n\
            return ((unix id of p) as text) & tab & (name of p)\n\
        end tell"
    );
    let stdout = run_applescript_capture(&script, 5, false)?;
    let parts: Vec<&str> = stdout.splitn(2, '\t').collect();
    if parts.len() != 2 {
        return Err(format!("unexpected output: {stdout}"));
    }
    let pid: i32 = parts[0]
        .trim()
        .parse()
        .map_err(|_| format!("invalid pid: {}", parts[0]))?;
    Ok((pid, parts[1].trim().to_string()))
}

/// Launch an app by name or bundle identifier via Launch Services.
/// Heuristic: an `id` argument with a `.` is treated as a bundle id (`open -b`),
/// otherwise as an app name (`open -a`). Returns immediately — the caller is
/// responsible for waiting on readiness if needed.
pub fn launch_app(id: &str) -> Result<(), String> {
    let flag = if id.contains('.') && !id.contains(' ') {
        "-b"
    } else {
        "-a"
    };
    let status = Command::new("open")
        .args([flag, id])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to spawn `open`: {e}"))?;
    if !status.status.success() {
        let stderr = String::from_utf8_lossy(&status.stderr);
        let msg = stderr.trim();
        let detail = if msg.is_empty() { "not found" } else { msg };
        return Err(format!("open {flag} {id} failed: {detail}"));
    }
    Ok(())
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
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
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
fn run_applescript_capture(
    script: &str,
    timeout_secs: u64,
    structured: bool,
) -> Result<String, String> {
    use std::sync::mpsc;

    let mut cmd = Command::new("osascript");
    if structured {
        cmd.arg("-ss");
    }

    // Use stdin for multi-line scripts (avoids -e quoting issues)
    let use_stdin = script.contains('\n');
    if use_stdin {
        cmd.arg("-"); // read from stdin
    } else {
        cmd.args(["-e", script]);
    }

    let child = cmd
        .stdin(if use_stdin {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run osascript: {e}"))?;

    // Write script to stdin if multi-line
    let mut child = child;
    if use_stdin && let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(script.as_bytes());
        // stdin is dropped here, closing the pipe
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
                return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
            }
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        }
        Ok(Err(e)) => Err(format!("failed to read osascript output: {e}")),
        Err(_) => {
            let _ = Command::new("kill")
                .arg("-9")
                .arg(child_id.to_string())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            Err(format!("osascript timed out after {timeout_secs}s"))
        }
    }
}
