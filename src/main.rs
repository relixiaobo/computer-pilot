mod ax;
mod key;
mod mouse;
mod ocr;
mod screenshot;
mod system;
mod wait;

use clap::{Parser, Subcommand};
use std::io::IsTerminal;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const POST_ACTION_DELAY_MS: u64 = 500;

// ── CLI definition ──────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "cu", version = VERSION, about = "Control your desktop from the command line")]
struct Cli {
    /// Force human-readable output
    #[arg(long)]
    human: bool,

    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Check permissions and guide setup
    Setup,
    /// Check status
    Status,
    /// List running applications
    Apps,
    /// AX tree snapshot with [ref] numbers
    Snapshot {
        /// Application name (default: frontmost)
        app: Option<String>,
        #[arg(long, default_value = "50")]
        limit: usize,
    },
    /// Type text into the focused element
    Type {
        text: String,
        #[arg(long)]
        app: Option<String>,
        #[arg(long)]
        no_snapshot: bool,
    },
    /// Send a keyboard shortcut (e.g., enter, cmd+c, cmd+shift+s)
    Key {
        combo: String,
        #[arg(long)]
        app: Option<String>,
        #[arg(long)]
        no_snapshot: bool,
    },
    /// Wait for a UI condition (text appears, element exists/gone)
    Wait {
        /// Wait for text in any element
        #[arg(long)]
        text: Option<String>,
        /// Wait for element ref to exist
        #[arg(long, name = "ref")]
        ref_id: Option<usize>,
        /// Wait for element ref to disappear
        #[arg(long)]
        gone: Option<usize>,
        #[arg(long)]
        app: Option<String>,
        /// Timeout in seconds (default: 10)
        #[arg(long, default_value = "10")]
        timeout: u64,
        #[arg(long, default_value = "200")]
        limit: usize,
    },
    /// OCR — recognize text on screen via macOS Vision framework
    Ocr {
        /// Application name (default: frontmost)
        app: Option<String>,
    },
    /// Click an element by ref or screen coordinates
    Click {
        /// Element ref number, or x coordinate
        target: String,
        /// Y coordinate (when target is x)
        y: Option<String>,
        #[arg(long)]
        app: Option<String>,
        #[arg(long, default_value = "200")]
        limit: usize,
        #[arg(long)]
        right: bool,
        #[arg(long, name = "double")]
        double_click: bool,
        /// Hold shift during click
        #[arg(long)]
        shift: bool,
        /// Hold cmd during click
        #[arg(long)]
        cmd: bool,
        /// Hold alt/option during click
        #[arg(long)]
        alt: bool,
        #[arg(long)]
        no_snapshot: bool,
    },
    /// Scroll at coordinates or current mouse position
    Scroll {
        /// Direction: up, down, left, right
        direction: String,
        /// Number of lines to scroll (default: 3)
        #[arg(default_value = "3")]
        amount: i32,
        /// X coordinate (optional, defaults to current position)
        #[arg(long)]
        x: Option<f64>,
        /// Y coordinate (optional, defaults to current position)
        #[arg(long)]
        y: Option<f64>,
    },
    /// Move mouse to coordinates (hover / trigger tooltips)
    Hover {
        x: f64,
        y: f64,
    },
    /// Drag from one position to another
    Drag {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
    /// Capture a silent screenshot (no app activation needed)
    Screenshot {
        /// Application name (default: frontmost)
        app: Option<String>,
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        full: bool,
    },
}

// ── Main ────────────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();
    let json = !cli.human && !std::io::stdout().is_terminal();

    if let Err(msg) = dispatch(cli.command, json) {
        if json {
            eprintln!("{}", serde_json::json!({"ok": false, "error": msg}));
        } else {
            eprintln!("Error: {msg}");
        }
        std::process::exit(1);
    }
}

fn dispatch(cmd: Cmd, json: bool) -> Result<(), String> {
    match cmd {
        Cmd::Setup => cmd_setup(json),
        Cmd::Status => cmd_status(json),
        Cmd::Apps => cmd_apps(json),
        Cmd::Snapshot { app, limit } => cmd_snapshot(json, app, limit),
        Cmd::Wait { text, ref_id, gone, app, timeout, limit } => {
            cmd_wait(json, text, ref_id, gone, app, timeout, limit)
        }
        Cmd::Ocr { app } => cmd_ocr(json, app),
        Cmd::Type { text, app, no_snapshot } => cmd_type(json, text, app, no_snapshot),
        Cmd::Key { combo, app, no_snapshot } => cmd_key(json, combo, app, no_snapshot),
        Cmd::Click { target, y, app, limit, right, double_click, shift, cmd, alt, no_snapshot } => {
            let mods = mouse::Modifiers { shift, cmd, alt, ctrl: false };
            cmd_click(json, target, y, app, limit, right, double_click, mods, no_snapshot)
        }
        Cmd::Scroll { direction, amount, x, y } => cmd_scroll(json, direction, amount, x, y),
        Cmd::Hover { x, y } => cmd_hover(json, x, y),
        Cmd::Drag { x1, y1, x2, y2 } => cmd_drag(json, x1, y1, x2, y2),
        Cmd::Screenshot { app, path, full } => cmd_screenshot(json, app, path, full),
    }
}

// ── Commands ────────────────────────────────────────────────────────────────

fn cmd_setup(json: bool) -> Result<(), String> {
    let ax = system::check_accessibility();
    let sr = system::check_screen_recording();

    if json {
        return ok(serde_json::json!({"ok": true, "accessibility": ax, "screen_recording": sr, "ready": ax && sr}));
    }

    println!("Accessibility:    {}", if ax { "granted" } else { "NOT GRANTED" });
    println!("Screen Recording: {}", if sr { "granted" } else { "NOT GRANTED" });
    println!();

    if ax && sr {
        println!("All permissions OK. computer-pilot is ready to use.");
    } else {
        if !ax {
            println!("Accessibility is required for snapshot, click, key, and type.\n→ System Settings → Privacy & Security → Accessibility\n");
        }
        if !sr {
            println!("Screen Recording is required for screenshot.\n→ System Settings → Privacy & Security → Screen Recording\n");
        }
        println!("Add your terminal app, then re-run: cu setup");
        let pane = if !ax { "Privacy_Accessibility" } else { "Privacy_ScreenCapture" };
        let _ = std::process::Command::new("open")
            .arg(format!("x-apple.systempreferences:com.apple.preference.security?{pane}"))
            .spawn();
    }
    Ok(())
}

fn cmd_status(json: bool) -> Result<(), String> {
    if json {
        ok(serde_json::json!({"ok": true, "platform": "macos", "version": VERSION}))
    } else {
        println!("computer-pilot: ok\nplatform: macos\nversion: {VERSION}");
        Ok(())
    }
}

fn cmd_apps(json: bool) -> Result<(), String> {
    let payload = system::list_apps()?;
    if json {
        println!("{payload}");
        return Ok(());
    }

    let parsed: serde_json::Value = serde_json::from_str(&payload)
        .map_err(|e| format!("failed to parse apps: {e}"))?;
    if let Some(apps) = parsed["apps"].as_array() {
        for app in apps {
            let active = if app["active"].as_bool() == Some(true) { "*" } else { " " };
            let scriptable = if app["scriptable"].as_bool() == Some(true) { "S" } else { " " };
            println!("{active}{scriptable} {} (pid {})", app["name"].as_str().unwrap_or("?"), app["pid"].as_i64().unwrap_or(0));
        }
    }
    Ok(())
}

fn cmd_snapshot(json: bool, app: Option<String>, limit: usize) -> Result<(), String> {
    let (pid, name) = system::resolve_target_app(&app)?;
    let result = ax::snapshot(pid, &name, limit);
    if !result.ok {
        return Err(result.error.unwrap_or_else(|| "snapshot failed".into()));
    }
    if json { emit(&result) } else { print_snapshot_human(&result) }
    Ok(())
}

fn cmd_wait(json: bool, text: Option<String>, ref_id: Option<usize>, gone: Option<usize>, app: Option<String>, timeout: u64, limit: usize) -> Result<(), String> {
    let condition = if let Some(t) = text {
        wait::Condition::Text(t)
    } else if let Some(r) = ref_id {
        wait::Condition::Ref(r)
    } else if let Some(g) = gone {
        wait::Condition::Gone(g)
    } else {
        return Err("specify one of: --text, --ref, or --gone".into());
    };

    let result = wait::wait_for(&condition, &app, timeout * 1000, limit)?;

    if !result.met {
        if json {
            emit(&serde_json::json!({"ok": false, "error": "timeout", "elapsed_ms": result.elapsed_ms, "snapshot": result.snapshot}));
        } else {
            eprintln!("Timeout after {}ms", result.elapsed_ms);
        }
        return Err("wait timed out".into());
    }

    if json {
        emit(&serde_json::json!({"ok": true, "elapsed_ms": result.elapsed_ms, "snapshot": result.snapshot}));
    } else {
        println!("Condition met after {}ms", result.elapsed_ms);
        print_snapshot_human(&result.snapshot);
    }
    Ok(())
}

fn cmd_ocr(json: bool, app: Option<String>) -> Result<(), String> {
    let (pid, _name) = system::resolve_target_app(&app)?;
    let result = ocr::recognize(pid);

    if !result.ok {
        return Err(result.error.unwrap_or_else(|| "OCR failed".into()));
    }

    if json {
        emit(&result);
    } else {
        for t in &result.texts {
            println!("[{:.0},{:.0} {:.0}×{:.0}] \"{}\" ({:.0}%)", t.x, t.y, t.width, t.height, t.text, t.confidence * 100.0);
        }
        if result.texts.is_empty() { println!("No text found."); }
    }
    Ok(())
}

fn cmd_type(json: bool, text: String, app: Option<String>, no_snapshot: bool) -> Result<(), String> {
    system::type_text(&text, app.as_deref())?;
    let mut result = serde_json::json!({"ok": true, "text": text});
    maybe_attach_snapshot(&mut result, json, no_snapshot, &app, 50);
    if json { ok(result) } else { println!("Typed: \"{text}\""); Ok(()) }
}

fn cmd_key(json: bool, combo: String, app: Option<String>, no_snapshot: bool) -> Result<(), String> {
    if let Some(ref app_name) = app {
        system::send_key(&combo, app_name)?;
    } else {
        key::send(&combo)?;
    }
    let mut result = serde_json::json!({"ok": true, "combo": combo});
    maybe_attach_snapshot(&mut result, json, no_snapshot, &app, 50);
    if json { ok(result) } else { println!("Sent key: {combo}"); Ok(()) }
}

fn cmd_click(json: bool, target: String, y: Option<String>, app: Option<String>, limit: usize, right: bool, double: bool, mods: mouse::Modifiers, no_snapshot: bool) -> Result<(), String> {
    // Coordinate mode
    if let Some(y_str) = y {
        let x: f64 = target.parse().map_err(|_| "invalid x coordinate")?;
        let y: f64 = y_str.parse().map_err(|_| "invalid y coordinate")?;
        if !x.is_finite() || !y.is_finite() {
            return Err("coordinates must be finite numbers".into());
        }
        if double {
            mouse::double_click(x, y, mods)?;
        } else {
            mouse::click(x, y, right, mods)?;
        }
        let mut result = serde_json::json!({"ok": true, "x": x, "y": y, "right": right});
        maybe_attach_snapshot(&mut result, json, no_snapshot, &app, limit);
        return if json { ok(result) } else { println!("Clicked ({x}, {y})"); Ok(()) };
    }

    // Ref mode — AX action first, CGEvent fallback
    let ref_id: usize = target.parse()
        .map_err(|_| "ref must be a positive integer (for coordinates: cu click <x> <y>)")?;
    if ref_id == 0 { return Err("ref must be >= 1".into()); }

    let (pid, name) = system::resolve_target_app(&app)?;
    let (ax_acted, cx, cy) = ax::ax_click(pid, ref_id, limit)?;

    let method = if double {
        mouse::double_click(cx, cy, mods)?;
        "double-click"
    } else if right || !ax_acted {
        mouse::click(cx, cy, right, mods)?;
        if right { "cgevent-right" } else { "cgevent" }
    } else {
        "ax-action"
    };

    let mut result = serde_json::json!({"ok": true, "ref": ref_id, "app": name, "method": method, "x": cx, "y": cy});
    maybe_attach_snapshot(&mut result, json, no_snapshot, &app, limit);
    if json { ok(result) } else { println!("Clicked [{ref_id}] via {method} at ({cx}, {cy})"); Ok(()) }
}

fn cmd_scroll(json: bool, direction: String, amount: i32, x: Option<f64>, y: Option<f64>) -> Result<(), String> {
    let (dx, dy) = match direction.to_lowercase().as_str() {
        "up" => (0, amount),
        "down" => (0, -amount),
        "left" => (-amount, 0),
        "right" => (amount, 0),
        other => return Err(format!("unknown direction: {other} (use: up, down, left, right)")),
    };
    let sx = x.unwrap_or(500.0);
    let sy = y.unwrap_or(400.0);
    mouse::scroll(sx, sy, dy, dx)?;
    if json { ok(serde_json::json!({"ok": true, "direction": direction, "amount": amount, "x": sx, "y": sy})) }
    else { println!("Scrolled {direction} {amount} at ({sx}, {sy})"); Ok(()) }
}

fn cmd_hover(json: bool, x: f64, y: f64) -> Result<(), String> {
    mouse::hover(x, y)?;
    if json { ok(serde_json::json!({"ok": true, "x": x, "y": y})) }
    else { println!("Hover at ({x}, {y})"); Ok(()) }
}

fn cmd_drag(json: bool, x1: f64, y1: f64, x2: f64, y2: f64) -> Result<(), String> {
    mouse::drag(x1, y1, x2, y2, mouse::Modifiers::default())?;
    if json { ok(serde_json::json!({"ok": true, "from": {"x": x1, "y": y1}, "to": {"x": x2, "y": y2}})) }
    else { println!("Dragged ({x1},{y1}) → ({x2},{y2})"); Ok(()) }
}

fn cmd_screenshot(json: bool, app: Option<String>, path: Option<String>, full: bool) -> Result<(), String> {
    let output_path = path.unwrap_or_else(|| {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis()).unwrap_or(0);
        format!("/tmp/cu-screenshot-{ts}.png")
    });

    if full {
        screenshot::capture_full_screen(&output_path)?;
        return if json {
            ok(serde_json::json!({"ok": true, "path": output_path, "mode": "full"}))
        } else {
            println!("Screenshot saved: {output_path} (full screen)"); Ok(())
        };
    }

    let (pid, name) = system::resolve_target_app(&app)?;
    let win = screenshot::find_window(pid)
        .ok_or("no on-screen window found for the target app")?;
    screenshot::capture_window(win.window_id, &output_path)?;

    if json {
        ok(serde_json::json!({"ok": true, "app": name, "path": output_path, "mode": "window", "offset_x": win.x, "offset_y": win.y}))
    } else {
        println!("Screenshot saved: {output_path} (window offset: {},{})", win.x, win.y); Ok(())
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn ok(value: serde_json::Value) -> Result<(), String> {
    emit(&value);
    Ok(())
}

fn emit(value: &impl serde::Serialize) {
    println!("{}", serde_json::to_string(value).unwrap_or_else(|_| r#"{"ok":false}"#.into()));
}

fn print_snapshot_human(snap: &ax::SnapshotResult) {
    let app = if snap.app.is_empty() { "Unknown" } else { &snap.app };
    let win = if snap.window.is_empty() { "Unknown" } else { &snap.window };
    println!("[app] {app} — \"{win}\"");
    for el in &snap.elements {
        let label = el.title.as_deref().or(el.value.as_deref()).unwrap_or("");
        let mut extra = Vec::new();
        if let Some(ref v) = el.value {
            if v != label { extra.push(format!("value=\"{v}\"")); }
        }
        extra.push(format!("{},{} {}×{}", el.x, el.y, el.width, el.height));
        println!("[{}] {} \"{}\" ({})", el.ref_id, el.role, label, extra.join(", "));
    }
    if snap.truncated { println!("  … truncated at {} elements", snap.elements.len()); }
}

fn maybe_attach_snapshot(result: &mut serde_json::Value, json: bool, no_snapshot: bool, app: &Option<String>, limit: usize) {
    if !json || no_snapshot { return; }
    std::thread::sleep(std::time::Duration::from_millis(POST_ACTION_DELAY_MS));
    if let Ok((pid, name)) = system::resolve_target_app(app) {
        let snap = ax::snapshot(pid, &name, limit);
        result["snapshot"] = serde_json::to_value(&snap).unwrap_or_default();
    }
}
