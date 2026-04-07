mod ax;
mod key;
mod mouse;
mod ocr;
mod screenshot;
mod sdef;
mod system;
mod wait;

use clap::{Parser, Subcommand};
use std::io::IsTerminal;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const POST_ACTION_DELAY_MS: u64 = 500;

// ── CLI definition ──────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "cu",
    version = VERSION,
    about = "macOS desktop automation CLI for AI agents",
    long_about = "macOS desktop automation CLI for AI agents.\n\n\
        THREE-TIER CONTROL:\n\
        1. AppleScript (scriptable apps) — cu tell / cu sdef\n\
        2. AX tree + CGEvent (any app)   — cu snapshot / cu click\n\
        3. OCR + screenshot (fallback)   — cu ocr / cu screenshot\n\n\
        WORKFLOW FOR SCRIPTABLE APPS (check S flag in cu apps):\n\
        1. cu apps                         — see what's running (S = scriptable)\n\
        2. cu sdef <app>                   — discover scripting dictionary\n\
        3. cu tell <app> '<AppleScript>'   — read/write app data directly\n\n\
        WORKFLOW FOR NON-SCRIPTABLE APPS:\n\
        1. cu menu <app>                   — discover what menus/features exist\n\
        2. cu snapshot [app] --limit 30    — get UI elements with [ref] numbers\n\
        3. cu click <ref> --app <name>     — click element by ref\n\n\
        SYSTEM CONTROL (no UI needed):\n\
        • cu defaults read/write           — change macOS preferences directly\n\
        • cu window list/move/resize       — manage windows of any app\n\
        • cu tell \"System Events\" '...'   — system-level control\n\n\
        TIPS FOR AI AGENTS:\n\
        • Use cu menu first to discover any app's capabilities via its menu bar\n\
        • Use cu defaults to change settings without navigating System Settings\n\
        • cu click --text \"label\" finds and clicks text via OCR\n\
        • Always use --app to target a specific app (avoids focus issues)\n\
        • Refs are ephemeral — they change after every action, always re-snapshot"
)]
struct Cli {
    /// Force human-readable output (default: JSON when piped, human when TTY)
    #[arg(long)]
    human: bool,

    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Check permissions, status, and version
    #[command(after_help = "Run this first on a new machine. Both permissions are required.")]
    Setup,

    /// List running applications with name, PID, and scriptable status
    #[command(
        after_help = "Example: cu apps\n  *S Finder (pid 572)     ← * = active, S = scriptable"
    )]
    Apps,

    /// Get UI elements with [ref] numbers (AX tree snapshot)
    #[command(after_help = "\
        Examples:\n  \
        cu snapshot Finder --limit 30\n  \
        cu snapshot \"Google Chrome\" --limit 50\n  \
        cu snapshot   # frontmost app\n\n\
        Output: elements with ref, role, title, value, x, y, width, height.\n\
        Use ref numbers with 'cu click <ref>' to interact with elements.")]
    Snapshot {
        /// Application name (default: frontmost app)
        app: Option<String>,
        /// Max elements to return
        #[arg(long, default_value = "50")]
        limit: usize,
    },

    /// Type text into the focused element (Unicode supported)
    #[command(after_help = "\
        Examples:\n  \
        cu type 'hello world' --app TextEdit\n  \
        cu type 'https://example.com' --app 'Google Chrome'")]
    Type {
        /// Text to type
        text: String,
        /// Target app (activates it first, types via System Events)
        #[arg(long)]
        app: Option<String>,
        /// Skip auto-snapshot in JSON output
        #[arg(long)]
        no_snapshot: bool,
    },

    /// Send a keyboard shortcut
    #[command(after_help = "\
        Examples:\n  \
        cu key cmd+c --app 'Google Chrome'    # copy\n  \
        cu key cmd+shift+n --app 'Google Chrome'  # new incognito\n  \
        cu key cmd+space                      # open Spotlight\n  \
        cu key enter --app Safari             # confirm\n  \
        cu key cmd+, --app Finder             # open Preferences\n  \
        cu key escape                         # cancel/close\n\n\
        Modifiers: cmd, shift, ctrl, alt (option)\n\
        Keys: a-z, 0-9, enter, tab, space, escape, delete, up/down/left/right, f1-f12")]
    Key {
        /// Key combination (e.g., cmd+c, enter, cmd+shift+s)
        combo: String,
        /// Target app (activates it, sends via System Events for reliability)
        #[arg(long)]
        app: Option<String>,
        /// Skip auto-snapshot in JSON output
        #[arg(long)]
        no_snapshot: bool,
    },

    /// Wait for a UI condition by polling the AX tree
    #[command(after_help = "\
        Examples:\n  \
        cu wait --text 'Submit' --app Safari --timeout 10\n  \
        cu wait --gone 5 --app Finder --timeout 5\n  \
        cu wait --ref 3 --app Contacts --timeout 10\n\n\
        Polls every 500ms. Returns snapshot when condition is met.")]
    Wait {
        /// Wait until any element contains this text (in title or value)
        #[arg(long)]
        text: Option<String>,
        /// Wait until element with this ref exists
        #[arg(long, name = "ref")]
        ref_id: Option<usize>,
        /// Wait until element with this ref disappears
        #[arg(long)]
        gone: Option<usize>,
        /// Target app (resolved once, prevents drift)
        #[arg(long)]
        app: Option<String>,
        /// Timeout in seconds
        #[arg(long, default_value = "10")]
        timeout: u64,
        /// Max elements per snapshot
        #[arg(long, default_value = "200")]
        limit: usize,
    },

    /// OCR — recognize text on screen via macOS Vision framework
    #[command(after_help = "\
        Examples:\n  \
        cu ocr Finder\n  \
        cu ocr 'Google Chrome'\n\n\
        Returns text with screen coordinates and confidence scores.\n\
        Use for apps with poor AX support (games, Qt, Java apps).")]
    Ocr {
        /// Application name (default: frontmost)
        app: Option<String>,
    },

    /// Click by ref, coordinates, or on-screen text (OCR)
    #[command(after_help = "\
        Three ways to click:\n  \
        cu click 3 --app Finder                # by ref (from cu snapshot)\n  \
        cu click 500 300                       # by coordinates\n  \
        cu click --text 'Submit' --app Safari  # by OCR text (finds text on screen)\n\n\
        Text mode (--text) uses OCR to find the text, then clicks its center.\n\
        Works for UI elements not in the AX tree (Notification Center, system panels).\n\
        Use --index N to click the Nth match (default: first).\n\n\
        Ref mode tries AX actions first, falls back to CGEvent.\n\
        Always use --app for reliability. Refs come from 'cu snapshot'.")]
    Click {
        /// Element ref number, or x coordinate
        target: Option<String>,
        /// Y coordinate (only when target is x coordinate)
        y: Option<String>,
        /// Find and click on-screen text via OCR
        #[arg(long)]
        text: Option<String>,
        /// Which match to click when using --text (default: 1 = first)
        #[arg(long, default_value = "1")]
        index: usize,
        /// Target application
        #[arg(long)]
        app: Option<String>,
        /// Max elements to scan in ref mode
        #[arg(long, default_value = "200")]
        limit: usize,
        /// Right-click instead of left-click
        #[arg(long)]
        right: bool,
        /// Double-click
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
        /// Skip auto-snapshot in JSON output
        #[arg(long)]
        no_snapshot: bool,
    },

    /// Scroll at a position (specify --x and --y)
    #[command(after_help = "\
        Examples:\n  \
        cu scroll down 5 --x 400 --y 300\n  \
        cu scroll up 3 --x 400 --y 300\n  \
        cu scroll left 2 --x 400 --y 300\n\n\
        Directions: up, down, left, right. Amount = number of lines.")]
    Scroll {
        /// Direction: up, down, left, right
        direction: String,
        /// Number of lines to scroll
        #[arg(default_value = "3")]
        amount: i32,
        /// X coordinate
        #[arg(long)]
        x: Option<f64>,
        /// Y coordinate
        #[arg(long)]
        y: Option<f64>,
    },

    /// Move mouse to coordinates (trigger tooltips, hover menus)
    #[command(after_help = "Example: cu hover 500 300")]
    Hover { x: f64, y: f64 },

    /// Drag from (x1,y1) to (x2,y2) with smooth interpolation
    #[command(after_help = "\
        Examples:\n  \
        cu drag 100 200 400 200             # drag right\n  \
        cu drag 100 200 400 200 --shift     # shift+drag (extend selection)\n  \
        cu drag 100 200 400 200 --alt       # option+drag (copy on macOS)")]
    Drag {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        /// Hold shift during drag
        #[arg(long)]
        shift: bool,
        /// Hold cmd during drag
        #[arg(long)]
        cmd: bool,
        /// Hold alt/option during drag
        #[arg(long)]
        alt: bool,
    },

    /// Capture window screenshot (silent, no app activation needed)
    #[command(after_help = "\
        Examples:\n  \
        cu screenshot 'Google Chrome' --path /tmp/chrome.png\n  \
        cu screenshot --full --path /tmp/screen.png\n\n\
        Window mode returns offset_x/offset_y for coordinate translation:\n  \
        screen_coord = image_pixel + offset")]
    Screenshot {
        /// Application name (default: frontmost)
        app: Option<String>,
        /// Output file path (default: /tmp/cu-screenshot-<ts>.png)
        #[arg(long)]
        path: Option<String>,
        /// Capture full screen instead of single window
        #[arg(long)]
        full: bool,
    },

    /// Manage windows (list, move, resize, focus, minimize, close)
    #[command(after_help = "\
        Window management via System Events. Works for ALL apps.\n\n\
        Examples:\n  \
        cu window list                        # list all windows\n  \
        cu window list --app Safari           # list Safari windows only\n  \
        cu window move 100 100 --app Safari   # move front window\n  \
        cu window resize 1200 800 --app Safari\n  \
        cu window focus --app Safari          # bring to front\n  \
        cu window minimize --app Safari\n  \
        cu window close --app Safari          # close front window\n\n\
        Actions: list, move, resize, focus, minimize, unminimize, close\n\
        Default target: front window of specified app")]
    Window {
        /// Action: list, move, resize, focus, minimize, unminimize, close
        action: String,
        /// First numeric arg (x for move, width for resize)
        arg1: Option<i64>,
        /// Second numeric arg (y for move, height for resize)
        arg2: Option<i64>,
        /// Target app
        #[arg(long)]
        app: Option<String>,
        /// Window index (1 = frontmost, default)
        #[arg(long, default_value = "1")]
        window: usize,
    },

    /// List an app's menu bar items (works for ALL apps via System Events)
    #[command(after_help = "\
        Enumerates every menu and menu item in the app's menu bar.\n\
        Works for ANY app — scriptable or not. Uses System Events.\n\n\
        Examples:\n  \
        cu menu Calculator     # see View > Scientific, View > Programmer\n  \
        cu menu Safari         # see File > New Window, View > Show Reader\n  \
        cu menu Finder         # see File > New Finder Window, Go > Home\n\n\
        To click a menu item, use cu tell with System Events:\n  \
        cu tell \"System Events\" 'tell process \"Calculator\" to click menu item \\\n    \
          \"Scientific\" of menu \"View\" of menu bar 1'")]
    Menu {
        /// Target application name
        app: String,
    },

    /// Read or write macOS system preferences (no UI needed)
    #[command(after_help = "\
        Read/write macOS preferences via the defaults system.\n\
        Bypasses System Settings UI entirely.\n\n\
        Examples:\n  \
        cu defaults read com.apple.dock autohide\n  \
        cu defaults write com.apple.dock autohide -bool true\n  \
        cu defaults read NSGlobalDomain KeyRepeat\n  \
        cu defaults write NSGlobalDomain KeyRepeat -int 2\n  \
        cu defaults read com.apple.calculator ViewDefaultsKey\n\n\
        After writing dock/finder settings, restart with:\n  \
        killall Dock   or   killall Finder")]
    Defaults {
        /// Subcommand: read or write
        action: String,
        /// Preference domain (e.g., com.apple.dock, NSGlobalDomain)
        domain: String,
        /// Preference key
        key: Option<String>,
        /// Value to write (with type flag: -bool, -int, -float, -string)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        value: Vec<String>,
    },

    /// Show an app's scripting dictionary (classes, commands, properties)
    #[command(after_help = "\
        Reads the app's sdef file and returns a structured summary of its\n\
        scripting capabilities: classes (with properties), commands, and elements.\n\
        Use this to discover what `cu tell` expressions are possible.\n\n\
        Examples:\n  \
        cu sdef Safari                  # full dictionary\n  \
        cu sdef Calendar                # calendar scripting API\n  \
        cu sdef Finder                  # file management API\n  \
        cu sdef \"System Events\"         # system controls\n\n\
        Workflow: cu apps → cu sdef <app> → cu tell <app> '<expression>'")]
    Sdef {
        /// Application name
        app: String,
    },

    /// Execute AppleScript against a scriptable app
    #[command(after_help = "\
        Run AppleScript in the context of a target application.\n\
        The expression is auto-wrapped in `tell application \"<app>\" ... end tell`.\n\n\
        Examples:\n  \
        cu tell Safari 'get URL of current tab of front window'\n  \
        cu tell Finder 'get name of every item of front window'\n  \
        cu tell Calendar 'get name of every calendar'\n  \
        cu tell Music 'get name of current track'\n  \
        cu tell \"System Events\" 'get dark mode of appearance preferences'\n  \
        cu tell Notes 'get name of every note'\n  \
        cu tell Reminders 'get name of every list'\n\n\
        Write operations:\n  \
        cu tell Calendar 'make new event at end of events of first calendar \\\n    \
          with properties {summary:\"Meeting\", start date:date \"2026-04-06 14:00\"}'\n  \
        cu tell Notes 'make new note with properties {name:\"Test\", body:\"Hello\"}'\n\n\
        Tip: Use `cu apps` to see which apps are scriptable (S flag),\n\
        then `cu sdef <app>` to discover what properties/commands are available.")]
    Tell {
        /// Target application name
        app: String,
        /// AppleScript expression (auto-wrapped in tell application ... end tell)
        expr: String,
        /// Timeout in seconds
        #[arg(long, default_value = "10")]
        timeout: u64,
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
        Cmd::Apps => cmd_apps(json),
        Cmd::Snapshot { app, limit } => cmd_snapshot(json, app, limit),
        Cmd::Wait {
            text,
            ref_id,
            gone,
            app,
            timeout,
            limit,
        } => cmd_wait(json, text, ref_id, gone, app, timeout, limit),
        Cmd::Ocr { app } => cmd_ocr(json, app),
        Cmd::Type {
            text,
            app,
            no_snapshot,
        } => cmd_type(json, text, app, no_snapshot),
        Cmd::Key {
            combo,
            app,
            no_snapshot,
        } => cmd_key(json, combo, app, no_snapshot),
        Cmd::Click {
            target,
            y,
            text,
            index,
            app,
            limit,
            right,
            double_click,
            shift,
            cmd,
            alt,
            no_snapshot,
        } => {
            let mods = mouse::Modifiers {
                shift,
                cmd,
                alt,
                ctrl: false,
            };
            cmd_click(ClickOptions {
                json,
                target,
                y,
                text,
                index,
                app,
                limit,
                right,
                double: double_click,
                mods,
                no_snapshot,
            })
        }
        Cmd::Scroll {
            direction,
            amount,
            x,
            y,
        } => cmd_scroll(json, direction, amount, x, y),
        Cmd::Hover { x, y } => cmd_hover(json, x, y),
        Cmd::Drag {
            x1,
            y1,
            x2,
            y2,
            shift,
            cmd,
            alt,
        } => {
            let mods = mouse::Modifiers {
                shift,
                cmd,
                alt,
                ctrl: false,
            };
            cmd_drag(json, x1, y1, x2, y2, mods)
        }
        Cmd::Screenshot { app, path, full } => cmd_screenshot(json, app, path, full),
        Cmd::Window {
            action,
            arg1,
            arg2,
            app,
            window,
        } => cmd_window(json, action, arg1, arg2, app, window),
        Cmd::Menu { app } => cmd_menu(json, app),
        Cmd::Defaults {
            action,
            domain,
            key,
            value,
        } => cmd_defaults(json, action, domain, key, value),
        Cmd::Sdef { app } => cmd_sdef(json, app),
        Cmd::Tell { app, expr, timeout } => cmd_tell(json, app, expr, timeout),
    }
}

// ── Commands ────────────────────────────────────────────────────────────────

fn cmd_setup(json: bool) -> Result<(), String> {
    let ax = system::check_accessibility();
    let sr = system::check_screen_recording();
    let auto = system::check_automation();
    let ready = ax && sr; // core: snapshot, click, key, type, screenshot, ocr
    let scripting_ready = ready && auto; // scripting: cu tell

    if json {
        return ok(serde_json::json!({
            "ok": true, "version": VERSION, "platform": "macos",
            "accessibility": ax, "screen_recording": sr, "automation": auto,
            "ready": ready, "scripting_ready": scripting_ready
        }));
    }

    println!("cu v{VERSION} — macOS desktop automation");
    println!(
        "Accessibility:    {}",
        if ax { "granted" } else { "NOT GRANTED" }
    );
    println!(
        "Screen Recording: {}",
        if sr { "granted" } else { "NOT GRANTED" }
    );
    println!(
        "Automation:       {}",
        if auto { "granted" } else { "NOT GRANTED" }
    );
    println!();

    if scripting_ready {
        println!("All permissions OK. Ready to use.");
    } else {
        if !ax {
            println!(
                "Accessibility is required for snapshot, click, key, and type.\n→ System Settings → Privacy & Security → Accessibility\n"
            );
        }
        if !sr {
            println!(
                "Screen Recording is required for screenshot and OCR.\n→ System Settings → Privacy & Security → Screen Recording\n"
            );
        }
        if !auto {
            println!(
                "Automation is needed for cu tell (scripting). Granted per-app on first use.\n→ System Settings → Privacy & Security → Automation\n"
            );
        }
        println!("Add your terminal app, then re-run: cu setup");
        if !ax {
            let _ = std::process::Command::new("open")
                .arg(
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
                )
                .spawn();
        } else if !sr {
            let _ = std::process::Command::new("open")
                .arg(
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
                )
                .spawn();
        }
        // Automation pane can't be opened directly; it's per-app on first use.
    }
    Ok(())
}

fn cmd_apps(json: bool) -> Result<(), String> {
    let payload = system::list_apps()?;
    if json {
        println!("{payload}");
        return Ok(());
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&payload).map_err(|e| format!("failed to parse apps: {e}"))?;
    if let Some(apps) = parsed["apps"].as_array() {
        for app in apps {
            let active = if app["active"].as_bool() == Some(true) {
                "*"
            } else {
                " "
            };
            let scriptable = if app["scriptable"].as_bool() == Some(true) {
                "S"
            } else {
                " "
            };
            let classes = app["sdef_classes"]
                .as_i64()
                .map(|n| format!(" [{n} classes]"))
                .unwrap_or_default();
            println!(
                "{active}{scriptable} {} (pid {}){classes}",
                app["name"].as_str().unwrap_or("?"),
                app["pid"].as_i64().unwrap_or(0)
            );
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
    if json {
        emit(&result)
    } else {
        print_snapshot_human(&result)
    }
    Ok(())
}

fn cmd_wait(
    json: bool,
    text: Option<String>,
    ref_id: Option<usize>,
    gone: Option<usize>,
    app: Option<String>,
    timeout: u64,
    limit: usize,
) -> Result<(), String> {
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
            emit(
                &serde_json::json!({"ok": false, "error": "timeout", "elapsed_ms": result.elapsed_ms, "snapshot": result.snapshot}),
            );
        } else {
            eprintln!("Timeout after {}ms", result.elapsed_ms);
        }
        std::process::exit(1);
    }

    if json {
        emit(
            &serde_json::json!({"ok": true, "elapsed_ms": result.elapsed_ms, "snapshot": result.snapshot}),
        );
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
            println!(
                "[{:.0},{:.0} {:.0}×{:.0}] \"{}\" ({:.0}%)",
                t.x,
                t.y,
                t.width,
                t.height,
                t.text,
                t.confidence * 100.0
            );
        }
        if result.texts.is_empty() {
            println!("No text found.");
        }
    }
    Ok(())
}

fn cmd_type(
    json: bool,
    text: String,
    app: Option<String>,
    no_snapshot: bool,
) -> Result<(), String> {
    system::type_text(&text, app.as_deref())?;
    let mut result = serde_json::json!({"ok": true, "text": text});
    maybe_attach_snapshot(&mut result, json, no_snapshot, &app, 50);
    if json {
        ok(result)
    } else {
        println!("Typed: \"{text}\"");
        Ok(())
    }
}

fn cmd_key(
    json: bool,
    combo: String,
    app: Option<String>,
    no_snapshot: bool,
) -> Result<(), String> {
    if let Some(ref app_name) = app {
        system::send_key(&combo, app_name)?;
    } else {
        key::send(&combo)?;
    }
    let mut result = serde_json::json!({"ok": true, "combo": combo});
    maybe_attach_snapshot(&mut result, json, no_snapshot, &app, 50);
    if json {
        ok(result)
    } else {
        println!("Sent key: {combo}");
        Ok(())
    }
}

struct ClickOptions {
    json: bool,
    target: Option<String>,
    y: Option<String>,
    text: Option<String>,
    index: usize,
    app: Option<String>,
    limit: usize,
    right: bool,
    double: bool,
    mods: mouse::Modifiers,
    no_snapshot: bool,
}

fn cmd_click(opts: ClickOptions) -> Result<(), String> {
    let ClickOptions {
        json,
        target,
        y,
        text,
        index,
        app,
        limit,
        right,
        double,
        mods,
        no_snapshot,
    } = opts;

    // Mode 1: --text "Submit" → OCR-based click
    if let Some(ref search_text) = text {
        let (pid, _) = if app.is_some() {
            system::resolve_target_app(&app)?
        } else {
            (0, String::new()) // full screen OCR
        };

        let result = if pid != 0 {
            ocr::recognize(pid)
        } else {
            // Full screen: use frontmost app as fallback for OCR
            let (fp, _) = system::resolve_target_app(&None)?;
            ocr::recognize(fp)
        };
        if !result.ok {
            return Err(result.error.unwrap_or_else(|| "OCR failed".into()));
        }

        // Find matching text regions (case-insensitive substring match)
        let lower_search = search_text.to_lowercase();
        let matches: Vec<&ocr::OcrText> = result
            .texts
            .iter()
            .filter(|t| t.text.to_lowercase().contains(&lower_search))
            .collect();

        if matches.is_empty() {
            return Err(format!(
                "text \"{}\" not found on screen (OCR found {} regions)",
                search_text,
                result.texts.len()
            ));
        }
        if index == 0 || index > matches.len() {
            return Err(format!(
                "--index {} out of range (found {} matches for \"{}\")",
                index,
                matches.len(),
                search_text
            ));
        }

        let matched = matches[index - 1];
        let cx = matched.x + matched.width / 2.0;
        let cy = matched.y + matched.height / 2.0;

        if double {
            mouse::double_click(cx, cy, mods)?;
        } else {
            mouse::click(cx, cy, right, mods)?;
        }

        let mut result = serde_json::json!({
            "ok": true, "method": "ocr-text", "text": matched.text,
            "x": cx, "y": cy, "matches": matches.len()
        });
        maybe_attach_snapshot(&mut result, json, no_snapshot, &app, limit);
        return if json {
            ok(result)
        } else {
            println!("Clicked \"{}\" at ({cx}, {cy})", matched.text);
            Ok(())
        };
    }

    let target = target.ok_or("specify a ref, coordinates (x y), or --text")?;

    // Mode 2: cu click <x> <y> → coordinate click
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
        return if json {
            ok(result)
        } else {
            println!("Clicked ({x}, {y})");
            Ok(())
        };
    }

    // Mode 3: cu click <ref> → AX ref click
    let ref_id: usize = target
        .parse()
        .map_err(|_| "ref must be a positive integer (for coordinates: cu click <x> <y>)")?;
    if ref_id == 0 {
        return Err("ref must be >= 1".into());
    }

    let (pid, name) = system::resolve_target_app(&app)?;

    let (method, cx, cy) = if right || double {
        let (_, cx, cy) = ax::ax_find_element(pid, ref_id, limit)?;
        if double {
            mouse::double_click(cx, cy, mods)?;
            ("double-click", cx, cy)
        } else {
            mouse::click(cx, cy, true, mods)?;
            ("cgevent-right", cx, cy)
        }
    } else {
        let (ax_acted, cx, cy) = ax::ax_click(pid, ref_id, limit)?;
        if !ax_acted {
            mouse::click(cx, cy, false, mods)?;
            ("cgevent", cx, cy)
        } else {
            ("ax-action", cx, cy)
        }
    };

    let mut result = serde_json::json!({"ok": true, "ref": ref_id, "app": name, "method": method, "x": cx, "y": cy});
    maybe_attach_snapshot(&mut result, json, no_snapshot, &app, limit);
    if json {
        ok(result)
    } else {
        println!("Clicked [{ref_id}] via {method} at ({cx}, {cy})");
        Ok(())
    }
}

fn cmd_scroll(
    json: bool,
    direction: String,
    amount: i32,
    x: Option<f64>,
    y: Option<f64>,
) -> Result<(), String> {
    let (dx, dy) = match direction.to_lowercase().as_str() {
        "up" => (0, amount),
        "down" => (0, -amount),
        "left" => (-amount, 0),
        "right" => (amount, 0),
        other => {
            return Err(format!(
                "unknown direction: {other} (use: up, down, left, right)"
            ));
        }
    };
    let sx = x.ok_or("--x is required for scroll")?;
    let sy = y.ok_or("--y is required for scroll")?;
    mouse::scroll(sx, sy, dy, dx)?;
    if json {
        ok(
            serde_json::json!({"ok": true, "direction": direction, "amount": amount, "x": sx, "y": sy}),
        )
    } else {
        println!("Scrolled {direction} {amount} at ({sx}, {sy})");
        Ok(())
    }
}

fn cmd_hover(json: bool, x: f64, y: f64) -> Result<(), String> {
    mouse::hover(x, y)?;
    if json {
        ok(serde_json::json!({"ok": true, "x": x, "y": y}))
    } else {
        println!("Hover at ({x}, {y})");
        Ok(())
    }
}

fn cmd_drag(
    json: bool,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    mods: mouse::Modifiers,
) -> Result<(), String> {
    mouse::drag(x1, y1, x2, y2, mods)?;
    if json {
        ok(serde_json::json!({"ok": true, "from": {"x": x1, "y": y1}, "to": {"x": x2, "y": y2}}))
    } else {
        println!("Dragged ({x1},{y1}) → ({x2},{y2})");
        Ok(())
    }
}

fn cmd_screenshot(
    json: bool,
    app: Option<String>,
    path: Option<String>,
    full: bool,
) -> Result<(), String> {
    let output_path = path.unwrap_or_else(|| {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("/tmp/cu-screenshot-{ts}.png")
    });

    if full {
        screenshot::capture_full_screen(&output_path)?;
        return if json {
            ok(serde_json::json!({"ok": true, "path": output_path, "mode": "full"}))
        } else {
            println!("Screenshot saved: {output_path} (full screen)");
            Ok(())
        };
    }

    let (pid, name) = system::resolve_target_app(&app)?;
    let win = screenshot::find_window(pid).ok_or("no on-screen window found for the target app")?;
    screenshot::capture_window(win.window_id, &output_path)?;

    if json {
        ok(
            serde_json::json!({"ok": true, "app": name, "path": output_path, "mode": "window", "offset_x": win.x, "offset_y": win.y}),
        )
    } else {
        println!(
            "Screenshot saved: {output_path} (window offset: {},{})",
            win.x, win.y
        );
        Ok(())
    }
}

fn cmd_window(
    json: bool,
    action: String,
    arg1: Option<i64>,
    arg2: Option<i64>,
    app: Option<String>,
    window_idx: usize,
) -> Result<(), String> {
    if action == "list" {
        let windows = system::list_windows(app.as_deref())?;
        if json {
            ok(serde_json::json!({"ok": true, "windows": windows}))
        } else {
            if windows.is_empty() {
                println!("No windows found.");
            } else {
                for w in &windows {
                    let flags = if w.minimized {
                        " [minimized]"
                    } else if w.focused {
                        " [focused]"
                    } else {
                        ""
                    };
                    println!(
                        "{} #{} \"{}\"  {}×{} at ({},{}){}",
                        w.app, w.index, w.title, w.width, w.height, w.x, w.y, flags
                    );
                }
            }
            Ok(())
        }
    } else {
        // All other actions require --app
        let app_name = app.ok_or("--app is required for this action")?;
        system::window_action(&action, &app_name, window_idx, arg1, arg2)?;
        if json {
            ok(
                serde_json::json!({"ok": true, "action": action, "app": app_name, "window": window_idx}),
            )
        } else {
            println!("{action} window {window_idx} of {app_name}");
            Ok(())
        }
    }
}

fn cmd_menu(json: bool, app: String) -> Result<(), String> {
    let items = system::list_menu(&app)?;

    if items.is_empty() {
        return Err(format!("no menu items found for {app} (is it running?)"));
    }

    if json {
        ok(serde_json::json!({"ok": true, "app": app, "items": items}))
    } else {
        println!("{app} menu bar:\n");
        let mut current_menu = String::new();
        for it in &items {
            if it.menu != current_menu {
                if !current_menu.is_empty() {
                    println!();
                }
                current_menu = it.menu.clone();
                println!("  {}", it.menu);
            }
            let suffix = if it.enabled { "" } else { " (disabled)" };
            println!("    {}{suffix}", it.item);
        }
        Ok(())
    }
}

fn cmd_defaults(
    json: bool,
    action: String,
    domain: String,
    key: Option<String>,
    value: Vec<String>,
) -> Result<(), String> {
    match action.as_str() {
        "read" => {
            let result = system::defaults_read(&domain, key.as_deref())?;
            if json {
                ok(serde_json::json!({"ok": true, "domain": domain, "key": key, "value": result}))
            } else {
                println!("{result}");
                Ok(())
            }
        }
        "write" => {
            let k = key.ok_or("key is required for defaults write")?;
            system::defaults_write(&domain, &k, &value)?;
            if json {
                ok(serde_json::json!({"ok": true, "domain": domain, "key": k}))
            } else {
                println!("Set {domain} {k}");
                Ok(())
            }
        }
        other => Err(format!(
            "unknown defaults action: {other} (use: read, write)"
        )),
    }
}

fn cmd_sdef(json: bool, app: String) -> Result<(), String> {
    let bundle_path = system::resolve_app_bundle_path(&app)?;
    let result = sdef::parse(&app, &bundle_path);

    if !result.ok {
        return Err(result.error.unwrap_or_else(|| "sdef parse failed".into()));
    }

    if json {
        emit(&result);
        return Ok(());
    }

    // Human-readable output
    println!("{} scripting dictionary:\n", result.app);
    if let Some(ref suites) = result.suites {
        for suite in suites {
            println!("  suite: {}", suite.name);
            for cls in &suite.classes {
                print!("    {}", cls.name);
                if !cls.properties.is_empty() {
                    let names: Vec<&str> = cls.properties.iter().map(|p| p.name.as_str()).collect();
                    print!(" — props: {}", names.join(", "));
                }
                if !cls.elements.is_empty() {
                    print!(" — elements: {}", cls.elements.join(", "));
                }
                if !cls.responds_to.is_empty() {
                    print!(" — responds-to: {}", cls.responds_to.join(", "));
                }
                println!();
            }
            if !suite.commands.is_empty() {
                let names: Vec<&str> = suite.commands.iter().map(|c| c.name.as_str()).collect();
                println!("    commands: {}", names.join(", "));
            }
            println!();
        }
    }
    Ok(())
}

fn cmd_tell(json: bool, app: String, expr: String, timeout: u64) -> Result<(), String> {
    let raw = system::tell_app(&app, &expr, timeout)?;

    if json {
        ok(serde_json::json!({"ok": true, "app": app, "result": raw}))
    } else {
        if raw.is_empty() {
            println!("OK");
        } else {
            println!("{raw}");
        }
        Ok(())
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn ok(value: serde_json::Value) -> Result<(), String> {
    emit(&value);
    Ok(())
}

fn emit(value: &impl serde::Serialize) {
    println!(
        "{}",
        serde_json::to_string(value).unwrap_or_else(|_| r#"{"ok":false}"#.into())
    );
}

fn print_snapshot_human(snap: &ax::SnapshotResult) {
    let app = if snap.app.is_empty() {
        "Unknown"
    } else {
        &snap.app
    };
    let win = if snap.window.is_empty() {
        "Unknown"
    } else {
        &snap.window
    };
    if let Some(ref wf) = snap.window_frame {
        println!(
            "[app] {app} — \"{win}\" ({}×{} at {},{})",
            wf.width, wf.height, wf.x, wf.y
        );
    } else {
        println!("[app] {app} — \"{win}\"");
    }
    for el in &snap.elements {
        let label = el.title.as_deref().or(el.value.as_deref()).unwrap_or("");
        let mut extra = Vec::new();
        if let Some(ref v) = el.value
            && v != label
        {
            extra.push(format!("value=\"{v}\""));
        }
        extra.push(format!("{},{} {}×{}", el.x, el.y, el.width, el.height));
        println!(
            "[{}] {} \"{}\" ({})",
            el.ref_id,
            el.role,
            label,
            extra.join(", ")
        );
    }
    if snap.truncated {
        println!("  … truncated at {} elements", snap.elements.len());
    }
}

fn maybe_attach_snapshot(
    result: &mut serde_json::Value,
    json: bool,
    no_snapshot: bool,
    app: &Option<String>,
    limit: usize,
) {
    if !json || no_snapshot {
        return;
    }
    std::thread::sleep(std::time::Duration::from_millis(POST_ACTION_DELAY_MS));
    if let Ok((pid, name)) = system::resolve_target_app(app) {
        let snap = ax::snapshot(pid, &name, limit);
        result["snapshot"] = serde_json::to_value(&snap).unwrap_or_default();
    }
}
