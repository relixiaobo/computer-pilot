//! macOS system integration — app resolution, permissions, launch, and scripting.
//! All scripting uses AppleScript (no JXA). Sdef parsing is in sdef.rs (Rust native).

use crate::error::{CuError, ErrorCode};
use std::ffi::{CStr, c_char, c_void};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}

unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    fn objc_msgSend();
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}

#[derive(Clone, Debug)]
struct NativeApp {
    name: String,
    pid: i32,
    bundle_id: String,
    bundle_path: String,
    active: bool,
    activation_policy: i64,
}

fn nsstring(
    receiver: *mut c_void,
    send_cstr: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *const c_char,
) -> String {
    if receiver.is_null() {
        return String::new();
    }
    let utf8 = unsafe { send_cstr(receiver, sel_registerName(c"UTF8String".as_ptr())) };
    if utf8.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(utf8) }
            .to_string_lossy()
            .into_owned()
    }
}

fn running_apps_native() -> Vec<NativeApp> {
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let send_id: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const ());
        let send_index: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const ());
        let send_usize: unsafe extern "C" fn(*mut c_void, *mut c_void) -> usize =
            std::mem::transmute(objc_msgSend as *const ());
        let send_i32: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32 =
            std::mem::transmute(objc_msgSend as *const ());
        let send_i64: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i64 =
            std::mem::transmute(objc_msgSend as *const ());
        let send_bool: unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool =
            std::mem::transmute(objc_msgSend as *const ());
        let send_cstr: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *const c_char =
            std::mem::transmute(objc_msgSend as *const ());

        let workspace_class = objc_getClass(c"NSWorkspace".as_ptr());
        let workspace = send_id(
            workspace_class,
            sel_registerName(c"sharedWorkspace".as_ptr()),
        );
        let array = send_id(workspace, sel_registerName(c"runningApplications".as_ptr()));
        let count = if array.is_null() {
            0
        } else {
            send_usize(array, sel_registerName(c"count".as_ptr()))
        };
        let mut apps = Vec::with_capacity(count);
        for index in 0..count {
            let app = send_index(array, sel_registerName(c"objectAtIndex:".as_ptr()), index);
            if app.is_null() {
                continue;
            }
            let policy = send_i64(app, sel_registerName(c"activationPolicy".as_ptr()));
            if policy == 2 {
                continue;
            }
            let name_obj = send_id(app, sel_registerName(c"localizedName".as_ptr()));
            let bundle_obj = send_id(app, sel_registerName(c"bundleIdentifier".as_ptr()));
            let url = send_id(app, sel_registerName(c"bundleURL".as_ptr()));
            let path_obj = if url.is_null() {
                std::ptr::null_mut()
            } else {
                send_id(url, sel_registerName(c"path".as_ptr()))
            };
            let name = nsstring(name_obj, send_cstr);
            let pid = send_i32(app, sel_registerName(c"processIdentifier".as_ptr()));
            if name.is_empty() || pid <= 0 {
                continue;
            }
            apps.push(NativeApp {
                name,
                pid,
                bundle_id: nsstring(bundle_obj, send_cstr),
                bundle_path: nsstring(path_obj, send_cstr),
                active: send_bool(app, sel_registerName(c"isActive".as_ptr())),
                activation_policy: policy,
            });
        }
        objc_autoreleasePoolPop(pool);
        apps
    }
}

/// Resolve a running application's bundle identifier without Apple Events.
pub fn bundle_id_for_pid(pid: i32) -> Option<String> {
    unsafe {
        let send_id: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const ());
        let send_pid: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const ());
        let send_cstr: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *const c_char =
            std::mem::transmute(objc_msgSend as *const ());
        let class = objc_getClass(c"NSRunningApplication".as_ptr());
        if class.is_null() {
            return None;
        }
        let app = send_pid(
            class,
            sel_registerName(c"runningApplicationWithProcessIdentifier:".as_ptr()),
            pid,
        );
        if app.is_null() {
            return None;
        }
        let bundle = send_id(app, sel_registerName(c"bundleIdentifier".as_ptr()));
        if bundle.is_null() {
            return None;
        }
        let utf8 = send_cstr(bundle, sel_registerName(c"UTF8String".as_ptr()));
        if utf8.is_null() {
            return None;
        }
        Some(CStr::from_ptr(utf8).to_string_lossy().into_owned())
    }
}

/// Return the process identifier that AppKit currently considers frontmost.
/// This is the authority used to verify focus changes; AXMain/AXRaise only
/// describe a window inside an application and do not prove app activation.
pub fn frontmost_pid() -> Option<i32> {
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let send_id: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const ());
        let send_i32: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32 =
            std::mem::transmute(objc_msgSend as *const ());
        let workspace_class = objc_getClass(c"NSWorkspace".as_ptr());
        if workspace_class.is_null() {
            objc_autoreleasePoolPop(pool);
            return None;
        }
        let workspace = send_id(
            workspace_class,
            sel_registerName(c"sharedWorkspace".as_ptr()),
        );
        let app = if workspace.is_null() {
            std::ptr::null_mut()
        } else {
            send_id(
                workspace,
                sel_registerName(c"frontmostApplication".as_ptr()),
            )
        };
        let pid = if app.is_null() {
            0
        } else {
            send_i32(app, sel_registerName(c"processIdentifier".as_ptr()))
        };
        objc_autoreleasePoolPop(pool);
        (pid > 0).then_some(pid)
    }
}

/// Activate one exact process and verify that AppKit made the same PID
/// frontmost. Application names and bundle identifiers are deliberately not
/// accepted here because development Electron instances commonly share both.
pub fn activate_pid(pid: i32) -> Result<(), CuError> {
    if frontmost_pid() == Some(pid) {
        return Ok(());
    }

    let accepted = unsafe {
        let pool = objc_autoreleasePoolPush();
        let send_pid: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const ());
        let send_bool_options: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> bool =
            std::mem::transmute(objc_msgSend as *const ());
        let class = objc_getClass(c"NSRunningApplication".as_ptr());
        let app = if class.is_null() {
            std::ptr::null_mut()
        } else {
            send_pid(
                class,
                sel_registerName(c"runningApplicationWithProcessIdentifier:".as_ptr()),
                pid,
            )
        };
        // NSApplicationActivateAllWindows | NSApplicationActivateIgnoringOtherApps.
        let accepted = !app.is_null()
            && send_bool_options(app, sel_registerName(c"activateWithOptions:".as_ptr()), 3);
        objc_autoreleasePoolPop(pool);
        accepted
    };

    if !accepted {
        return Err(CuError::msg(format!(
            "focus failed: NSRunningApplication rejected activation for pid:{pid}"
        ))
        .with_code(ErrorCode::FocusFailed)
        .with_hint("confirm the process still exists, then run `cu apps` and retry with its current pid selector"));
    }

    let deadline = Instant::now() + Duration::from_millis(1500);
    loop {
        if frontmost_pid() == Some(pid) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            let actual = frontmost_pid();
            return Err(CuError::msg(format!(
                "focus failed: requested pid:{pid}, but the frontmost pid is {}",
                actual
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unknown".into())
            ))
            .with_code(ErrorCode::FocusFailed)
            .with_hint("do not send global input; re-resolve the target with `cu apps` and inspect whether macOS denied activation")
            .with_diagnostics(serde_json::json!({
                "requested_pid": pid,
                "frontmost_pid": actual,
            })));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

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

// ── TCC attribution subject ─────────────────────────────────────────────────

unsafe extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn proc_pidpath(pid: i32, buffer: *mut c_void, buffersize: u32) -> i32;
}

/// The process macOS TCC may attribute permission checks to. TCC often keys
/// grants on the "responsible process" (the app that spawned the shell chain)
/// rather than the leaf binary, so accurate System Settings guidance must name
/// the real subject instead of guessing "your terminal app".
pub struct TccSubject {
    /// `None` when the running executable's path cannot be resolved — the one
    /// case where no subject can be named, and callers must say so instead of
    /// rendering an empty name into remediation text.
    pub executable: Option<String>,
    pub responsible_pid: Option<i32>,
    pub responsible_process: Option<String>,
}

pub fn tcc_subject() -> TccSubject {
    let executable = std::env::current_exe()
        .ok()
        .and_then(|path| path.canonicalize().ok())
        .map(|path| path.to_string_lossy().into_owned());

    // Private but long-stable libquarantine symbol; resolved dynamically so a
    // macOS release that drops it degrades to executable-only reporting.
    let responsible_pid = unsafe {
        const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;
        let symbol = dlsym(
            RTLD_DEFAULT,
            c"responsibility_get_pid_responsible_for_pid".as_ptr(),
        );
        if symbol.is_null() {
            None
        } else {
            let get: unsafe extern "C" fn(i32) -> i32 = std::mem::transmute(symbol);
            let pid = get(std::process::id() as i32);
            (pid > 0).then_some(pid)
        }
    };

    let responsible_process = responsible_pid.and_then(|pid| {
        if pid == std::process::id() as i32 {
            return None; // cu is its own responsible process
        }
        running_apps_native()
            .into_iter()
            .find(|app| app.pid == pid)
            .map(|app| app.name)
            .or_else(|| {
                let mut buffer = [0u8; 4096];
                let written = unsafe {
                    proc_pidpath(pid, buffer.as_mut_ptr() as *mut c_void, buffer.len() as u32)
                };
                (written > 0)
                    .then(|| String::from_utf8_lossy(&buffer[..written as usize]).into_owned())
            })
    });

    TccSubject {
        executable,
        responsible_pid,
        responsible_process,
    }
}

// ── App resolution ──────────────────────────────────────────────────────────

fn parse_pid_selector(selector: &str) -> Result<Option<i32>, CuError> {
    let selector = selector.trim();
    if !selector
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("pid:"))
    {
        return Ok(None);
    }

    let raw = selector.get(4..).unwrap_or_default();
    if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(CuError::msg(format!(
            "invalid PID selector \"{selector}\": expected pid:<positive integer>"
        ))
        .with_code(ErrorCode::InvalidArgument)
        .with_hint("run `cu apps` and copy the candidate's `selector` field"));
    }
    let pid = raw.parse::<i32>().map_err(|_| {
        CuError::msg(format!(
            "invalid PID selector \"{selector}\": PID is outside the supported range"
        ))
        .with_code(ErrorCode::InvalidArgument)
    })?;
    if pid <= 0 {
        return Err(CuError::msg(format!(
            "invalid PID selector \"{selector}\": PID must be positive"
        ))
        .with_code(ErrorCode::InvalidArgument));
    }
    Ok(Some(pid))
}

fn candidate_json(app: &NativeApp) -> serde_json::Value {
    serde_json::json!({
        "name": app.name,
        "pid": app.pid,
        "selector": format!("pid:{}", app.pid),
        "bundle_id": app.bundle_id,
        "bundle_path": app.bundle_path,
        "active": app.active,
    })
}

fn resolve_target_from_apps(
    apps: &[NativeApp],
    selector: &Option<String>,
) -> Result<(i32, String), CuError> {
    let Some(selector) = selector else {
        return apps
            .iter()
            .find(|app| app.active)
            .map(|app| (app.pid, app.name.clone()))
            .ok_or_else(|| {
                CuError::msg("no frontmost application found")
                    .with_code(ErrorCode::AppNotFound)
                    .with_hint("run `cu apps` and pass an explicit app selector")
            });
    };

    if let Some(pid) = parse_pid_selector(selector)? {
        return apps
            .iter()
            .find(|app| app.pid == pid)
            .map(|app| (app.pid, app.name.clone()))
            .ok_or_else(|| {
                CuError::msg(format!("app not running: pid:{pid}"))
                    .with_code(ErrorCode::AppNotFound)
                    .with_hint(
                        "the process may have exited; run `cu apps` and refresh the selector",
                    )
            });
    }

    let mut matches: Vec<&NativeApp> = apps
        .iter()
        .filter(|app| {
            app.name.eq_ignore_ascii_case(selector) || app.bundle_id.eq_ignore_ascii_case(selector)
        })
        .collect();
    matches.sort_by_key(|app| app.pid);

    match matches.as_slice() {
        [] => Err(CuError::msg(format!("app not running: {selector}"))
            .with_code(ErrorCode::AppNotFound)
            .with_hint("run `cu apps` to inspect running application selectors")),
        [app] => Ok((app.pid, app.name.clone())),
        _ => {
            let summary = matches
                .iter()
                .map(|app| format!("{} (pid {})", app.name, app.pid))
                .collect::<Vec<_>>()
                .join(", ");
            let candidates = matches
                .iter()
                .map(|app| candidate_json(app))
                .collect::<Vec<_>>();
            Err(CuError::msg(format!(
                "ambiguous target \"{selector}\": matched {} running processes: {summary}",
                matches.len()
            ))
            .with_code(ErrorCode::AmbiguousTarget)
            .with_hint(
                "choose the intended process from diagnostics.candidates, then reuse its pid:<PID> selector for every state, action, and wait command",
            )
            .with_next("cu state \"pid:<PID>\"")
            .with_diagnostics(serde_json::json!({
                "selector": selector,
                "candidates": candidates,
            })))
        }
    }
}

pub fn resolve_target_app(name: &Option<String>) -> Result<(i32, String), CuError> {
    resolve_target_from_apps(&running_apps_native(), name)
}

/// Return running foreground-capable processes for native AX enumeration.
pub fn running_app_processes() -> Vec<(i32, String)> {
    running_apps_native()
        .into_iter()
        .map(|app| (app.pid, app.name))
        .collect()
}

/// Apps where stray keystrokes are most damaging — terminals execute every line as a
/// command, IDEs interpret shortcuts as destructive operations (close tab, delete file).
/// When `cu key` / `cu type` runs without `--app`, events go to whatever is frontmost
/// via the global HID tap; if the user happens to have one of these focused, the agent
/// silently types into the wrong window. Refuse early with a clear error instead.
pub const DANGEROUS_FRONTMOST: &[&str] = &[
    // Terminal emulators
    "Terminal",
    "iTerm",
    "iTerm2",
    "Ghostty",
    "Alacritty",
    "kitty",
    "WezTerm",
    "Tabby",
    "Hyper",
    "Warp",
    // Editors / IDEs
    "Code",
    "Visual Studio Code",
    "Code - Insiders",
    "Cursor",
    "Windsurf",
    "Xcode",
    "Sublime Text",
    "Nova",
    "Zed",
    "IntelliJ IDEA",
    "IntelliJ IDEA CE",
    "PyCharm",
    "PyCharm CE",
    "WebStorm",
    "RustRover",
    "GoLand",
    "CLion",
    "RubyMine",
    "PhpStorm",
    "Android Studio",
    "DataGrip",
];

/// Cheap query: name of the frontmost GUI process.
/// 5s timeout; intended for safety-check fast paths.
///
/// `CU_TEST_FRONTMOST_OVERRIDE` (unset in production) lets shell tests inject
/// a deterministic frontmost name without activating an app and disrupting the
/// user. This is the only test seam in this module.
pub fn frontmost_app_name() -> Result<String, String> {
    if let Ok(name) = std::env::var("CU_TEST_FRONTMOST_OVERRIDE")
        && !name.is_empty()
    {
        return Ok(name);
    }
    running_apps_native()
        .into_iter()
        .find(|app| app.active)
        .map(|app| app.name)
        .ok_or_else(|| "no frontmost application found".into())
}

/// PID of the frontmost GUI process. Use this when duplicate app names exist.
pub fn frontmost_app_pid() -> Result<i32, String> {
    running_apps_native()
        .into_iter()
        .find(|app| app.active)
        .map(|app| app.pid)
        .ok_or_else(|| "no frontmost application found".into())
}

/// Returns Err with a structured message when the frontmost app is one of
/// `DANGEROUS_FRONTMOST`. Soft-fails (returns Ok) if the frontmost lookup itself
/// fails — we don't want to block legitimate use because app discovery raced.
pub fn check_global_frontmost_safety(verb: &str) -> Result<(), String> {
    let front = match frontmost_app_name() {
        Ok(name) if !name.is_empty() => name,
        _ => return Ok(()),
    };
    if DANGEROUS_FRONTMOST
        .iter()
        .any(|d| d.eq_ignore_ascii_case(&front))
    {
        return Err(format!(
            "refusing to {verb} without --app: frontmost is \"{front}\" \
             (terminal/IDE — stray input would execute commands or destructive shortcuts). \
             Pass --app <Name> to target a specific app, or --allow-global to override."
        ));
    }
    Ok(())
}

// ── List apps ──────────────────────────────────────────────────────────────

pub fn list_apps() -> Result<String, String> {
    let mut apps: Vec<serde_json::Value> = Vec::new();
    for app in running_apps_native() {
        let bundle = if app.bundle_path.ends_with('/') {
            app.bundle_path.clone()
        } else {
            format!("{}/", app.bundle_path)
        };
        let sdef_classes = if !app.bundle_path.is_empty() {
            crate::sdef::count_classes(&bundle)
        } else {
            None
        };
        let mut entry = serde_json::json!({
            "name": app.name,
            "pid": app.pid,
            "selector": format!("pid:{}", app.pid),
            "bundle_id": app.bundle_id,
            "bundle_path": app.bundle_path,
            "active": app.active,
            "activation_policy": app.activation_policy,
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
            .then_with(|| a["pid"].as_i64().cmp(&b["pid"].as_i64()))
    });

    Ok(serde_json::to_string(&serde_json::json!({"apps": apps}))
        .unwrap_or_else(|_| r#"{"apps":[]}"#.to_string()))
}

/// Resolve app bundle path — running apps first, then filesystem search.
pub fn resolve_app_bundle_path(app: &str) -> Result<String, String> {
    // Running application metadata comes from NSWorkspace and does not require
    // Apple Events Automation permission.
    if let Some(path) = running_apps_native()
        .into_iter()
        .find(|candidate| {
            candidate.name.eq_ignore_ascii_case(app)
                || candidate.bundle_id.eq_ignore_ascii_case(app)
        })
        .map(|candidate| candidate.bundle_path)
        .filter(|path| !path.is_empty())
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

/// Resolve a process by bundle identifier through NSWorkspace.
/// Returns `(pid, app_name)` once the process exists, error otherwise.
pub fn resolve_by_bundle_id(bundle_id: &str) -> Result<(i32, String), CuError> {
    resolve_target_from_apps(&running_apps_native(), &Some(bundle_id.to_string()))
}

/// Launch an app by name or bundle identifier via Launch Services.
/// Heuristic: an `id` argument with a `.` is treated as a bundle id (`open -b`),
/// otherwise as an app name (`open -a`). Returns immediately — the caller is
/// responsible for waiting on readiness if needed.
///
/// Always passes `-g` (background): launching must not steal the user's
/// frontmost app. Agents that need the app foregrounded use `cu window focus`
/// explicitly, which reports the activation as user-visible.
pub fn launch_app(id: &str) -> Result<(), String> {
    let flag = if id.contains('.') && !id.contains(' ') {
        "-b"
    } else {
        "-a"
    };
    let mut last_detail = String::new();
    for attempt in 0..=10 {
        let status = Command::new("open")
            .args(["-g", flag, id])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("failed to spawn `open`: {e}"))?;
        if status.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&status.stderr);
        let msg = stderr.trim();
        last_detail = if msg.is_empty() {
            "not found".into()
        } else {
            msg.into()
        };
        if !last_detail.contains("-600") || attempt == 10 {
            break;
        }
        // Launch Services returns -600 while an app is still terminating.
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    Err(format!("open {flag} {id} failed: {last_detail}"))
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
            // Launch the app via Launch Services (not AppleScript) and wait
            // for it. -g keeps the launch in the background so the retry
            // doesn't steal the user's frontmost app.
            let _ = std::process::Command::new("open")
                .args(["-g", "-a", app])
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

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, pid: i32, bundle_id: &str, bundle_path: &str, active: bool) -> NativeApp {
        NativeApp {
            name: name.into(),
            pid,
            bundle_id: bundle_id.into(),
            bundle_path: bundle_path.into(),
            active,
            activation_policy: 0,
        }
    }

    #[test]
    fn duplicate_names_and_bundles_require_pid_selector() {
        let apps = vec![
            app(
                "WorkBuddy",
                45660,
                "com.example.workbuddy",
                "/Applications/WorkBuddy.app",
                true,
            ),
            app(
                "WorkBuddy",
                89806,
                "com.example.workbuddy",
                "/workspace/WorkBuddy.app",
                false,
            ),
        ];

        for selector in ["WorkBuddy", "com.example.workbuddy"] {
            let error = resolve_target_from_apps(&apps, &Some(selector.into())).unwrap_err();
            assert_eq!(error.code, ErrorCode::AmbiguousTarget);
            assert_eq!(error.to_json()["code"], "ambiguous_target");
            assert_eq!(
                error.to_json()["diagnostics"]["candidates"][0]["pid"],
                45660
            );
            assert_eq!(
                error.to_json()["diagnostics"]["candidates"][1]["pid"],
                89806
            );
        }

        assert_eq!(
            resolve_target_from_apps(&apps, &Some("pid:89806".into())).unwrap(),
            (89806, "WorkBuddy".into())
        );
    }

    #[test]
    fn pid_selector_is_strict_and_reports_exited_processes() {
        let apps = vec![app("Finder", 101, "com.apple.finder", "/Finder.app", true)];

        let invalid =
            resolve_target_from_apps(&apps, &Some("pid:not-a-number".into())).unwrap_err();
        assert_eq!(invalid.code, ErrorCode::InvalidArgument);

        let missing = resolve_target_from_apps(&apps, &Some("pid:999".into())).unwrap_err();
        assert_eq!(missing.code, ErrorCode::AppNotFound);
    }
}
