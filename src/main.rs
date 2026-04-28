mod ax;
mod diff;
mod display;
mod error;
mod key;
mod mouse;
mod observer;
mod ocr;
mod sck;
mod screenshot;
mod sdef;
mod system;
mod wait;

use clap::{Parser, Subcommand};
use error::CuError;
use std::io::IsTerminal;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const POST_ACTION_DELAY_MS: u64 = 500;

/// Maps an action `method` string to a (confidence, advice) pair.
///
/// `confidence` is one of "high" / "medium" / "low" — agents can use it to
/// decide whether to verify with a fresh snapshot.
/// `advice` is empty when the method is best-case; otherwise a one-line hint
/// with the next-best action.
fn method_meta(method: &str) -> (&'static str, &'static str) {
    match method {
        // Best — direct AX call, no cursor move at all.
        "ax-action" | "ax-set-value" | "ax-perform" => ("high", ""),
        // PID-targeted CGEvent — non-disruptive, but a small set of sandboxed
        // apps ignore PID-targeted events. Verify with snapshot if unsure.
        "cgevent-pid" | "key-pid" | "unicode-pid" => ("high", ""),
        // OCR text click — visual coords; element may have moved/re-laid-out.
        "ocr-text-pid" => (
            "medium",
            "OCR-located coordinates — verify outcome with a fresh snapshot",
        ),
        // Global HID tap — disruptive (cursor warps, focus may steal).
        "cgevent-global" | "key-global" | "unicode-global" => (
            "low",
            "global HID tap was used (disruptive) — pass --app <Name> to keep cursor/focus put",
        ),
        "ocr-text-global" => (
            "low",
            "global tap + OCR coords — pass --app and verify with snapshot",
        ),
        _ => ("medium", ""),
    }
}

/// Inserts `confidence` and `advice` (when non-empty) into an action result
/// JSON object, keying off its `method` field. No-op if `method` is missing.
fn annotate_method(result: &mut serde_json::Value) {
    let Some(method) = result.get("method").and_then(|v| v.as_str()) else {
        return;
    };
    let (confidence, advice) = method_meta(method);
    if let Some(obj) = result.as_object_mut() {
        obj.insert(
            "confidence".to_string(),
            serde_json::Value::String(confidence.to_string()),
        );
        if !advice.is_empty() {
            obj.insert(
                "advice".to_string(),
                serde_json::Value::String(advice.to_string()),
            );
        }
    }
}

// ── CLI definition ──────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "cu",
    version = VERSION,
    about = "macOS desktop automation CLI for AI agents",
    before_help = "COMMANDS BY CATEGORY (27 total):\n  \
        Discover         setup · apps · menu · sdef · examples\n  \
        Observe          snapshot · state · find · nearest · observe-region · ocr · screenshot · wait\n  \
        Act              click · type · key · set-value · perform · scroll · hover · drag\n  \
        Script & System  tell · defaults · window · launch · warm · why\n\n\
        Run `cu <command> --help` for any command's full reference + examples.\n\
        Stuck? Try `cu examples` for a built-in recipe list.",
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
        WORKFLOW FOR VLM AGENTS (vision-equipped):\n\
        1. cu snapshot <app> --annotated   — PNG with each ref's box+number drawn\n\
        2. agent looks at PNG, picks ref by visual identification\n\
        3. cu click <ref> --app <name>     — act by ref, no coordinate guessing\n\
        Or: cu nearest <x> <y> --app <X>   — translate visual coords to ref\n\
            cu observe-region <x> <y> <w> <h> — list candidate refs in a region\n\n\
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
        /// Return only elements that changed since the last snapshot of this app.
        /// First call (no cache) returns the full snapshot with `first_snapshot:true`.
        /// Element identity is (role, round(x), round(y)); content change = title/value/size.
        #[arg(long)]
        diff: bool,
        /// Save a window screenshot with each ref's bounding box + number drawn on it.
        /// VLM agent flow: look at the PNG, identify element visually, then `cu click <ref>`.
        /// Path defaults to /tmp/cu-annotated-<ts>.png; override with --output.
        #[arg(long)]
        annotated: bool,
        /// Attach a plain (un-annotated) window screenshot to the snapshot output.
        /// Guarantees the tree and image are from the same instant. Skipped when --annotated
        /// is also set (annotated already includes a screenshot).
        #[arg(long = "with-screenshot")]
        with_screenshot: bool,
        /// Output path for the annotated PNG or --with-screenshot PNG.
        #[arg(long)]
        output: Option<String>,
    },

    /// Type text into the focused element (Unicode supported)
    #[command(after_help = "\
        PREFER:\n  \
        Use `cu set-value <ref>` instead when filling an AX textfield (faster, no focus needed).\n  \
        Use this when the target is non-AX (Electron, browser page input) or when you need\n  \
        to drive a multi-step keystroke flow that requires real focus + IME-bypass entry.\n\n\
        Examples:\n  \
        cu type 'hello world' --app TextEdit\n  \
        cu type 'https://example.com' --app 'Google Chrome'\n  \
        cu type '你好世界' --app WeChat            # auto-routed via paste (CJK + chat app)\n\
        cu type 'hello' --app TextEdit --no-paste # force unicode events\n\n\
        With --app: events go directly to that app's pid via Unicode CGEvent —\n\
        no focus theft, no clipboard pollution, IME bypassed.\n\
        Without --app: refused by default if the frontmost is a terminal or IDE\n\
        (stray text would execute commands). Pass --allow-global to override.\n\n\
        Paste mode (auto, opt-in via --paste, opt-out via --no-paste):\n\
        Routes through pbcopy + ⌘V instead of N unicode events. Auto-enabled when\n\
        the text contains CJK characters OR the target app is in the chat-app list\n\
        (WeChat, Slack, Discord, Telegram, QQ, Lark/Feishu, DingTalk). These apps\n\
        drop the first character of unicode events. Original clipboard preserved.")]
    Type {
        /// Text to type
        text: String,
        /// Target app — events delivered directly to its pid (no focus theft)
        #[arg(long)]
        app: Option<String>,
        /// Skip auto-snapshot in JSON output
        #[arg(long)]
        no_snapshot: bool,
        /// Skip the frontmost-app safety check. Without --app, cu refuses by default
        /// when the frontmost is a terminal or IDE — pass this to override.
        #[arg(long = "allow-global")]
        allow_global: bool,
        /// Force clipboard paste (pbcopy + ⌘V) regardless of auto-detection.
        #[arg(long, conflicts_with = "no_paste")]
        paste: bool,
        /// Force unicode-event typing even when auto-detection would route via paste.
        /// Use when you've confirmed the target app handles unicode events correctly.
        #[arg(long = "no-paste")]
        no_paste: bool,
    },

    /// Perform any AX action on an element (AXShowMenu, AXIncrement, AXScrollToVisible, ...)
    #[command(after_help = "\
        PREFER:\n  \
        Use `cu click` for the common AXPress case (cu click already tries the AX action chain).\n  \
        Use this when you need a non-press action: AXShowMenu (right-click context menu),\n  \
        AXIncrement/AXDecrement (steppers), AXScrollToVisible (reveal off-screen row), AXOpen, etc.\n\n\
        Exposes AXUIElementPerformAction directly. Lets the agent trigger actions\n\
        beyond what `cu click` covers — open context menus, increment steppers,\n\
        scroll to make an off-screen row visible, etc.\n\n\
        On failure the response includes the element's actual available actions\n\
        in `diagnostics.available_actions` and `suggested_next`, so the agent\n\
        can self-correct without an extra snapshot.\n\n\
        Common actions: AXPress, AXShowMenu, AXIncrement, AXDecrement,\n\
                        AXScrollToVisible, AXRaise, AXCancel, AXConfirm, AXOpen\n\n\
        Examples:\n  \
        cu perform 12 AXShowMenu --app Finder       # right-click menu on item\n  \
        cu perform 5 AXIncrement --app 'System Settings'\n  \
        cu perform 99 AXScrollToVisible --app Mail")]
    Perform {
        /// Element ref (from cu snapshot). Optional when --ax-path is given.
        ref_id: Option<usize>,
        /// AX action name (e.g. AXShowMenu, AXIncrement)
        action: String,
        /// Target app
        #[arg(long)]
        app: Option<String>,
        /// AX tree walk depth limit
        #[arg(long, default_value = "50")]
        limit: usize,
        /// Stable element selector (axPath). Survives across snapshots.
        #[arg(long = "ax-path")]
        ax_path: Option<String>,
        /// Skip auto-snapshot in JSON output
        #[arg(long)]
        no_snapshot: bool,
    },

    /// Write text directly into a UI element via AX (no focus, no IME, no clipboard)
    #[command(after_help = "\
        PREFER:\n  \
        Use this over `cu type` when the target is an AX textfield/textarea/combobox\n  \
        (faster, no focus shift, no clipboard pollution).\n  \
        Fall back to `cu click + cu type` for non-AX inputs (Electron, web pages, etc).\n\n\
        Fastest way to fill a text field. Uses AXUIElementSetAttributeValue\n\
        on AXValue — works on AXTextField / AXTextArea / AXComboBox.\n\
        For controls that reject AXValue writes, fall back to:\n  \
        cu click <ref> --app X    # focus first\n  \
        cu type 'text' --app X    # then type\n\n\
        Examples:\n  \
        cu set-value 5 'alice@example.com' --app Mail\n  \
        cu set-value 12 'https://github.com' --app Safari")]
    SetValue {
        /// Element ref (from cu snapshot). Optional when --ax-path is given.
        ref_id: Option<usize>,
        /// Value to write
        value: String,
        /// Target app
        #[arg(long)]
        app: Option<String>,
        /// AX tree walk depth limit
        #[arg(long, default_value = "50")]
        limit: usize,
        /// Stable element selector (axPath). Survives across snapshots.
        #[arg(long = "ax-path")]
        ax_path: Option<String>,
        /// Skip auto-snapshot in JSON output
        #[arg(long)]
        no_snapshot: bool,
    },

    /// Send a keyboard shortcut
    #[command(after_help = "\
        Examples:\n  \
        cu key cmd+c --app 'Google Chrome'    # copy\n  \
        cu key cmd+shift+n --app 'Google Chrome'  # new incognito\n  \
        cu key cmd+space --allow-global       # open Spotlight (system-level)\n  \
        cu key enter --app Safari             # confirm\n  \
        cu key cmd+, --app Finder             # open Preferences\n\n\
        Without --app: refused by default if frontmost is a terminal or IDE\n\
        (stray keys would execute commands). Pass --allow-global to override.\n\n\
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
        /// Skip the frontmost-app safety check. Without --app, cu refuses by default
        /// when the frontmost is a terminal or IDE — pass this to override.
        #[arg(long = "allow-global")]
        allow_global: bool,
    },

    /// List interactive AX elements whose bounding box intersects a screen rectangle
    #[command(after_help = "\
        PREFER:\n  \
        VLM workflow — use this when the agent has narrowed to a region (a dialog,\n  \
        a list area, a toolbar) and wants candidate refs within it.\n  \
        `cu nearest` for single-point closest; `cu find` for role/title-based filtering.\n\n\
        Returns all interactive refs whose bbox overlaps the given rectangle.\n\
        Use this when a VLM agent has identified a region of interest (a dialog,\n\
        a list area) and wants the candidate set of elements within it — narrower\n\
        than `cu snapshot`, more flexible than `cu nearest` (single closest only).\n\n\
        Coordinates and dimensions are in points (same space as snapshot.x/y).\n\n\
        Examples:\n  \
        cu observe-region 480 200 400 300 --app Mail\n  \
        cu observe-region 480 200 400 300 --app Mail --mode center\n\n\
        --mode controls the membership rule:\n  \
        intersect (default) — element bbox overlaps the rect at all\n  \
        center              — element center point is inside the rect (less noise)\n  \
        inside              — element is entirely within the rect (strictest)")]
    ObserveRegion {
        /// Region top-left x (point space)
        x: f64,
        /// Region top-left y
        y: f64,
        /// Region width
        width: f64,
        /// Region height
        height: f64,
        /// Target app
        #[arg(long)]
        app: Option<String>,
        /// AX tree walk limit
        #[arg(long, default_value = "200")]
        limit: usize,
        /// Membership rule: intersect (default) | center | inside
        #[arg(long, default_value = "intersect")]
        mode: String,
    },

    /// Find the AX element nearest to a screen coordinate (for VLM agents that "see" pixels)
    #[command(after_help = "\
        PREFER:\n  \
        VLM workflow — use this when the agent has visual coordinates from a screenshot and\n  \
        wants the closest AX ref instead of clicking by raw pixel (refs survive layout shifts).\n  \
        Use `cu find` instead when you know the role/title; `cu observe-region` for a region.\n\n\
        Given a screen-space (x, y), returns the closest interactive AX element with its\n\
        ref + distance. Use this when a vision-equipped agent has identified WHERE on\n\
        screen something is and needs to translate that to a ref it can act on.\n\n\
        distance = 0 means the point falls inside the element's bounding box.\n\n\
        Examples:\n  \
        cu nearest 480 320 --app Finder\n  \
        cu nearest 480 320 --app Finder --max-distance 50\n  \
        REF=$(cu nearest 480 320 --app Mail | jq -r .match.ref) && cu click $REF --app Mail\n\n\
        With --max-distance: returns null match if nothing within that pixel radius.\n\
        Empty result is `ok:true match:null` — not an error.")]
    Nearest {
        /// Screen-space x coordinate (point, not Retina pixel)
        x: f64,
        /// Screen-space y coordinate
        y: f64,
        /// Target app
        #[arg(long)]
        app: Option<String>,
        /// AX tree walk limit (how many elements to scan)
        #[arg(long, default_value = "200")]
        limit: usize,
        /// Optional max distance in points; null match returned if nothing closer
        #[arg(long = "max-distance")]
        max_distance: Option<f64>,
    },

    /// Find elements matching role/title/value predicates (saves a snapshot+grep round-trip)
    #[command(after_help = "\
        PREFER:\n  \
        Use this over `cu snapshot | grep` whenever you know what you're looking for.\n  \
        Use `--first --raw` to pipe straight into `xargs cu click` (no jq needed).\n\n\
        Predicate query over the AX tree — returns matching elements with refs\n\
        usable directly with `cu click`. Replaces the snapshot+search pattern.\n\n\
        Filters (combine with AND; at least one is required):\n  \
        --role <r>            normalized role (button, textfield, row, cell, ...)\n  \
        --title-contains <s>  case-insensitive substring of title\n  \
        --title-equals <s>    exact title match\n  \
        --value-contains <s>  case-insensitive substring of value\n\n\
        Examples:\n  \
        cu find --app Finder --role row --title-contains Documents\n  \
        cu find --app Safari --role button --title-equals 'Reload' --first\n  \
        cu find --app Mail --role textfield --first | jq -r .match.ref | \\\n    \
          xargs -I{} cu set-value {} 'alice@example.com' --app Mail\n\n\
        --first returns one best match in `.match`; otherwise `.matches` is an array.\n\
        Empty result is not an error — check `.matches | length` (or `.match == null`).")]
    Find {
        /// Target app (default: frontmost)
        #[arg(long)]
        app: Option<String>,
        /// Filter by normalized role (button, textfield, row, ...)
        #[arg(long)]
        role: Option<String>,
        /// Filter by title containing this substring (case-insensitive)
        #[arg(long = "title-contains")]
        title_contains: Option<String>,
        /// Filter by exact title match
        #[arg(long = "title-equals")]
        title_equals: Option<String>,
        /// Filter by value containing this substring (case-insensitive)
        #[arg(long = "value-contains")]
        value_contains: Option<String>,
        /// AX tree walk limit (how many elements to scan)
        #[arg(long, default_value = "200")]
        limit: usize,
        /// Return only the first match in a `.match` field instead of `.matches` array
        #[arg(long)]
        first: bool,
        /// Print just bare ref integer(s), one per line — for `xargs cu click` (no jq needed).
        /// Combined with --first, prints exactly one ref or exits 1 if no match.
        #[arg(long)]
        raw: bool,
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
        /// Wait until window count exceeds the baseline at start (C3)
        #[arg(long)]
        new_window: bool,
        /// Wait until a modal (sheet/dialog) appears (C3)
        #[arg(long)]
        modal: bool,
        /// Wait until the focused element changes from the baseline at start (C3)
        #[arg(long)]
        focused_changed: bool,
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
        cu click --text 'Submit' --app Safari  # by OCR text (finds text on screen)\n  \
        cu click 3 --app WeChat                # verify is on by default (~50-150ms)\n\
        cu click 3 --app Finder --no-verify    # opt out of verify when speed matters\n\n\
        Text mode (--text) uses OCR to find the text, then clicks its center.\n\
        Works for UI elements not in the AX tree (Notification Center, system panels).\n\
        Use --index N to click the Nth match (default: first).\n\n\
        Ref mode tries AX actions first, falls back to CGEvent.\n\
        Always use --app for reliability. Refs come from 'cu snapshot'.\n\n\
        Verify (default ON): takes a pre-action AX snapshot and diffs after the click.\n\
        Returns `verified: bool` + `verify_diff` + remediation `verify_advice`.\n\
        Catches sandboxed apps that silently swallow PID-targeted CGEvents — the\n\
        #1 cause of \"ok=true but the UI didn't change\" agent confusion.")]
    Click {
        /// Element ref number, or x coordinate
        target: Option<String>,
        /// Y coordinate (only when target is x coordinate)
        y: Option<String>,
        /// Find and click on-screen text via OCR
        #[arg(long)]
        text: Option<String>,
        /// Stable element selector (axPath). Survives across snapshots.
        #[arg(long = "ax-path")]
        ax_path: Option<String>,
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
        /// Skip pre/post AX diff. Verify is ON by default to catch silent
        /// failures (sandboxed apps that ignore PID-targeted CGEvents).
        /// Use this when you've measured the verify cost (~50–150ms) and
        /// know the target app is reliable.
        #[arg(long = "no-verify")]
        no_verify: bool,

        /// Deprecated: verify is now the default. Kept as a hidden no-op
        /// for backward compatibility with existing scripts.
        #[arg(long, hide = true)]
        verify: bool,
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
        /// Target app — when set, the scroll is delivered only to that app
        /// (no cursor warp, no focus theft)
        #[arg(long)]
        app: Option<String>,
        /// Skip auto-snapshot in JSON output
        #[arg(long)]
        no_snapshot: bool,
    },

    /// Move mouse to coordinates (trigger tooltips, hover menus)
    #[command(after_help = "Example: cu hover 500 300")]
    Hover {
        x: f64,
        y: f64,
        /// Target app — when set, the move is delivered only to that app
        /// (no cursor warp, no focus theft)
        #[arg(long)]
        app: Option<String>,
        /// Skip auto-snapshot in JSON output
        #[arg(long)]
        no_snapshot: bool,
    },

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
        /// Target app — when set, the drag is delivered only to that app
        /// (no cursor warp, no focus theft)
        #[arg(long)]
        app: Option<String>,
        /// Skip auto-snapshot in JSON output
        #[arg(long)]
        no_snapshot: bool,
    },

    /// Capture window screenshot (silent, no app activation needed)
    #[command(after_help = "\
        Three modes:\n  \
        cu screenshot 'Google Chrome' --path /tmp/chrome.png  # window\n  \
        cu screenshot --full --path /tmp/screen.png           # full screen\n  \
        cu screenshot --region '480,200 400x300' --path /tmp/r.png  # region\n\n\
        Region format: 'x,y WxH' (point space, same as snapshot element coords).\n\
        Region mode is great for VLM verification: instead of re-screenshotting\n\
        a 1920×1200 window (~1500 tokens), grab just the area you care about\n\
        (~150 tokens for a 200×100 region).\n\n\
        All modes return offset_x/offset_y so screen_coord = image_pixel/scale + offset.")]
    Screenshot {
        /// Application name (default: frontmost)
        app: Option<String>,
        /// Output file path (default: /tmp/cu-screenshot-<ts>.png)
        #[arg(long)]
        path: Option<String>,
        /// Capture full screen instead of single window
        #[arg(long)]
        full: bool,
        /// Capture a screen rectangle: "x,y WxH" or "x,y,w,h" (in points).
        /// Overrides --full and the app argument.
        #[arg(long)]
        region: Option<String>,
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

    /// Launch an app and (by default) wait until its first window is ready
    #[command(after_help = "\
        Launches an app via Launch Services. Accepts either an app name\n\
        (e.g. \"TextEdit\") or a bundle id (e.g. \"com.apple.TextEdit\").\n\n\
        By default waits until the app reports a main/focused window via the\n\
        AX tree, polling every 100ms. Pass --no-wait to return immediately.\n\n\
        Examples:\n  \
        cu launch TextEdit                    # launch + wait for window\n  \
        cu launch com.apple.TextEdit          # bundle id form\n  \
        cu launch Calculator --timeout 5      # custom timeout\n  \
        cu launch Mail --no-wait              # spawn-and-go\n\n\
        Returns: { ok, app, pid, ready_in_ms, window: { x, y, width, height } }\n\
        Use this before `cu snapshot` to avoid empty AX trees during startup.")]
    Launch {
        /// App name or bundle identifier
        id: String,
        /// Skip waiting for first window
        #[arg(long)]
        no_wait: bool,
        /// Max seconds to wait for first window
        #[arg(long, default_value = "10")]
        timeout: u64,
    },

    /// Diagnose why a click/perform/set-value targeting a ref might fail
    #[command(after_help = "\
        Run this AFTER a click/perform/set-value returns ok=false (or after a click\n\
        appeared to succeed but the UI didn't change). Returns a structured report:\n\
        whether the ref is in the current snapshot, the element's role/title/coords,\n\
        whether it's inside the focused-window bounds, AXEnabled, AXSubrole, and the\n\
        list of AX actions the element actually supports.\n\n\
        Examples:\n  \
        cu why 17 --app TextEdit       # diagnose ref 17\n  \
        cu why 99 --app Calculator     # for a ref past the snapshot's --limit\n\n\
        Returns: { ok, ref, found, element?, checks, advice, snapshot_size }\n\
        Pair with the next-step suggestions in 'advice' to recover.")]
    Why {
        /// Element ref from the most recent snapshot
        ref_id: usize,
        #[arg(long)]
        app: Option<String>,
        /// Snapshot --limit to use when looking up the ref (default 50)
        #[arg(long, default_value = "50")]
        limit: usize,
    },

    /// Warm up the AX bridge for an already-running app
    #[command(after_help = "\
        Triggers a tiny AX snapshot so the first real `cu snapshot`/`cu click`\n\
        is fast. Useful for apps you didn't launch via `cu launch` — some apps\n\
        (TextEdit, Mail, ...) take 200-500ms on the very first AX walk.\n\n\
        Examples:\n  \
        cu warm Mail            # one-off after the user opened Mail manually\n  \
        cu warm TextEdit        # before a hot loop of clicks\n\n\
        Returns: { ok, app, pid, warmup_ms }")]
    Warm {
        /// App name (must already be running)
        app: String,
    },

    /// Unified state probe — snapshot + windows + screenshot + frontmost in one call
    #[command(after_help = "\
        PREFER:\n  \
        First call when starting a task on a specific app — saves one LLM round-trip\n  \
        vs `cu snapshot` + `cu window list` + `cu screenshot`. The screenshot gives\n  \
        a VLM agent visual context; the snapshot gives ref-based interaction targets.\n\n\
        Examples:\n  \
        cu state Safari                # full state, screenshot to /tmp/cu-state-<ts>.png\n  \
        cu state Mail --no-screenshot  # tree + windows only (faster)\n  \
        cu state TextEdit --limit 100  # deeper tree walk\n\n\
        Returns: { ok, app, pid, frontmost, windows[], displays[], elements[],\n\
                   snapshot_size, tree_truncated, screenshot?, image_scale? }")]
    State {
        /// Application name (must be running)
        app: String,
        /// AX tree depth limit
        #[arg(long, default_value = "50")]
        limit: usize,
        /// Skip the screenshot capture (faster, tree+windows only)
        #[arg(long = "no-screenshot")]
        no_screenshot: bool,
        /// Output path for the window screenshot
        #[arg(long)]
        output: Option<String>,
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

    /// Show curated recipes for high-frequency tasks (e.g. cu examples launch-app)
    #[command(after_help = "\
        Built-in recipe library — short working snippets for common automation patterns.\n\n\
        Examples:\n  \
        cu examples                # list all topics\n  \
        cu examples launch-app     # print the launch-app recipe\n\n\
        When stuck, this is faster than reading SKILL.md end-to-end.")]
    Examples {
        /// Topic name. Run `cu examples` with no arg to list all topics.
        topic: Option<String>,
    },

    /// Execute AppleScript against a scriptable app
    #[command(after_help = "\
        PREFER:\n  \
        Check `cu apps` for the S flag — if the target is scriptable, prefer this over\n  \
        the snapshot+click loop (faster, more reliable, fewer tokens).\n  \
        Fall back to AX (cu snapshot + cu click) when the app has no S flag\n  \
        (Electron, Firefox, custom-render apps) or when the data isn't in the scripting dict.\n\n\
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

    if let Err(e) = dispatch(cli.command, json) {
        if json {
            eprintln!("{}", e.to_json());
        } else {
            eprintln!("Error: {}", e.error);
            if let Some(h) = &e.hint {
                eprintln!("Hint: {h}");
            }
            for s in &e.suggested_next {
                eprintln!("Try : {s}");
            }
        }
        std::process::exit(1);
    }
}

fn dispatch(cmd: Cmd, json: bool) -> Result<(), CuError> {
    match cmd {
        Cmd::Setup => cmd_setup(json),
        Cmd::Apps => cmd_apps(json),
        Cmd::Snapshot {
            app,
            limit,
            diff,
            annotated,
            with_screenshot,
            output,
        } => cmd_snapshot(json, app, limit, diff, annotated, with_screenshot, output),
        Cmd::ObserveRegion {
            x,
            y,
            width,
            height,
            app,
            limit,
            mode,
        } => cmd_observe_region(json, x, y, width, height, app, limit, mode),
        Cmd::Nearest {
            x,
            y,
            app,
            limit,
            max_distance,
        } => cmd_nearest(json, x, y, app, limit, max_distance),
        Cmd::Find {
            app,
            role,
            title_contains,
            title_equals,
            value_contains,
            limit,
            first,
            raw,
        } => cmd_find(
            json,
            app,
            role,
            title_contains,
            title_equals,
            value_contains,
            limit,
            first,
            raw,
        ),
        Cmd::Wait {
            text,
            ref_id,
            gone,
            new_window,
            modal,
            focused_changed,
            app,
            timeout,
            limit,
        } => cmd_wait(
            json,
            text,
            ref_id,
            gone,
            new_window,
            modal,
            focused_changed,
            app,
            timeout,
            limit,
        ),
        Cmd::Ocr { app } => cmd_ocr(json, app),
        Cmd::Type {
            text,
            app,
            no_snapshot,
            allow_global,
            paste,
            no_paste,
        } => cmd_type(json, text, app, no_snapshot, allow_global, paste, no_paste),
        Cmd::SetValue {
            ref_id,
            value,
            app,
            limit,
            ax_path,
            no_snapshot,
        } => cmd_set_value(json, ref_id, value, app, limit, ax_path, no_snapshot),
        Cmd::Perform {
            ref_id,
            action,
            app,
            limit,
            ax_path,
            no_snapshot,
        } => cmd_perform(json, ref_id, action, app, limit, ax_path, no_snapshot),
        Cmd::Key {
            combo,
            app,
            no_snapshot,
            allow_global,
        } => cmd_key(json, combo, app, no_snapshot, allow_global),
        Cmd::Click {
            target,
            y,
            text,
            ax_path,
            index,
            app,
            limit,
            right,
            double_click,
            shift,
            cmd,
            alt,
            no_snapshot,
            no_verify,
            verify: _legacy_verify,
        } => {
            let mods = mouse::Modifiers {
                shift,
                cmd,
                alt,
                ctrl: false,
            };
            // Verify is ON by default (R2). --no-snapshot disables it
            // mechanically — verify needs the post-action snapshot to
            // diff against. The deprecated `--verify` flag is accepted
            // and ignored (already the default behavior).
            let verify = !no_verify && !no_snapshot;
            cmd_click(ClickOptions {
                json,
                target,
                y,
                text,
                ax_path,
                index,
                app,
                limit,
                right,
                double: double_click,
                mods,
                no_snapshot,
                verify,
            })
        }
        Cmd::Scroll {
            direction,
            amount,
            x,
            y,
            app,
            no_snapshot,
        } => cmd_scroll(json, direction, amount, x, y, app, no_snapshot),
        Cmd::Hover {
            x,
            y,
            app,
            no_snapshot,
        } => cmd_hover(json, x, y, app, no_snapshot),
        Cmd::Drag {
            x1,
            y1,
            x2,
            y2,
            shift,
            cmd,
            alt,
            app,
            no_snapshot,
        } => {
            let mods = mouse::Modifiers {
                shift,
                cmd,
                alt,
                ctrl: false,
            };
            cmd_drag(json, x1, y1, x2, y2, mods, app, no_snapshot)
        }
        Cmd::Screenshot {
            app,
            path,
            full,
            region,
        } => cmd_screenshot(json, app, path, full, region),
        Cmd::Window {
            action,
            arg1,
            arg2,
            app,
            window,
        } => cmd_window(json, action, arg1, arg2, app, window),
        Cmd::Launch {
            id,
            no_wait,
            timeout,
        } => cmd_launch(json, id, no_wait, timeout),
        Cmd::Warm { app } => cmd_warm(json, app),
        Cmd::Why { ref_id, app, limit } => cmd_why(json, ref_id, app, limit),
        Cmd::State {
            app,
            limit,
            no_screenshot,
            output,
        } => cmd_state(json, app, limit, no_screenshot, output),
        Cmd::Menu { app } => cmd_menu(json, app),
        Cmd::Defaults {
            action,
            domain,
            key,
            value,
        } => cmd_defaults(json, action, domain, key, value),
        Cmd::Sdef { app } => cmd_sdef(json, app),
        Cmd::Examples { topic } => cmd_examples(json, topic),
        Cmd::Tell { app, expr, timeout } => cmd_tell(json, app, expr, timeout),
    }
}

// ── Commands ────────────────────────────────────────────────────────────────

fn cmd_setup(json: bool) -> Result<(), CuError> {
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

fn cmd_apps(json: bool) -> Result<(), CuError> {
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

#[allow(clippy::too_many_arguments)]
fn cmd_snapshot(
    json: bool,
    app: Option<String>,
    limit: usize,
    diff_mode: bool,
    annotated: bool,
    with_screenshot: bool,
    output: Option<String>,
) -> Result<(), CuError> {
    let (pid, name) = system::resolve_target_app(&app)?;
    let result = ax::snapshot(pid, &name, limit);
    if !result.ok {
        return Err(result
            .error
            .unwrap_or_else(|| "snapshot failed".into())
            .into());
    }

    // --with-screenshot (skipped when --annotated is set since annotated already
    // bakes a screenshot into the response).
    let plain_screenshot: Option<(String, f64)> = if with_screenshot && !annotated {
        let win =
            screenshot::find_window(pid).ok_or("no on-screen window found for the target app")?;
        let path = output.clone().unwrap_or_else(|| {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            format!("/tmp/cu-snapshot-{ts}.png")
        });
        let scale = screenshot::capture_window_with_scale(&win, &path)?;
        Some((path, scale))
    } else {
        None
    };

    // --annotated: capture window + draw ref boxes/labels, attach path to JSON output.
    let annotated_info: Option<(String, f64)> = if annotated {
        let win =
            screenshot::find_window(pid).ok_or("no on-screen window found for the target app")?;
        let path = output.clone().unwrap_or_else(|| {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            format!("/tmp/cu-annotated-{ts}.png")
        });
        let anns: Vec<screenshot::Annotation> = result
            .elements
            .iter()
            .map(|e| screenshot::Annotation {
                ref_id: e.ref_id,
                x: e.x,
                y: e.y,
                width: e.width,
                height: e.height,
            })
            .collect();
        let scale = screenshot::annotate_window(&win, &anns, &path)?;
        Some((path, scale))
    } else {
        None
    };

    if !diff_mode {
        if json {
            // D1: always attach the active displays list so the agent can resolve
            // (x,y) → screen for any element in the snapshot.
            let mut full = serde_json::to_value(&result).unwrap_or_default();
            full["displays"] = serde_json::to_value(display::list()).unwrap_or_default();
            if let Some((path, scale)) = &annotated_info {
                full["annotated_screenshot"] = serde_json::json!(path);
                full["image_scale"] = serde_json::json!(scale);
            } else if let Some((path, scale)) = &plain_screenshot {
                full["screenshot"] = serde_json::json!(path);
                full["image_scale"] = serde_json::json!(scale);
            }
            emit(&full);
        } else {
            print_snapshot_human(&result);
            if let Some((path, _)) = &annotated_info {
                println!("Annotated screenshot: {path}");
            } else if let Some((path, _)) = &plain_screenshot {
                println!("Screenshot: {path}");
            }
        }
        return Ok(());
    }

    // Diff mode: compare against the cached previous snapshot for this pid.
    let previous = diff::load_previous(pid);
    let _ = diff::save_current(pid, &result.elements);

    match previous {
        None => {
            // First call: no diff possible — return full snapshot with a flag.
            if json {
                let mut full = serde_json::to_value(&result).unwrap_or_default();
                full["first_snapshot"] = serde_json::Value::Bool(true);
                full["displays"] = serde_json::to_value(display::list()).unwrap_or_default();
                if let Some((path, scale)) = &annotated_info {
                    full["annotated_screenshot"] = serde_json::json!(path);
                    full["image_scale"] = serde_json::json!(scale);
                } else if let Some((path, scale)) = &plain_screenshot {
                    full["screenshot"] = serde_json::json!(path);
                    full["image_scale"] = serde_json::json!(scale);
                }
                emit(&full);
            } else {
                println!("(first snapshot for pid {pid} — no diff yet)");
                print_snapshot_human(&result);
                if let Some((path, _)) = &annotated_info {
                    println!("Annotated screenshot: {path}");
                } else if let Some((path, _)) = &plain_screenshot {
                    println!("Screenshot: {path}");
                }
            }
            Ok(())
        }
        Some(prev) => {
            let d = diff::diff(&prev, &result.elements);
            if json {
                let mut body = serde_json::json!({
                    "ok": true,
                    "app": result.app,
                    "window": result.window,
                    "window_frame": result.window_frame,
                    "focused": result.focused,
                    "modal": result.modal,
                    "diff": d,
                    "limit": limit,
                    "truncated": result.truncated,
                    "displays": display::list(),
                });
                if let Some((path, scale)) = &annotated_info {
                    body["annotated_screenshot"] = serde_json::json!(path);
                    body["image_scale"] = serde_json::json!(scale);
                } else if let Some((path, scale)) = &plain_screenshot {
                    body["screenshot"] = serde_json::json!(path);
                    body["image_scale"] = serde_json::json!(scale);
                }
                emit(&body);
            } else {
                print_diff_human(&result, &d);
                if let Some((path, _)) = &annotated_info {
                    println!("Annotated screenshot: {path}");
                } else if let Some((path, _)) = &plain_screenshot {
                    println!("Screenshot: {path}");
                }
            }
            Ok(())
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_observe_region(
    json: bool,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    app: Option<String>,
    limit: usize,
    mode: String,
) -> Result<(), CuError> {
    if !x.is_finite() || !y.is_finite() || !width.is_finite() || !height.is_finite() {
        return Err("region coordinates must be finite numbers".into());
    }
    if width <= 0.0 || height <= 0.0 {
        return Err("region width and height must be > 0".into());
    }
    let mode_lc = mode.to_lowercase();
    if mode_lc != "intersect" && mode_lc != "center" && mode_lc != "inside" {
        return Err(CuError::msg(format!("unknown --mode: {mode}"))
            .with_hint("use one of: intersect, center, inside")
            .with_next("cu observe-region <x> <y> <w> <h> --app <Name> --mode intersect"));
    }

    let (pid, name) = system::resolve_target_app(&app)?;
    let snap = ax::snapshot(pid, &name, limit);
    if !snap.ok {
        return Err(snap
            .error
            .unwrap_or_else(|| "snapshot failed".into())
            .into());
    }

    let rx0 = x;
    let ry0 = y;
    let rx1 = x + width;
    let ry1 = y + height;

    let matches: Vec<&ax::Element> = snap
        .elements
        .iter()
        .filter(|e| {
            let ex0 = e.x;
            let ey0 = e.y;
            let ex1 = e.x + e.width;
            let ey1 = e.y + e.height;
            match mode_lc.as_str() {
                "inside" => ex0 >= rx0 && ey0 >= ry0 && ex1 <= rx1 && ey1 <= ry1,
                "center" => {
                    let cx = e.x + e.width / 2.0;
                    let cy = e.y + e.height / 2.0;
                    cx >= rx0 && cx < rx1 && cy >= ry0 && cy < ry1
                }
                _ => {
                    // intersect: bboxes overlap (touching counts as no-overlap)
                    !(ex1 <= rx0 || ex0 >= rx1 || ey1 <= ry0 || ey0 >= ry1)
                }
            }
        })
        .collect();

    if json {
        let body = serde_json::json!({
            "ok": true,
            "app": name,
            "region": {"x": x, "y": y, "width": width, "height": height},
            "mode": mode_lc,
            "matches": matches,
            "count": matches.len(),
            "scanned": snap.elements.len(),
            "truncated": snap.truncated,
        });
        ok(body)
    } else {
        if matches.is_empty() {
            println!(
                "No elements in region ({},{} {}×{}) under mode={mode_lc}.",
                x, y, width, height
            );
        } else {
            for el in &matches {
                let label = el.title.as_deref().or(el.value.as_deref()).unwrap_or("");
                println!(
                    "[{}] {} \"{}\" ({},{} {}×{})",
                    el.ref_id, el.role, label, el.x, el.y, el.width, el.height
                );
            }
        }
        Ok(())
    }
}

fn cmd_nearest(
    json: bool,
    x: f64,
    y: f64,
    app: Option<String>,
    limit: usize,
    max_distance: Option<f64>,
) -> Result<(), CuError> {
    if !x.is_finite() || !y.is_finite() {
        return Err("coordinates must be finite numbers".into());
    }
    let (pid, name) = system::resolve_target_app(&app)?;
    let snap = ax::snapshot(pid, &name, limit);
    if !snap.ok {
        return Err(snap
            .error
            .unwrap_or_else(|| "snapshot failed".into())
            .into());
    }

    // Distance from (x, y) to each element's bounding box (0 if point is inside).
    let mut best: Option<(f64, &ax::Element)> = None;
    for el in &snap.elements {
        let cx = el.x.max(x.min(el.x + el.width));
        let cy = el.y.max(y.min(el.y + el.height));
        let dx = x - cx;
        let dy = y - cy;
        let dist = (dx * dx + dy * dy).sqrt();
        match best {
            None => best = Some((dist, el)),
            Some((d, _)) if dist < d => best = Some((dist, el)),
            _ => {}
        }
    }

    let scanned = snap.elements.len();
    let truncated = snap.truncated;

    let pick = best.and_then(|(dist, el)| {
        if let Some(max) = max_distance
            && dist > max
        {
            None
        } else {
            Some((dist, el))
        }
    });

    if json {
        let body = match pick {
            Some((dist, el)) => {
                let inside = dist == 0.0;
                serde_json::json!({
                    "ok": true,
                    "app": name,
                    "match": {
                        "ref": el.ref_id,
                        "role": el.role,
                        "title": el.title,
                        "value": el.value,
                        "x": el.x, "y": el.y, "width": el.width, "height": el.height,
                        "distance": dist,
                        "inside": inside,
                    },
                    "query": {"x": x, "y": y},
                    "scanned": scanned,
                    "truncated": truncated,
                    "max_distance": max_distance,
                })
            }
            None => serde_json::json!({
                "ok": true,
                "app": name,
                "match": serde_json::Value::Null,
                "query": {"x": x, "y": y},
                "scanned": scanned,
                "truncated": truncated,
                "max_distance": max_distance,
            }),
        };
        ok(body)
    } else {
        match pick {
            Some((dist, el)) => {
                let label = el.title.as_deref().or(el.value.as_deref()).unwrap_or("");
                let inside = if dist == 0.0 { " (inside)" } else { "" };
                println!(
                    "[{}] {} \"{}\" ({},{} {}×{}) — distance {:.1}{inside}",
                    el.ref_id, el.role, label, el.x, el.y, el.width, el.height, dist
                );
            }
            None => {
                println!("No element within {:?} of ({}, {}).", max_distance, x, y);
            }
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_find(
    json: bool,
    app: Option<String>,
    role: Option<String>,
    title_contains: Option<String>,
    title_equals: Option<String>,
    value_contains: Option<String>,
    limit: usize,
    first: bool,
    raw: bool,
) -> Result<(), CuError> {
    if role.is_none()
        && title_contains.is_none()
        && title_equals.is_none()
        && value_contains.is_none()
    {
        return Err(CuError::msg("specify at least one filter")
            .with_hint("use --role, --title-contains, --title-equals, or --value-contains")
            .with_next("cu find --app <Name> --role button --title-contains Save"));
    }

    let (pid, name) = system::resolve_target_app(&app)?;
    let snap = ax::snapshot(pid, &name, limit);
    if !snap.ok {
        return Err(snap
            .error
            .unwrap_or_else(|| "snapshot failed".into())
            .into());
    }

    let role_filter = role.as_deref().map(|r| r.to_lowercase());
    let title_contains_lc = title_contains.as_deref().map(|s| s.to_lowercase());
    let value_contains_lc = value_contains.as_deref().map(|s| s.to_lowercase());

    let matches: Vec<&ax::Element> = snap
        .elements
        .iter()
        .filter(|e| {
            if let Some(ref r) = role_filter
                && &e.role != r
            {
                return false;
            }
            if let Some(ref needle) = title_contains_lc {
                let hay = e.title.as_deref().unwrap_or("").to_lowercase();
                if !hay.contains(needle) {
                    return false;
                }
            }
            if let Some(ref exact) = title_equals
                && e.title.as_deref().unwrap_or("") != exact
            {
                return false;
            }
            if let Some(ref needle) = value_contains_lc {
                let hay = e.value.as_deref().unwrap_or("").to_lowercase();
                if !hay.contains(needle) {
                    return false;
                }
            }
            true
        })
        .collect();

    // --raw: bypass JSON/human entirely and emit bare ref integers (one per line)
    // for direct piping to `xargs cu click`. Empty result → exit 1.
    if raw {
        let picks: Vec<&ax::Element> = if first {
            matches.iter().take(1).copied().collect()
        } else {
            matches.clone()
        };
        if picks.is_empty() {
            std::process::exit(1);
        }
        for el in picks {
            println!("{}", el.ref_id);
        }
        return Ok(());
    }

    if json {
        let scanned = snap.elements.len();
        let truncated = snap.truncated;
        if first {
            let m = matches.first().copied();
            let body = serde_json::json!({
                "ok": true,
                "app": name,
                "match": m,
                "count": matches.len(),
                "scanned": scanned,
                "truncated": truncated,
            });
            ok(body)
        } else {
            let body = serde_json::json!({
                "ok": true,
                "app": name,
                "matches": matches,
                "count": matches.len(),
                "scanned": scanned,
                "truncated": truncated,
            });
            ok(body)
        }
    } else {
        if matches.is_empty() {
            println!("No matches (scanned {} elements).", snap.elements.len());
        } else {
            for el in &matches {
                let label = el.title.as_deref().or(el.value.as_deref()).unwrap_or("");
                println!(
                    "[{}] {} \"{}\" ({},{} {}×{})",
                    el.ref_id, el.role, label, el.x, el.y, el.width, el.height
                );
            }
            if first && matches.len() > 1 {
                println!(
                    "  … {} more match(es); --first picks [{}]",
                    matches.len() - 1,
                    matches[0].ref_id
                );
            }
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_wait(
    json: bool,
    text: Option<String>,
    ref_id: Option<usize>,
    gone: Option<usize>,
    new_window: bool,
    modal: bool,
    focused_changed: bool,
    app: Option<String>,
    timeout: u64,
    limit: usize,
) -> Result<(), CuError> {
    let condition = if let Some(t) = text {
        wait::Condition::Text(t)
    } else if let Some(r) = ref_id {
        wait::Condition::Ref(r)
    } else if let Some(g) = gone {
        wait::Condition::Gone(g)
    } else if new_window {
        wait::Condition::NewWindow
    } else if modal {
        wait::Condition::Modal
    } else if focused_changed {
        wait::Condition::FocusedChanged
    } else {
        return Err(
            "specify one of: --text, --ref, --gone, --new-window, --modal, --focused-changed"
                .into(),
        );
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

fn cmd_ocr(json: bool, app: Option<String>) -> Result<(), CuError> {
    let (pid, _name) = system::resolve_target_app(&app)?;
    let result = ocr::recognize(pid);

    if !result.ok {
        return Err(result.error.unwrap_or_else(|| "OCR failed".into()).into());
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

/// Apps known to drop the first character of PID-targeted unicode CGEvents.
/// All CEF/Electron-based chat clients exhibit the bug because their text
/// input field is a webview and the first synthetic event reaches the
/// pre-init JS layer. Hardcoded list is conservative — adding a false
/// positive only forces clipboard paste, which still works.
const PASTE_APPS: &[&str] = &[
    "WeChat", "微信",
    "Slack",
    "Discord",
    "Telegram",
    "QQ", "TIM",
    "Lark", "飞书", "Feishu",
    "DingTalk", "钉钉",
    "WhatsApp",
    "Signal",
];

fn contains_cjk(text: &str) -> bool {
    text.chars().any(|c| {
        let n = c as u32;
        // CJK Unified Ideographs + Extension A, Hiragana, Katakana, Hangul,
        // CJK Symbols and Punctuation, Halfwidth/Fullwidth forms.
        matches!(n,
            0x3000..=0x303F |   // CJK symbols
            0x3040..=0x309F |   // Hiragana
            0x30A0..=0x30FF |   // Katakana
            0x3400..=0x4DBF |   // CJK Ext A
            0x4E00..=0x9FFF |   // CJK Unified Ideographs
            0xAC00..=0xD7AF |   // Hangul
            0xFF00..=0xFFEF     // Halfwidth and Fullwidth Forms
        )
    })
}

/// Returns Some(reason) when the type call should auto-route through paste.
/// `reason` is surfaced in the JSON output so the agent can see why.
fn should_auto_paste(text: &str, app: &Option<String>) -> Option<String> {
    if contains_cjk(text) {
        return Some("text contains CJK characters (unicode events drop first char in CEF/Electron)".into());
    }
    if let Some(name) = app
        && PASTE_APPS.iter().any(|known| name.eq_ignore_ascii_case(known) || name.contains(*known))
    {
        return Some(format!("target app '{name}' is in the paste list (CEF chat apps drop unicode events)"));
    }
    None
}

fn cmd_type(
    json: bool,
    text: String,
    app: Option<String>,
    no_snapshot: bool,
    allow_global: bool,
    paste: bool,
    no_paste: bool,
) -> Result<(), CuError> {
    // With --app: deliver Unicode events directly to the target pid (no focus
    // theft, no clipboard pollution, IME bypassed). Without --app: events go
    // to whatever app is frontmost via the global HID tap.
    let target_pid = if app.is_some() {
        Some(system::resolve_target_app(&app)?.0)
    } else {
        if !allow_global {
            system::check_global_frontmost_safety("type")?;
        }
        None
    };

    // R7: auto-route via paste when the text contains CJK or the target is
    // a chat app known to drop unicode events. --paste forces on, --no-paste
    // forces off, otherwise auto.
    let (use_paste, paste_reason) = if paste {
        (true, Some("explicit --paste".to_string()))
    } else if no_paste {
        (false, None)
    } else {
        let auto_reason = should_auto_paste(&text, &app);
        (auto_reason.is_some(), auto_reason)
    };

    let method = if use_paste {
        key::type_via_paste(&text, target_pid)?;
        if target_pid.is_some() { "paste-pid" } else { "paste-global" }
    } else {
        key::type_text(&text, target_pid)?;
        if target_pid.is_some() { "unicode-pid" } else { "unicode-global" }
    };
    let mut result = serde_json::json!({"ok": true, "text": text, "method": method});
    if let Some(reason) = paste_reason {
        result["paste_reason"] = serde_json::Value::String(reason);
    }
    maybe_attach_snapshot(&mut result, json, no_snapshot, &app, 50);
    if json {
        ok(result)
    } else {
        println!("Typed: \"{text}\"");
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_set_value(
    json: bool,
    ref_id: Option<usize>,
    value: String,
    app: Option<String>,
    limit: usize,
    ax_path: Option<String>,
    no_snapshot: bool,
) -> Result<(), CuError> {
    if ref_id.is_none() && ax_path.is_none() {
        return Err("provide either <ref> or --ax-path".into());
    }
    if let Some(r) = ref_id
        && r == 0
    {
        return Err("ref must be >= 1".into());
    }
    let (pid, name) = system::resolve_target_app(&app)?;

    let (selector_kind, selector_value) = if let Some(p) = ax_path.as_deref() {
        ax::ax_set_value_by_path(pid, p, &value)?;
        ("ax-path", serde_json::Value::String(p.to_string()))
    } else {
        let r = ref_id.unwrap();
        ax::ax_set_value(pid, r, limit, &value)?;
        ("ref", serde_json::Value::Number(r.into()))
    };

    let mut result = serde_json::json!({
        "ok": true,
        selector_kind: selector_value,
        "app": name,
        "value": value,
        "method": "ax-set-value",
    });
    maybe_attach_snapshot(&mut result, json, no_snapshot, &app, limit);
    if json {
        ok(result)
    } else {
        let label = ref_id
            .map(|r| format!("[{r}]"))
            .unwrap_or_else(|| ax_path.clone().unwrap_or_default());
        println!("Set {label} AXValue = \"{value}\"");
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_perform(
    json: bool,
    ref_id: Option<usize>,
    action: String,
    app: Option<String>,
    limit: usize,
    ax_path: Option<String>,
    no_snapshot: bool,
) -> Result<(), CuError> {
    if ref_id.is_none() && ax_path.is_none() {
        return Err("provide either <ref> or --ax-path".into());
    }
    if let Some(r) = ref_id
        && r == 0
    {
        return Err("ref must be >= 1".into());
    }
    let (pid, name) = system::resolve_target_app(&app)?;

    let (selector_kind, selector_value, available) = if let Some(p) = ax_path.as_deref() {
        // Resolve via axPath, fire AXAction directly. We piggyback on
        // resolve_by_ax_path to validate the path first; then list actions
        // and perform the named one via a focused AX descent.
        let (_acted, _cx, _cy) = ax::resolve_by_ax_path(pid, p, false)?;
        // For axPath we don't currently re-list actions — pass an empty list.
        // The action attempt itself will fail with a clear error if not supported.
        ax::ax_perform_by_path(pid, p, &action)?;
        (
            "ax-path",
            serde_json::Value::String(p.to_string()),
            Vec::<String>::new(),
        )
    } else {
        let r = ref_id.unwrap();
        let avail = ax::ax_perform(pid, r, limit, &action)?;
        ("ref", serde_json::Value::Number(r.into()), avail)
    };

    let mut result = serde_json::json!({
        "ok": true,
        selector_kind: selector_value,
        "app": name,
        "action": action,
        "method": "ax-perform",
        "available_actions": available,
    });
    maybe_attach_snapshot(&mut result, json, no_snapshot, &app, limit);
    if json {
        ok(result)
    } else {
        let label = ref_id
            .map(|r| format!("[{r}]"))
            .unwrap_or_else(|| ax_path.clone().unwrap_or_default());
        println!("Performed {label} {action}");
        Ok(())
    }
}

fn cmd_key(
    json: bool,
    combo: String,
    app: Option<String>,
    no_snapshot: bool,
    allow_global: bool,
) -> Result<(), CuError> {
    // With --app: PID-targeted (no focus theft). Without --app: global HID tap.
    let target_pid = if app.is_some() {
        Some(system::resolve_target_app(&app)?.0)
    } else {
        if !allow_global {
            system::check_global_frontmost_safety("send keys")?;
        }
        None
    };
    key::send(&combo, target_pid)?;
    let method = if target_pid.is_some() {
        "key-pid"
    } else {
        "key-global"
    };
    let mut result = serde_json::json!({"ok": true, "combo": combo, "method": method});
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
    ax_path: Option<String>,
    index: usize,
    app: Option<String>,
    limit: usize,
    right: bool,
    double: bool,
    mods: mouse::Modifiers,
    no_snapshot: bool,
    verify: bool,
}

fn cmd_click(opts: ClickOptions) -> Result<(), CuError> {
    let ClickOptions {
        json,
        target,
        y,
        text,
        ax_path,
        index,
        app,
        limit,
        right,
        double,
        mods,
        no_snapshot,
        verify,
    } = opts;

    // --verify: capture the AX state before any action so we can diff it
    // against the post-action snapshot. We resolve the app once here and stash
    // the (pid, name, prev_elements) tuple; downstream code paths read it back
    // through `attach_verification` after maybe_attach_snapshot has populated
    // result["snapshot"].
    let pre_state: Option<(i32, String, Vec<ax::Element>)> = if verify {
        match system::resolve_target_app(&app) {
            Ok((pid, name)) => {
                let snap = ax::snapshot(pid, &name, limit);
                if snap.ok {
                    Some((pid, name, snap.elements))
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    } else {
        None
    };

    // Mode 0: --ax-path → resolve via stable selector, then dispatch to AX
    // action chain (or CGEvent fallback). Same code path as ref click but the
    // resolver is path-based instead of DFS-counter-based.
    if let Some(p) = ax_path.as_deref() {
        let (pid, name) = system::resolve_target_app(&app)?;
        let target_pid = Some(pid);
        let (method, cx, cy) = if right || double {
            let (_, cx, cy) = ax::resolve_by_ax_path(pid, p, false)?;
            if double {
                mouse::double_click(cx, cy, mods, target_pid)?;
                ("double-click-pid", cx, cy)
            } else {
                mouse::click(cx, cy, true, mods, target_pid)?;
                ("cgevent-right-pid", cx, cy)
            }
        } else {
            let (ax_acted, cx, cy) = ax::resolve_by_ax_path(pid, p, true)?;
            if !ax_acted {
                mouse::click(cx, cy, false, mods, target_pid)?;
                ("cgevent-pid", cx, cy)
            } else {
                ("ax-action", cx, cy)
            }
        };
        let mut result = serde_json::json!({
            "ok": true, "ax_path": p, "app": name, "method": method, "x": cx, "y": cy
        });
        maybe_attach_snapshot(&mut result, json, no_snapshot, &app, limit);
        if let Some((_, _, ref prev)) = pre_state {
            attach_verification(&mut result, prev, method);
        }
        return if json {
            ok(result)
        } else {
            println!("Clicked '{p}' via {method} at ({cx}, {cy})");
            Ok(())
        };
    }

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
            return Err(result.error.unwrap_or_else(|| "OCR failed".into()).into());
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
            )
            .into());
        }
        if index == 0 || index > matches.len() {
            return Err(format!(
                "--index {} out of range (found {} matches for \"{}\")",
                index,
                matches.len(),
                search_text
            )
            .into());
        }

        let matched = matches[index - 1];
        let cx = matched.x + matched.width / 2.0;
        let cy = matched.y + matched.height / 2.0;

        // Route to the target app's pid when --app was given; falls back to global
        // delivery for full-screen OCR (pid == 0 sentinel from resolve above).
        let target_pid = if pid != 0 { Some(pid) } else { None };
        if double {
            mouse::double_click(cx, cy, mods, target_pid)?;
        } else {
            mouse::click(cx, cy, right, mods, target_pid)?;
        }

        let method = if target_pid.is_some() {
            "ocr-text-pid"
        } else {
            "ocr-text-global"
        };
        let mut result = serde_json::json!({
            "ok": true, "method": method, "text": matched.text,
            "x": cx, "y": cy, "matches": matches.len()
        });
        maybe_attach_snapshot(&mut result, json, no_snapshot, &app, limit);
        if let Some((_, _, ref prev)) = pre_state {
            attach_verification(&mut result, prev, method);
        }
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
        // When --app is given we know the target pid, so route the event directly
        // to that process. Without --app the click goes through the global HID tap.
        let target_pid = if app.is_some() {
            Some(system::resolve_target_app(&app)?.0)
        } else {
            None
        };
        if double {
            mouse::double_click(x, y, mods, target_pid)?;
        } else {
            mouse::click(x, y, right, mods, target_pid)?;
        }
        let method = match (target_pid.is_some(), double, right) {
            (true, true, _) => "double-click-pid",
            (false, true, _) => "double-click-global",
            (true, false, true) => "cgevent-right-pid",
            (false, false, true) => "cgevent-right-global",
            (true, false, false) => "cgevent-pid",
            (false, false, false) => "cgevent-global",
        };
        let mut result =
            serde_json::json!({"ok": true, "method": method, "x": x, "y": y, "right": right});
        maybe_attach_snapshot(&mut result, json, no_snapshot, &app, limit);
        if let Some((_, _, ref prev)) = pre_state {
            attach_verification(&mut result, prev, method);
        }
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

    // Mode 3 always knows the target pid, so all CGEvent fallbacks are PID-targeted —
    // no cursor warp, no focus theft.
    let target_pid = Some(pid);
    let (method, cx, cy) = if right || double {
        let (_, cx, cy) = ax::ax_find_element(pid, ref_id, limit)?;
        if double {
            mouse::double_click(cx, cy, mods, target_pid)?;
            ("double-click-pid", cx, cy)
        } else {
            mouse::click(cx, cy, true, mods, target_pid)?;
            ("cgevent-right-pid", cx, cy)
        }
    } else {
        let (ax_acted, cx, cy) = ax::ax_click(pid, ref_id, limit)?;
        if !ax_acted {
            mouse::click(cx, cy, false, mods, target_pid)?;
            ("cgevent-pid", cx, cy)
        } else {
            ("ax-action", cx, cy)
        }
    };

    let mut result = serde_json::json!({"ok": true, "ref": ref_id, "app": name, "method": method, "x": cx, "y": cy});
    maybe_attach_snapshot(&mut result, json, no_snapshot, &app, limit);
    if let Some((_, _, ref prev)) = pre_state {
        attach_verification(&mut result, prev, method);
    }
    if json {
        ok(result)
    } else {
        println!("Clicked [{ref_id}] via {method} at ({cx}, {cy})");
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_scroll(
    json: bool,
    direction: String,
    amount: i32,
    x: Option<f64>,
    y: Option<f64>,
    app: Option<String>,
    no_snapshot: bool,
) -> Result<(), CuError> {
    let (dx, dy) = match direction.to_lowercase().as_str() {
        "up" => (0, amount),
        "down" => (0, -amount),
        "left" => (-amount, 0),
        "right" => (amount, 0),
        other => {
            return Err(format!("unknown direction: {other} (use: up, down, left, right)").into());
        }
    };
    let sx = x.ok_or("--x is required for scroll")?;
    let sy = y.ok_or("--y is required for scroll")?;
    let target_pid = if app.is_some() {
        Some(system::resolve_target_app(&app)?.0)
    } else {
        None
    };
    mouse::scroll(sx, sy, dy, dx, target_pid)?;
    let method = if target_pid.is_some() {
        "cgevent-pid"
    } else {
        "cgevent-global"
    };
    let mut result = serde_json::json!({
        "ok": true, "method": method, "direction": direction,
        "amount": amount, "x": sx, "y": sy,
    });
    maybe_attach_snapshot(&mut result, json, no_snapshot, &app, 50);
    if json {
        ok(result)
    } else {
        println!("Scrolled {direction} {amount} at ({sx}, {sy})");
        Ok(())
    }
}

fn cmd_hover(
    json: bool,
    x: f64,
    y: f64,
    app: Option<String>,
    no_snapshot: bool,
) -> Result<(), CuError> {
    let target_pid = if app.is_some() {
        Some(system::resolve_target_app(&app)?.0)
    } else {
        None
    };
    mouse::hover(x, y, target_pid)?;
    let method = if target_pid.is_some() {
        "cgevent-pid"
    } else {
        "cgevent-global"
    };
    let mut result = serde_json::json!({"ok": true, "method": method, "x": x, "y": y});
    maybe_attach_snapshot(&mut result, json, no_snapshot, &app, 50);
    if json {
        ok(result)
    } else {
        println!("Hover at ({x}, {y})");
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_drag(
    json: bool,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    mods: mouse::Modifiers,
    app: Option<String>,
    no_snapshot: bool,
) -> Result<(), CuError> {
    let target_pid = if app.is_some() {
        Some(system::resolve_target_app(&app)?.0)
    } else {
        None
    };
    mouse::drag(x1, y1, x2, y2, mods, target_pid)?;
    let method = if target_pid.is_some() {
        "cgevent-pid"
    } else {
        "cgevent-global"
    };
    let mut result = serde_json::json!({
        "ok": true, "method": method,
        "from": {"x": x1, "y": y1}, "to": {"x": x2, "y": y2},
    });
    maybe_attach_snapshot(&mut result, json, no_snapshot, &app, 50);
    if json {
        ok(result)
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
    region: Option<String>,
) -> Result<(), CuError> {
    let output_path = path.unwrap_or_else(|| {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("/tmp/cu-screenshot-{ts}.png")
    });

    if let Some(spec) = region {
        let (rx, ry, rw, rh) = parse_region(&spec)?;
        screenshot::capture_region(rx, ry, rw, rh, &output_path)?;
        return if json {
            ok(serde_json::json!({
                "ok": true, "path": output_path, "mode": "region",
                "offset_x": rx, "offset_y": ry, "width": rw, "height": rh
            }))
        } else {
            println!("Screenshot saved: {output_path} (region {rw}×{rh} at {rx},{ry})");
            Ok(())
        };
    }

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
    screenshot::capture_window(&win, &output_path)?;

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
) -> Result<(), CuError> {
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
        // B6: prefer direct AX raise for focus — non-disruptive, no global activate.
        let method = if action == "focus" {
            let (pid, _) = system::resolve_target_app(&Some(app_name.clone()))?;
            if ax::raise_window(pid) {
                "ax-raise"
            } else {
                // Fall back to System Events bridge if AX path failed.
                system::window_action(&action, &app_name, window_idx, arg1, arg2)?;
                "applescript-frontmost"
            }
        } else {
            system::window_action(&action, &app_name, window_idx, arg1, arg2)?;
            "applescript"
        };
        if json {
            ok(
                serde_json::json!({"ok": true, "action": action, "app": app_name, "window": window_idx, "method": method}),
            )
        } else {
            println!("{action} window {window_idx} of {app_name}");
            Ok(())
        }
    }
}

fn cmd_launch(json: bool, id: String, no_wait: bool, timeout: u64) -> Result<(), CuError> {
    use std::time::{Duration, Instant};
    let started = Instant::now();
    system::launch_app(&id)?;

    if no_wait {
        if json {
            return ok(
                serde_json::json!({"ok": true, "id": id, "ready_in_ms": 0, "waited": false}),
            );
        }
        println!("Launched {id}");
        return Ok(());
    }

    // Poll for app to register + report a window via AX. App name resolution
    // can fail until the process appears in System Events, so we tolerate
    // resolve errors during the polling window.
    let is_bundle_id = id.contains('.') && !id.contains(' ');
    let deadline = started + Duration::from_secs(timeout);
    let mut last_err: Option<String> = None;
    loop {
        let resolved = if is_bundle_id {
            system::resolve_by_bundle_id(&id)
        } else {
            system::resolve_target_app(&Some(id.clone()))
        };
        match resolved {
            Ok((pid, name)) => {
                if let Some((wx, wy, ww, wh)) = ax::window_bounds(pid) {
                    // D8: warm the AX bridge so the first downstream snapshot is fast.
                    // Some apps (TextEdit, Mail) take 200-500ms on the very first AX walk.
                    let warm_started = Instant::now();
                    let _ = ax::snapshot(pid, &name, 5);
                    let warmup_ms = warm_started.elapsed().as_millis() as u64;
                    let ms = started.elapsed().as_millis() as u64;
                    if json {
                        return ok(serde_json::json!({
                            "ok": true,
                            "id": id,
                            "app": name,
                            "pid": pid,
                            "ready_in_ms": ms,
                            "warmup_ms": warmup_ms,
                            "waited": true,
                            "window": {"x": wx, "y": wy, "width": ww, "height": wh},
                        }));
                    }
                    println!(
                        "Launched {name} (pid {pid}) — window ready in {ms}ms (warmup {warmup_ms}ms)"
                    );
                    return Ok(());
                }
            }
            Err(e) => last_err = Some(e),
        }
        if Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(format!(
        "timed out after {timeout}s waiting for window of '{id}'{}",
        last_err
            .map(|e| format!(" (last: {e})"))
            .unwrap_or_default()
    )
    .into())
}

fn cmd_why(json: bool, ref_id: usize, app: Option<String>, limit: usize) -> Result<(), CuError> {
    let (pid, name) = system::resolve_target_app(&app)?;
    let snap = ax::snapshot(pid, &name, limit);
    if !snap.ok {
        return Err(snap
            .error
            .unwrap_or_else(|| "snapshot failed".into())
            .into());
    }

    let snapshot_size = snap.elements.len();
    let element = snap.elements.iter().find(|e| e.ref_id == ref_id);

    let window_frame = snap.window_frame.as_ref();
    let modal_present = snap.modal.is_some();

    let (found, element_json, checks, advice) = match element {
        None => {
            let advice = if snap.truncated {
                // Suggest a limit big enough to actually reach this ref, with headroom.
                let suggested = limit.max(ref_id + 50).max(100);
                format!(
                    "Ref {ref_id} not in snapshot (size={snapshot_size}, truncated). Re-snapshot with --limit {suggested}."
                )
            } else if ref_id > snapshot_size {
                format!(
                    "Ref {ref_id} is past snapshot end (size={snapshot_size}). Either the ref is stale or the UI changed — re-run cu snapshot."
                )
            } else {
                format!(
                    "Ref {ref_id} not found in snapshot (size={snapshot_size}). The element may have been removed; re-snapshot."
                )
            };
            (
                false,
                serde_json::Value::Null,
                serde_json::json!({
                    "in_snapshot": false,
                    "snapshot_truncated": snap.truncated,
                    "modal_present": modal_present,
                }),
                advice,
            )
        }
        Some(el) => {
            let inspection = ax::inspect_ref(pid, ref_id);
            let cx = el.x + el.width / 2.0;
            let cy = el.y + el.height / 2.0;
            let in_window_bounds = window_frame
                .map(|w| cx >= w.x && cx <= w.x + w.width && cy >= w.y && cy <= w.y + w.height)
                .unwrap_or(true);

            let (actions, enabled, focused, subrole) = match inspection {
                Some(i) => (i.actions, i.enabled, i.focused, i.subrole),
                None => (Vec::new(), None, None, None),
            };

            let click_supported = actions
                .iter()
                .any(|a| a == "AXPress" || a == "AXConfirm" || a == "AXOpen");

            let mut advice_parts: Vec<String> = Vec::new();
            if modal_present {
                advice_parts
                    .push("A modal sheet/dialog is blocking the window — dismiss it first.".into());
            }
            if !in_window_bounds {
                advice_parts.push(format!(
                    "Click point ({cx:.0}, {cy:.0}) is outside the focused window — element may be in a sibling window or offscreen."
                ));
            }
            if matches!(enabled, Some(false)) {
                advice_parts.push(
                    "Element is disabled (AXEnabled=false) — clicking will be a no-op.".into(),
                );
            }
            if actions.is_empty() {
                advice_parts.push(
                    "Element exposes no AX actions — likely a static container (label/group). Try clicking a child element or a parent that owns the click target.".into(),
                );
            } else if !click_supported {
                advice_parts.push(format!(
                    "Element does not support AXPress/AXConfirm/AXOpen. Available: [{}]. Try `cu perform <ref> <action>`.",
                    actions.join(", ")
                ));
            }
            if advice_parts.is_empty() {
                advice_parts.push(
                    "Element looks clickable. If click still fails, the app may ignore PID-targeted events (some sandboxed apps) — focus the app and retry without --app.".into(),
                );
            }

            let element_json = serde_json::json!({
                "ref": el.ref_id,
                "role": el.role,
                "title": el.title,
                "value": el.value,
                "x": el.x,
                "y": el.y,
                "width": el.width,
                "height": el.height,
                "click_x": cx,
                "click_y": cy,
                "axPath": el.ax_path,
                "subrole": subrole,
            });
            let checks_json = serde_json::json!({
                "in_snapshot": true,
                "in_window_bounds": in_window_bounds,
                "enabled": enabled,
                "focused": focused,
                "click_supported": click_supported,
                "actions_supported": actions,
                "modal_present": modal_present,
            });
            (true, element_json, checks_json, advice_parts.join(" "))
        }
    };

    if json {
        ok(serde_json::json!({
            "ok": true,
            "ref": ref_id,
            "app": name,
            "found": found,
            "snapshot_size": snapshot_size,
            "element": element_json,
            "checks": checks,
            "advice": advice,
        }))
    } else {
        if !found {
            println!("ref {ref_id}: NOT FOUND (snapshot size={snapshot_size})");
        } else if let Some(el) = element {
            let title = el.title.as_deref().unwrap_or("");
            println!(
                "ref {ref_id}: {} \"{title}\" at ({:.0}, {:.0}) {:.0}x{:.0}",
                el.role, el.x, el.y, el.width, el.height
            );
            println!("checks: {checks}");
        }
        println!("advice: {advice}");
        Ok(())
    }
}

fn cmd_warm(json: bool, app: String) -> Result<(), CuError> {
    use std::time::Instant;
    let (pid, name) = system::resolve_target_app(&Some(app.clone()))?;
    let started = Instant::now();
    let snap = ax::snapshot(pid, &name, 5);
    if !snap.ok {
        return Err(snap
            .error
            .unwrap_or_else(|| "AX snapshot failed during warm-up".into())
            .into());
    }
    let warmup_ms = started.elapsed().as_millis() as u64;
    if json {
        ok(serde_json::json!({
            "ok": true,
            "app": name,
            "pid": pid,
            "warmup_ms": warmup_ms,
        }))
    } else {
        println!("Warmed AX bridge for {name} (pid {pid}) in {warmup_ms}ms");
        Ok(())
    }
}

fn cmd_state(
    json: bool,
    app: String,
    limit: usize,
    no_screenshot: bool,
    output: Option<String>,
) -> Result<(), CuError> {
    let (pid, name) = system::resolve_target_app(&Some(app.clone()))?;

    // Snapshot tree (load-bearing — fail loudly if AX is broken)
    let snap = ax::snapshot(pid, &name, limit);
    if !snap.ok {
        return Err(snap
            .error
            .unwrap_or_else(|| "AX snapshot failed".into())
            .into());
    }

    // Window list (best-effort — empty list is allowed if app has no windows yet)
    let windows = system::list_windows(Some(&name)).unwrap_or_default();

    // Frontmost flag — soft-fail (don't block on a flaky System Events)
    let frontmost = system::frontmost_app_name()
        .map(|f| f.eq_ignore_ascii_case(&name))
        .unwrap_or(false);

    // Optional screenshot of the front window. Soft-fails on capture errors but
    // surfaces the reason in `screenshot_error` so the agent knows whether the
    // image was skipped (e.g. capture-protected app like WeChat) versus available.
    let (screenshot_info, screenshot_err): (Option<(String, f64)>, Option<String>) = if no_screenshot {
        (None, None)
    } else {
        match screenshot::find_window(pid) {
            Some(win) => {
                let path = output.unwrap_or_else(|| {
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis())
                        .unwrap_or(0);
                    format!("/tmp/cu-state-{ts}.png")
                });
                match screenshot::capture_window_with_scale(&win, &path) {
                    Ok(scale) => (Some((path, scale)), None),
                    Err(e) => (None, Some(e)),
                }
            }
            None => (None, Some("no on-screen window found for the target app".into())),
        }
    };

    if json {
        let mut full = serde_json::json!({
            "ok": true,
            "app": name,
            "pid": pid,
            "frontmost": frontmost,
            "windows": windows,
            "displays": display::list(),
            "elements": snap.elements,
            "snapshot_size": snap.elements.len(),
            "tree_truncated": snap.truncated,
        });
        if let Some(frame) = &snap.window_frame {
            full["window_frame"] = serde_json::json!({
                "x": frame.x, "y": frame.y,
                "width": frame.width, "height": frame.height,
            });
        }
        if let Some((path, scale)) = screenshot_info {
            full["screenshot"] = serde_json::json!(path);
            full["image_scale"] = serde_json::json!(scale);
        } else if let Some(err) = screenshot_err {
            full["screenshot_error"] = serde_json::json!(err);
        }
        ok(full)
    } else {
        println!(
            "{name} (pid {pid}){}  windows={}  elements={}{}",
            if frontmost { " [frontmost]" } else { "" },
            windows.len(),
            snap.elements.len(),
            if snap.truncated { " (truncated)" } else { "" },
        );
        if let Some((path, _)) = &screenshot_info {
            println!("Screenshot: {path}");
        } else if let Some(err) = &screenshot_err {
            println!("Screenshot skipped: {err}");
        }
        Ok(())
    }
}

fn cmd_menu(json: bool, app: String) -> Result<(), CuError> {
    let items = system::list_menu(&app)?;

    if items.is_empty() {
        return Err(format!("no menu items found for {app} (is it running?)").into());
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
) -> Result<(), CuError> {
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
        other => Err(format!("unknown defaults action: {other} (use: read, write)").into()),
    }
}

fn cmd_sdef(json: bool, app: String) -> Result<(), CuError> {
    let bundle_path = system::resolve_app_bundle_path(&app)?;
    let result = sdef::parse(&app, &bundle_path);

    if !result.ok {
        return Err(result
            .error
            .unwrap_or_else(|| "sdef parse failed".into())
            .into());
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

// ── Recipes (G2) ────────────────────────────────────────────────────────────
//
// Each entry: (topic, one-line summary, multi-line recipe body).
// Recipes are working shell snippets — agents can copy-paste and adapt.

const RECIPES: &[(&str, &str, &str)] = &[
    (
        "launch-app",
        "Open any app via Spotlight",
        "cu key cmd+space\n\
         cu type \"Calculator\"\n\
         cu key enter\n\
         cu wait --text \"Calculator\" --app Calculator --timeout 5",
    ),
    (
        "fill-form",
        "Write text into a textfield (no focus, no clipboard)",
        "# Preferred: direct AX write\n\
         REF=$(cu find --app Mail --role textfield --title-contains 'Subject' --first | jq -r .match.ref)\n\
         cu set-value \"$REF\" \"Hello\" --app Mail\n\n\
         # Fallback if set-value fails (non-AX field, e.g. Electron):\n\
         cu click \"$REF\" --app Mail\n\
         cu type \"Hello\" --app Mail",
    ),
    (
        "dismiss-modal",
        "Handle a save sheet / alert that's blocking the window",
        "# Snapshot surfaces a '⚠ Modal:' line when a sheet is up\n\
         cu snapshot TextEdit --limit 50 --human\n\n\
         # Click the sheet's button by title\n\
         REF=$(cu find --app TextEdit --role button --title-equals \"Don't Save\" --first | jq -r .match.ref)\n\
         cu click \"$REF\" --app TextEdit",
    ),
    (
        "read-app-data",
        "Read app data via scripting (no UI traversal)",
        "# Find scriptable apps (S flag in human mode)\n\
         cu apps | jq '.apps[] | select(.scriptable == true) | .name'\n\n\
         # Run AppleScript directly\n\
         cu tell Safari 'get URL of current tab of front window'\n\
         cu tell Notes 'get plaintext of note 1'\n\
         cu tell Mail 'get subject of message 1 of inbox'",
    ),
    (
        "wait-for-ui",
        "Wait until a UI condition is met",
        "cu wait --text \"Loaded\" --app Safari --timeout 10\n\
         cu wait --ref 5    --app Mail   --timeout 5    # ref appears\n\
         cu wait --gone 3   --app Finder --timeout 3    # ref disappears",
    ),
    (
        "vlm-click-by-image",
        "VLM agent: look at annotated screenshot, click by ref",
        "cu snapshot Mail --limit 50 --annotated --output /tmp/m.png\n\
         # VLM looks at /tmp/m.png, picks ref by visual cues (color/position/text)\n\
         cu click 12 --app Mail",
    ),
    (
        "vlm-coord-to-ref",
        "VLM agent: translate a visual pixel into the closest AX ref",
        "# VLM said \"the thing at (480, 320)\" — translate to ref\n\
         REF=$(cu nearest 480 320 --app Mail | jq -r .match.ref)\n\
         cu click \"$REF\" --app Mail",
    ),
    (
        "vlm-region-candidates",
        "VLM agent: list all interactive refs inside a screen rectangle",
        "# VLM narrowed the region; cu lists the candidates\n\
         cu observe-region 480 200 400 300 --app Mail --mode center | \\\n  \
           jq '.matches[] | {ref, role, title}'",
    ),
    (
        "diff-after-action",
        "Cheap re-snapshot — only return changed elements",
        "cu snapshot Mail --limit 50 --diff > /dev/null   # baseline\n\
         cu click 12 --app Mail --no-snapshot\n\
         cu snapshot Mail --limit 50 --diff               # +N ~M -K only",
    ),
    (
        "menu-click",
        "Click any menu bar item (works for ANY app)",
        "cu menu Calculator                                # discover menu structure\n\
         cu tell \"System Events\" 'tell process \"Calculator\" to \\\n  \
           click menu item \"Scientific\" of menu \"View\" of menu bar 1'",
    ),
    (
        "region-screenshot",
        "Cheap VLM verification — screenshot only the area you care about",
        "# Full window screenshot ≈ 1500 tokens; small region ≈ 150 tokens\n\
         cu screenshot --region \"480,200 200x100\" --path /tmp/check.png",
    ),
    (
        "system-pref",
        "Change a macOS preference without opening System Settings",
        "cu defaults read  com.apple.dock autohide\n\
         cu defaults write com.apple.dock autohide -bool true\n\
         killall Dock                                       # apply",
    ),
];

fn cmd_examples(json: bool, topic: Option<String>) -> Result<(), CuError> {
    match topic {
        None => {
            if json {
                let topics: Vec<serde_json::Value> = RECIPES
                    .iter()
                    .map(|(name, summary, _)| serde_json::json!({"name": name, "summary": summary}))
                    .collect();
                ok(serde_json::json!({"ok": true, "topics": topics}))
            } else {
                println!("Available recipe topics ({} total):\n", RECIPES.len());
                let max_name = RECIPES.iter().map(|(n, _, _)| n.len()).max().unwrap_or(0);
                for (name, summary, _) in RECIPES {
                    println!("  {:<width$}  {}", name, summary, width = max_name);
                }
                println!("\nRun `cu examples <topic>` for the recipe.");
                Ok(())
            }
        }
        Some(t) => match RECIPES.iter().find(|(name, _, _)| *name == t) {
            Some((name, summary, body)) => {
                if json {
                    ok(serde_json::json!({
                        "ok": true,
                        "topic": name,
                        "summary": summary,
                        "recipe": body,
                    }))
                } else {
                    println!("# {name} — {summary}\n");
                    println!("{body}");
                    Ok(())
                }
            }
            None => {
                let topic_names: Vec<&str> = RECIPES.iter().map(|(n, _, _)| *n).collect();
                Err(CuError::msg(format!("unknown topic: {t}"))
                    .with_hint(format!("available topics: {}", topic_names.join(", ")))
                    .with_next("cu examples"))
            }
        },
    }
}

fn cmd_tell(json: bool, app: String, expr: String, timeout: u64) -> Result<(), CuError> {
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

/// Parse a region spec into (x, y, width, height).
/// Accepts "x,y WxH" / "x,y,w,h" / "x y w h" / "x,y wxh" — splits on whitespace, comma, or 'x'/'X'.
fn parse_region(spec: &str) -> Result<(f64, f64, f64, f64), CuError> {
    let parts: Vec<&str> = spec
        .split(|c: char| c.is_whitespace() || c == ',' || c == 'x' || c == 'X')
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() != 4 {
        return Err(CuError::msg(format!(
            "invalid --region spec: expected 4 numbers, got {}",
            parts.len()
        ))
        .with_hint("use 'x,y WxH' or 'x,y,w,h' (in points)")
        .with_next("cu screenshot --region '480,200 400x300'"));
    }
    let nums: Result<Vec<f64>, _> = parts.iter().map(|s| s.parse::<f64>()).collect();
    let nums = nums.map_err(|_| {
        CuError::msg("--region contains a non-numeric component")
            .with_hint("each of x/y/width/height must be a finite number")
    })?;
    Ok((nums[0], nums[1], nums[2], nums[3]))
}

fn ok(value: serde_json::Value) -> Result<(), CuError> {
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
    if let Some(ref m) = snap.modal {
        let title = m.title.as_deref().unwrap_or("");
        let subrole = m.subrole.as_deref().unwrap_or("");
        let suffix = if subrole.is_empty() {
            String::new()
        } else {
            format!(" ({subrole})")
        };
        println!("⚠ Modal: {}{} \"{}\"", m.role, suffix, title);
    }
    if let Some(ref f) = snap.focused {
        let ref_part = match f.ref_id {
            Some(r) => format!("[{r}] "),
            None => "(off-snapshot) ".to_string(),
        };
        let title = f.title.as_deref().unwrap_or("");
        let value = match f.value.as_deref() {
            Some(v) if !v.is_empty() => format!(" value=\"{v}\""),
            _ => String::new(),
        };
        println!("Focused: {ref_part}{} \"{title}\"{value}", f.role);
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

fn print_diff_human(snap: &ax::SnapshotResult, d: &diff::Diff) {
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
    println!("[app] {app} — \"{win}\" (diff)");
    if let Some(ref m) = snap.modal {
        let title = m.title.as_deref().unwrap_or("");
        println!("⚠ Modal: {} \"{}\"", m.role, title);
    }
    if let Some(ref f) = snap.focused {
        let ref_part = match f.ref_id {
            Some(r) => format!("[{r}] "),
            None => "(off-snapshot) ".to_string(),
        };
        let title = f.title.as_deref().unwrap_or("");
        println!("Focused: {ref_part}{} \"{title}\"", f.role);
    }
    if d.added.is_empty() && d.changed.is_empty() && d.removed.is_empty() {
        println!(
            "(no changes — {} elements unchanged of {})",
            d.unchanged_count, d.total
        );
        return;
    }
    for el in &d.added {
        let label = el.title.as_deref().or(el.value.as_deref()).unwrap_or("");
        println!(
            "+ [{}] {} \"{}\" ({},{} {}×{})",
            el.ref_id, el.role, label, el.x, el.y, el.width, el.height
        );
    }
    for el in &d.changed {
        let label = el.title.as_deref().or(el.value.as_deref()).unwrap_or("");
        println!(
            "~ [{}] {} \"{}\" ({},{} {}×{})",
            el.ref_id, el.role, label, el.x, el.y, el.width, el.height
        );
    }
    for ref_id in &d.removed {
        println!("- [{ref_id}] (removed)");
    }
    println!(
        "Summary: +{} ~{} -{} (={} unchanged of {})",
        d.added.len(),
        d.changed.len(),
        d.removed.len(),
        d.unchanged_count,
        d.total
    );
}

/// Compares the pre-action AX state against the snapshot already attached to
/// `result` by `maybe_attach_snapshot`, then enriches the response with a
/// `verified` flag and per-action diff stats. Used by `cu click --verify`.
///
/// `verified=false` means the AX tree did not change after the action — most
/// often a sandboxed/Electron app silently ignored a PID-targeted CGEvent.
/// We don't auto-retry: focus stealing is disruptive and AX-empty diffs have
/// false positives (network roundtrips, animations not yet started). The
/// agent reads the advice and chooses whether to recover.
fn attach_verification(
    result: &mut serde_json::Value,
    pre: &[ax::Element],
    method: &str,
) {
    // Pull post-action elements from the snapshot maybe_attach_snapshot already
    // attached. If there's no snapshot (e.g. --no-snapshot), verification is
    // still meaningful — fall back to {"verified": null} with an explanation.
    let post_elements: Option<Vec<ax::Element>> = result
        .get("snapshot")
        .and_then(|s| s.get("elements"))
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let Some(post) = post_elements else {
        if let Some(obj) = result.as_object_mut() {
            obj.insert("verified".to_string(), serde_json::Value::Null);
            obj.insert(
                "verification_skipped".to_string(),
                serde_json::Value::String(
                    "no post-action snapshot available (--no-snapshot was set)".into(),
                ),
            );
        }
        return;
    };

    let d = diff::diff(pre, &post);
    let verified = !(d.added.is_empty() && d.changed.is_empty() && d.removed.is_empty());

    if let Some(obj) = result.as_object_mut() {
        obj.insert(
            "verified".to_string(),
            serde_json::Value::Bool(verified),
        );
        obj.insert(
            "verify_diff".to_string(),
            serde_json::json!({
                "added": d.added.len(),
                "changed": d.changed.len(),
                "removed": d.removed.len(),
            }),
        );
        if !verified {
            // Tailor the advice to the method that produced the silent failure.
            let advice = if method.starts_with("cgevent-pid")
                || method == "double-click-pid"
                || method == "cgevent-right-pid"
            {
                "AX tree unchanged after PID-targeted CGEvent click — target app may be sandboxed (some Mac App Store / CEF apps drop pid-targeted events). Recovery: `cu window focus --app <Name>` then retry without --app via `cu click <ref> --allow-global` (focus shifts but the click lands)."
            } else if method == "ax-action" {
                "AX action returned ok but the tree didn't change. The action may have triggered an async operation (network, navigation) whose result hasn't landed yet — re-run cu snapshot after a short wait, or use `cu wait --gone <ref>` to wait for the actual transition."
            } else {
                "AX tree unchanged after the action. Either the click missed the target, the app handled it silently, or the resulting UI change hasn't landed yet."
            };
            obj.insert(
                "verify_advice".to_string(),
                serde_json::Value::String(advice.into()),
            );
        }
    }
}

fn maybe_attach_snapshot(
    result: &mut serde_json::Value,
    json: bool,
    no_snapshot: bool,
    app: &Option<String>,
    limit: usize,
) {
    // Annotate every action result with confidence/advice derived from its
    // `method` field, regardless of snapshot attachment (C4).
    annotate_method(result);
    if !json || no_snapshot {
        return;
    }
    if let Ok((pid, name)) = system::resolve_target_app(app) {
        // D7: replace the unconditional 500ms sleep with a single-shot
        // AXObserver wait. Returns as soon as the first AX notification fires
        // (typical: ~50ms) or after POST_ACTION_DELAY_MS at most.
        let waited = observer::wait_for_settle(pid, POST_ACTION_DELAY_MS);
        let snap = ax::snapshot(pid, &name, limit);
        // D1: agents read the auto-attached snapshot far more often than they
        // call `cu snapshot` directly, so the displays array must be reachable
        // here too — otherwise multi-display info silently drops out of the
        // most common code path.
        let mut snap_value = serde_json::to_value(&snap).unwrap_or_default();
        if let Some(obj) = snap_value.as_object_mut() {
            obj.insert(
                "displays".to_string(),
                serde_json::to_value(display::list()).unwrap_or_default(),
            );
        }
        if let Some(obj) = result.as_object_mut() {
            obj.insert(
                "settle_ms".to_string(),
                serde_json::Value::Number(serde_json::Number::from(waited)),
            );
        }
        result["snapshot"] = snap_value;
    }
}
