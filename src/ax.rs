//! macOS Accessibility (AX) snapshot — walks the UI element tree of a target application.
#![allow(unsafe_op_in_unsafe_fn)]

use crate::error::CuError;
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, c_char, c_long, c_void};
use std::sync::OnceLock;

// ── Core Foundation FFI ─────────────────────────────────────────────────────

type CFTypeRef = *const c_void;
type CFStringRef = CFTypeRef;
type CFArrayRef = CFTypeRef;
type CFIndex = c_long;
type CFTypeID = u64;
type Boolean = u8;

const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: CFTypeRef);
    fn CFGetTypeID(cf: CFTypeRef) -> CFTypeID;
    fn CFStringGetTypeID() -> CFTypeID;
    fn CFArrayGetTypeID() -> CFTypeID;

    fn CFStringCreateWithBytes(
        alloc: CFTypeRef,
        bytes: *const u8,
        num_bytes: CFIndex,
        encoding: u32,
        is_external_representation: Boolean,
    ) -> CFStringRef;
    fn CFStringGetLength(the_string: CFStringRef) -> CFIndex;
    fn CFStringGetCString(
        the_string: CFStringRef,
        buffer: *mut c_char,
        buffer_size: CFIndex,
        encoding: u32,
    ) -> Boolean;

    fn CFArrayGetCount(the_array: CFArrayRef) -> CFIndex;
    fn CFArrayGetValueAtIndex(the_array: CFArrayRef, idx: CFIndex) -> CFTypeRef;

    fn CFBooleanGetTypeID() -> CFTypeID;
}

// ── Accessibility FFI ───────────────────────────────────────────────────────

type AXError = i32;
const AX_OK: AXError = 0;
const AX_VALUE_CG_POINT: u32 = 1;
const AX_VALUE_CG_SIZE: u32 = 2;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXValueGetValue(value: CFTypeRef, the_type: u32, value_ptr: *mut c_void) -> Boolean;
    fn AXValueCreate(the_type: u32, value_ptr: *const c_void) -> CFTypeRef;
    fn AXUIElementPerformAction(element: CFTypeRef, action: CFStringRef) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: CFTypeRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetMessagingTimeout(element: CFTypeRef, timeout_secs: f32) -> AXError;
    fn AXUIElementCopyMultipleAttributeValues(
        element: CFTypeRef,
        attributes: CFArrayRef,
        options: u32, // 0 = normal
        values: *mut CFArrayRef,
    ) -> AXError;
    fn AXUIElementCopyActionNames(element: CFTypeRef, names: *mut CFArrayRef) -> AXError;

    /// Private API exported by HIServices since 10.5 — used by Chromium /
    /// Electron / VS Code / every major Mac browser to map an AXUIElement
    /// to its CGWindowID. We need it because AX is the only authoritative
    /// "this is the window the app considers primary"; CGWindowList is a
    /// flat list with no such notion. Symbol is permanently stable.
    fn _AXUIElementGetWindow(element: CFTypeRef, window_id: *mut u32) -> AXError;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFArrayCreate(
        allocator: CFTypeRef,
        values: *const CFTypeRef,
        count: CFIndex,
        callbacks: CFTypeRef, // kCFTypeArrayCallBacks
    ) -> CFArrayRef;
    static kCFTypeArrayCallBacks: CFTypeRef;
    static kCFBooleanTrue: CFTypeRef;
    static kCFBooleanFalse: CFTypeRef;
}

// ── Geometry ────────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGSize {
    width: f64,
    height: f64,
}

// ── Public types ────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct SnapshotResult {
    pub ok: bool,
    pub app: String,
    pub window: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_frame: Option<WindowFrame>,
    /// The currently focused element (where the next keystroke would go).
    /// Lets the agent skip a redundant click when the field it wants is
    /// already focused. `ref` may be None if the focused element is outside
    /// the snapshot's `--limit` window.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused: Option<FocusedSummary>,
    /// A modal (AXSheet / AXSystemDialog) is currently blocking the window.
    /// Agent should dismiss it before doing anything else.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modal: Option<ModalSummary>,
    pub elements: Vec<Element>,
    pub limit: usize,
    pub truncated: bool,
    /// Actionable hint attached only when `truncated=true`. Agents skim
    /// for unfamiliar fields more reliably than for boolean flags — making
    /// the cause loud here prevents the "I keep searching for ref [73]
    /// that was never returned" failure mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation_hint: Option<String>,
    pub depth_limited: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct FocusedSummary {
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<usize>,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(rename = "axPath", skip_serializing_if = "Option::is_none")]
    pub ax_path: Option<String>,
}

#[derive(Serialize)]
pub struct ModalSummary {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subrole: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Serialize)]
pub struct WindowFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Serialize)]
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

#[derive(Serialize)]
pub struct MenuItem {
    pub menu: String,
    pub item: String,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Element {
    #[serde(rename = "ref")]
    pub ref_id: usize,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub value: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Stable selector that survives across snapshots even when ref numbers
    /// shuffle. Format: `Role[Title]/Role[Title]:N/...`. The `:N` suffix is
    /// the 0-indexed position among siblings with the same `Role[Title]`,
    /// omitted when `N=0`. (A2)
    #[serde(rename = "axPath", skip_serializing_if = "Option::is_none", default)]
    pub ax_path: Option<String>,
}

/// Build one `Role[Title]` segment from raw role + title, sanitizing chars
/// reserved by the path syntax (`/`, `[`, `]`).
fn build_path_segment(role: &str, title: Option<&str>) -> String {
    let role = normalize_role(role);
    match title {
        Some(t) if !t.is_empty() => {
            let safe: String = t
                .chars()
                .map(|c| if matches!(c, '/' | '[' | ']') { '_' } else { c })
                .collect();
            // Cap title length so the path stays readable on long values.
            let cut = if safe.chars().count() > 60 {
                let mut s: String = safe.chars().take(60).collect();
                s.push('…');
                s
            } else {
                safe
            };
            format!("{role}[{cut}]")
        }
        _ => role,
    }
}

// ── CF helpers ──────────────────────────────────────────────────────────────

/// Create a CFString from a Rust `&str`. Caller must `CFRelease`.
/// Returns `None` if CoreFoundation allocation fails.
unsafe fn cfstr(s: &str) -> Option<CFStringRef> {
    let ptr = CFStringCreateWithBytes(
        std::ptr::null(), // kCFAllocatorDefault
        s.as_ptr(),
        s.len() as CFIndex,
        CF_STRING_ENCODING_UTF8,
        0,
    );
    if ptr.is_null() { None } else { Some(ptr) }
}

/// Convert a CFStringRef to a Rust String. Does **not** release the input.
unsafe fn cfstring_to_string(cf: CFStringRef) -> Option<String> {
    if cf.is_null() {
        return None;
    }
    let len = CFStringGetLength(cf);
    if len == 0 {
        return Some(String::new());
    }
    // UTF-8 can use up to 4 bytes per UTF-16 code unit, +1 for NUL
    let buf_size = len * 4 + 1;
    let mut buf: Vec<u8> = vec![0; buf_size as usize];
    if CFStringGetCString(
        cf,
        buf.as_mut_ptr() as *mut c_char,
        buf_size,
        CF_STRING_ENCODING_UTF8,
    ) != 0
    {
        CStr::from_ptr(buf.as_ptr() as *const c_char)
            .to_str()
            .ok()
            .map(|s| s.to_owned())
    } else {
        None
    }
}

// ── AX helpers ──────────────────────────────────────────────────────────────

/// Per-element AX IPC timeout in seconds. Prevents Chrome/Electron hangs.
const AX_TIMEOUT_SECS: f32 = 3.0;

/// Create an AXUIElement for an app with timeout set.
unsafe fn create_app_element(pid: i32) -> CFTypeRef {
    let el = AXUIElementCreateApplication(pid);
    if !el.is_null() {
        AXUIElementSetMessagingTimeout(el, AX_TIMEOUT_SECS);
    }
    el
}

/// Set timeout on a window/child element (timeout may not inherit from parent).
unsafe fn set_element_timeout(el: CFTypeRef) {
    if !el.is_null() {
        AXUIElementSetMessagingTimeout(el, AX_TIMEOUT_SECS);
    }
}

/// Get a raw attribute value (+1 retained). Caller must `CFRelease`.
unsafe fn ax_attr(element: CFTypeRef, name: &str) -> Option<CFTypeRef> {
    let key = cfstr(name)?;
    let mut value: CFTypeRef = std::ptr::null();
    let err = AXUIElementCopyAttributeValue(element, key, &mut value);
    CFRelease(key);
    if err == AX_OK && !value.is_null() {
        Some(value)
    } else {
        None
    }
}

/// Get a raw attribute value and return the AXError code (for diagnostics).
unsafe fn ax_attr_with_err(element: CFTypeRef, name: &str) -> (AXError, CFTypeRef) {
    let Some(key) = cfstr(name) else {
        return (-1, std::ptr::null());
    };
    let mut value: CFTypeRef = std::ptr::null();
    let err = AXUIElementCopyAttributeValue(element, key, &mut value);
    CFRelease(key);
    (err, value)
}

/// Get a string attribute from an AX element.
unsafe fn ax_string(element: CFTypeRef, name: &str) -> Option<String> {
    let value = ax_attr(element, name)?;
    let result = if CFGetTypeID(value) == CFStringGetTypeID() {
        cfstring_to_string(value)
    } else {
        None
    };
    CFRelease(value);
    result
}

/// Get the position (AXPosition → CGPoint).
unsafe fn ax_position(element: CFTypeRef) -> Option<CGPoint> {
    let value = ax_attr(element, "AXPosition")?;
    let mut point = CGPoint::default();
    let ok = AXValueGetValue(
        value,
        AX_VALUE_CG_POINT,
        &mut point as *mut _ as *mut c_void,
    );
    CFRelease(value);
    if ok != 0 { Some(point) } else { None }
}

/// Get the size (AXSize → CGSize).
unsafe fn ax_size(element: CFTypeRef) -> Option<CGSize> {
    let value = ax_attr(element, "AXSize")?;
    let mut size = CGSize::default();
    let ok = AXValueGetValue(value, AX_VALUE_CG_SIZE, &mut size as *mut _ as *mut c_void);
    CFRelease(value);
    if ok != 0 { Some(size) } else { None }
}

unsafe fn ax_bool(element: CFTypeRef, name: &str) -> Option<bool> {
    let value = ax_attr(element, name)?;
    let result =
        (CFGetTypeID(value) == CFBooleanGetTypeID()).then(|| std::ptr::eq(value, kCFBooleanTrue));
    CFRelease(value);
    result
}

unsafe fn try_set_point(element: CFTypeRef, point: CGPoint) -> bool {
    let value = AXValueCreate(AX_VALUE_CG_POINT, &point as *const CGPoint as *const c_void);
    if value.is_null() {
        return false;
    }
    let result = try_set_value(element, "AXPosition", value);
    CFRelease(value);
    result
}

unsafe fn try_set_size(element: CFTypeRef, size: CGSize) -> bool {
    let value = AXValueCreate(AX_VALUE_CG_SIZE, &size as *const CGSize as *const c_void);
    if value.is_null() {
        return false;
    }
    let result = try_set_value(element, "AXSize", value);
    CFRelease(value);
    result
}

// ── Public window discovery (single source of truth for "which window") ────

/// Geometry of an app's authoritative window, as AX sees it. Used by
/// `screenshot::find_window` so the screenshot path agrees with everything
/// else (`cu snapshot`, `cu click`, `cu find` already drive off AX).
#[derive(Debug, Clone, Copy)]
pub struct AxWindowGeom {
    pub window_id: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Resolve the primary window of `pid` via AX (`AXFocusedWindow` →
/// `AXMainWindow` fallback) and return its CGWindowID + bounds. None if
/// AX is unavailable (no a11y permission, or app exposes no AX windows).
///
/// Why AX-first: layer-0 windows in CGWindowList include menu-bar proxies,
/// AX helpers, palette stubs, and minimized stand-ins. CGWindowList has no
/// way to identify which is "the real window"; AX does, by definition.
pub fn focused_window_geom(pid: i32) -> Option<AxWindowGeom> {
    unsafe {
        let app = create_app_element(pid);
        if app.is_null() {
            return None;
        }
        let window = ax_attr(app, "AXFocusedWindow").or_else(|| ax_attr(app, "AXMainWindow"));
        CFRelease(app);
        let window = window?;
        set_element_timeout(window);

        let pos = ax_position(window);
        let size = ax_size(window);
        let mut wid: u32 = 0;
        let err = _AXUIElementGetWindow(window, &mut wid);
        CFRelease(window);

        if err != AX_OK || wid == 0 {
            return None;
        }
        let pos = pos?;
        let size = size?;
        if size.width <= 1.0 || size.height <= 1.0 {
            return None;
        }
        Some(AxWindowGeom {
            window_id: wid,
            x: pos.x,
            y: pos.y,
            width: size.width,
            height: size.height,
        })
    }
}

fn rounded_i64(value: f64) -> i64 {
    if value.is_finite() {
        value.round() as i64
    } else {
        0
    }
}

/// Enumerate one process's windows directly through Accessibility.
pub fn list_windows(pid: i32, app_name: &str) -> Result<Vec<WindowInfo>, String> {
    unsafe {
        let app = create_app_element(pid);
        if app.is_null() {
            return Err("failed to create AX element for application".into());
        }
        let Some(windows) = ax_attr(app, "AXWindows") else {
            CFRelease(app);
            return Ok(Vec::new());
        };
        if CFGetTypeID(windows) != CFArrayGetTypeID() {
            CFRelease(windows);
            CFRelease(app);
            return Ok(Vec::new());
        }

        let mut result = Vec::new();
        for index in 0..CFArrayGetCount(windows) {
            let window = CFArrayGetValueAtIndex(windows, index);
            if window.is_null() {
                continue;
            }
            set_element_timeout(window);
            let position = ax_position(window).unwrap_or_default();
            let size = ax_size(window).unwrap_or_default();
            result.push(WindowInfo {
                app: app_name.to_string(),
                index: index as usize + 1,
                title: ax_string(window, "AXTitle").unwrap_or_default(),
                x: rounded_i64(position.x),
                y: rounded_i64(position.y),
                width: rounded_i64(size.width),
                height: rounded_i64(size.height),
                minimized: ax_bool(window, "AXMinimized").unwrap_or(false),
                focused: ax_bool(window, "AXMain")
                    .or_else(|| ax_bool(window, "AXFocused"))
                    .unwrap_or(false),
            });
        }
        CFRelease(windows);
        CFRelease(app);
        Ok(result)
    }
}

/// Mutate one process's indexed window directly through Accessibility.
pub fn window_action(
    pid: i32,
    action: &str,
    window_index: usize,
    arg1: Option<i64>,
    arg2: Option<i64>,
) -> Result<&'static str, String> {
    if window_index == 0 {
        return Err("window index must be at least 1".into());
    }
    match action {
        "move" if arg1.is_none() || arg2.is_none() => return Err("move requires x y".into()),
        "resize" if arg1.is_none() || arg2.is_none() => {
            return Err("resize requires width height".into());
        }
        "resize" if arg1.unwrap_or(0) <= 0 || arg2.unwrap_or(0) <= 0 => {
            return Err("resize requires positive width and height".into());
        }
        _ => {}
    }
    unsafe {
        let app = create_app_element(pid);
        if app.is_null() {
            return Err("failed to create AX element for application".into());
        }
        let Some(windows) = ax_attr(app, "AXWindows") else {
            CFRelease(app);
            return Err(format!("window not found: index {window_index}"));
        };
        let count = if CFGetTypeID(windows) == CFArrayGetTypeID() {
            CFArrayGetCount(windows)
        } else {
            0
        };
        if window_index > count as usize {
            CFRelease(windows);
            CFRelease(app);
            return Err(format!(
                "window not found: index {window_index} (app has {count})"
            ));
        }
        let window = CFArrayGetValueAtIndex(windows, window_index as CFIndex - 1);
        set_element_timeout(window);
        let changed = match action {
            "move" => {
                let x = arg1.unwrap_or_default();
                let y = arg2.unwrap_or_default();
                try_set_point(
                    window,
                    CGPoint {
                        x: x as f64,
                        y: y as f64,
                    },
                )
            }
            "resize" => {
                let width = arg1.unwrap_or_default();
                let height = arg2.unwrap_or_default();
                try_set_size(
                    window,
                    CGSize {
                        width: width as f64,
                        height: height as f64,
                    },
                )
            }
            "focus" => {
                let frontmost = try_set_bool(app, "AXFrontmost", true);
                let main = try_set_bool(window, "AXMain", true);
                let raised = try_action(window, "AXRaise");
                frontmost || main || raised
            }
            "minimize" => try_set_bool(window, "AXMinimized", true),
            "unminimize" => try_set_bool(window, "AXMinimized", false),
            "close" => {
                let close_button = ax_attr(window, "AXCloseButton");
                let pressed = close_button
                    .map(|button| {
                        let result = try_action(button, "AXPress");
                        CFRelease(button);
                        result
                    })
                    .unwrap_or(false);
                pressed || try_action(window, "AXClose")
            }
            other => {
                CFRelease(windows);
                CFRelease(app);
                return Err(format!(
                    "unknown window action: {other} (use: list, move, resize, focus, minimize, unminimize, close)"
                ));
            }
        };
        CFRelease(windows);
        CFRelease(app);
        if changed {
            Ok(if action == "focus" {
                "ax-raise"
            } else {
                "ax-window"
            })
        } else {
            Err(format!("AX {action} was rejected by window {window_index}"))
        }
    }
}

/// Enumerate the top-level items in an app's menu bar through Accessibility.
pub fn list_menu(pid: i32) -> Result<Vec<MenuItem>, String> {
    unsafe {
        let app = create_app_element(pid);
        if app.is_null() {
            return Err("failed to create AX element for application".into());
        }
        let Some(menu_bar) = ax_attr(app, "AXMenuBar") else {
            CFRelease(app);
            return Ok(Vec::new());
        };
        let Some(menu_bar_items) = ax_attr(menu_bar, "AXChildren") else {
            CFRelease(menu_bar);
            CFRelease(app);
            return Ok(Vec::new());
        };
        let mut result = Vec::new();
        if CFGetTypeID(menu_bar_items) == CFArrayGetTypeID() {
            for menu_index in 0..CFArrayGetCount(menu_bar_items) {
                let menu_bar_item = CFArrayGetValueAtIndex(menu_bar_items, menu_index);
                if menu_bar_item.is_null() {
                    continue;
                }
                let menu_name = ax_string(menu_bar_item, "AXTitle").unwrap_or_default();
                let Some(menus) = ax_attr(menu_bar_item, "AXChildren") else {
                    continue;
                };
                if CFGetTypeID(menus) == CFArrayGetTypeID() {
                    for child_index in 0..CFArrayGetCount(menus) {
                        let menu = CFArrayGetValueAtIndex(menus, child_index);
                        if menu.is_null() {
                            continue;
                        }
                        let Some(items) = ax_attr(menu, "AXChildren") else {
                            continue;
                        };
                        if CFGetTypeID(items) == CFArrayGetTypeID() {
                            for item_index in 0..CFArrayGetCount(items) {
                                let item = CFArrayGetValueAtIndex(items, item_index);
                                if item.is_null() {
                                    continue;
                                }
                                let title = ax_string(item, "AXTitle").unwrap_or_default();
                                if title.is_empty() {
                                    continue;
                                }
                                result.push(MenuItem {
                                    menu: menu_name.clone(),
                                    item: title,
                                    enabled: ax_bool(item, "AXEnabled").unwrap_or(true),
                                });
                            }
                        }
                        CFRelease(items);
                    }
                }
                CFRelease(menus);
            }
        }
        CFRelease(menu_bar_items);
        CFRelease(menu_bar);
        CFRelease(app);
        Ok(result)
    }
}

// ── Role filtering ──────────────────────────────────────────────────────────

const INCLUDED_ROLES: &[&str] = &[
    "AXButton",
    "AXTextField",
    "AXTextArea",
    "AXStaticText",
    "AXRow",
    "AXCell",
    "AXCheckBox",
    "AXRadioButton",
    "AXPopUpButton",
    "AXComboBox",
    "AXLink",
    "AXMenuItem",
    "AXMenuButton",
    "AXTab",
    "AXSlider",
    "AXImage",
];

fn is_included(role: &str) -> bool {
    INCLUDED_ROLES.contains(&role)
}

/// "AXButton" → "button", "AXStaticText" → "statictext"
fn normalize_role(role: &str) -> String {
    role.strip_prefix("AX").unwrap_or(role).to_lowercase()
}

// ── Batch attribute reading ──────────────────────────────────────────────────

// Attribute indices in the batch array (order must match BATCH_ATTRS)
const BA_ROLE: usize = 0;
const BA_TITLE: usize = 1;
const BA_DESC: usize = 2;
const BA_VALUE: usize = 3;
const BA_POS: usize = 4;
const BA_SIZE: usize = 5;
const BA_CHILDREN: usize = 6;
const BA_HELP: usize = 7;
const BA_IDENTIFIER: usize = 8;
const BATCH_ATTR_NAMES: &[&str] = &[
    "AXRole",
    "AXTitle",
    "AXDescription",
    "AXValue",
    "AXPosition",
    "AXSize",
    "AXChildren",
    // Extra label sources in fallback chain (R5). Electron/CEF apps often
    // set AXTitle to internal IDs ("submit_btn_primary") while the
    // user-visible label lives in AXHelp (tooltip) or AXIdentifier (aria-label).
    // Adding two batch keys is one extra IPC field per element — negligible
    // at typical 200-element snapshots.
    "AXHelp",
    "AXIdentifier",
];

/// Create the CFArray of attribute name strings. Returns null on failure.
unsafe fn create_batch_keys() -> CFArrayRef {
    let mut keys: Vec<CFTypeRef> = Vec::with_capacity(BATCH_ATTR_NAMES.len());
    for name in BATCH_ATTR_NAMES {
        match cfstr(name) {
            Some(k) => keys.push(k),
            None => {
                // Allocation failed — release what we have and bail
                for k in &keys {
                    CFRelease(*k);
                }
                return std::ptr::null();
            }
        }
    }
    assert_eq!(
        keys.len(),
        BATCH_ATTR_NAMES.len(),
        "batch key count mismatch"
    );
    let array = CFArrayCreate(
        std::ptr::null(),
        keys.as_ptr(),
        keys.len() as CFIndex,
        &kCFTypeArrayCallBacks as *const _ as CFTypeRef,
    );
    // Release the individual strings (array retains them)
    for k in &keys {
        CFRelease(*k);
    }
    array
}

/// Read all batch attributes from an element in a single IPC call.
/// Returns the values array (caller must CFRelease), or null on failure.
unsafe fn batch_read(element: CFTypeRef, keys: CFArrayRef) -> CFArrayRef {
    // Test-only fault injection used by the O1 behavior gate. The production
    // path never sets these variables; keeping the seam here exercises the
    // same per-node fallback that handles a real AX error marker.
    let fault_role = test_ax_faults().batch_fail_role.as_deref();
    if fault_role.is_some() && ax_string(element, "AXRole").as_deref() == fault_role {
        return std::ptr::null();
    }
    let mut values: CFArrayRef = std::ptr::null();
    let err = AXUIElementCopyMultipleAttributeValues(element, keys, 0, &mut values);
    if err == AX_OK && !values.is_null() {
        values
    } else {
        std::ptr::null()
    }
}

struct TestAxFaults {
    batch_fail_role: Option<String>,
    children_fallback_role: Option<String>,
}

fn test_ax_faults() -> &'static TestAxFaults {
    static FAULTS: OnceLock<TestAxFaults> = OnceLock::new();
    FAULTS.get_or_init(|| TestAxFaults {
        batch_fail_role: std::env::var("CU_TEST_AX_BATCH_FAIL_ROLE").ok(),
        children_fallback_role: std::env::var("CU_TEST_AX_BATCH_CHILDREN_FALLBACK_ROLE").ok(),
    })
}

/// Extract a string from position `idx` in the batch values array.
unsafe fn batch_string(values: CFArrayRef, idx: usize) -> Option<String> {
    let count = CFArrayGetCount(values) as usize;
    if idx >= count {
        return None;
    }
    let val = CFArrayGetValueAtIndex(values, idx as CFIndex);
    if val.is_null() {
        return None;
    }
    // Check it's actually a CFString (not NSNull / error marker)
    if CFGetTypeID(val) != CFStringGetTypeID() {
        return None;
    }
    cfstring_to_string(val)
}

/// Extract position (CGPoint) from batch values.
unsafe fn batch_position(values: CFArrayRef, idx: usize) -> Option<CGPoint> {
    let count = CFArrayGetCount(values) as usize;
    if idx >= count {
        return None;
    }
    let val = CFArrayGetValueAtIndex(values, idx as CFIndex);
    if val.is_null() {
        return None;
    }
    let mut point = CGPoint::default();
    if AXValueGetValue(val, AX_VALUE_CG_POINT, &mut point as *mut _ as *mut c_void) != 0 {
        Some(point)
    } else {
        None
    }
}

/// Extract size (CGSize) from batch values.
unsafe fn batch_size(values: CFArrayRef, idx: usize) -> Option<CGSize> {
    let count = CFArrayGetCount(values) as usize;
    if idx >= count {
        return None;
    }
    let val = CFArrayGetValueAtIndex(values, idx as CFIndex);
    if val.is_null() {
        return None;
    }
    let mut size = CGSize::default();
    if AXValueGetValue(val, AX_VALUE_CG_SIZE, &mut size as *mut _ as *mut c_void) != 0 {
        Some(size)
    } else {
        None
    }
}

/// Extract children array from batch values (not retained — use before releasing batch).
unsafe fn batch_children(values: CFArrayRef, idx: usize) -> Option<CFArrayRef> {
    let count = CFArrayGetCount(values) as usize;
    if idx >= count {
        return None;
    }
    let val = CFArrayGetValueAtIndex(values, idx as CFIndex);
    if val.is_null() {
        return None;
    }
    if CFGetTypeID(val) != CFArrayGetTypeID() {
        return None;
    }
    Some(val)
}

// ── Canonical ref traversal ────────────────────────────────────────────────

const MAX_DEPTH: usize = 30;

/// The sole policy for assigning numeric refs.
///
/// A node consumes a ref exactly when its AX role is in `INCLUDED_ROLES` and
/// at least one dimension of its AX bounding box is positive.  Keeping this
/// decision pure makes it usable by both the batch snapshot reader and the
/// single-element action reader; neither reader may invent its own counting
/// rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RefProjection {
    emits_ref: bool,
}

impl RefProjection {
    fn from_parts(role: Option<&str>, size: CGSize) -> Self {
        Self {
            emits_ref: role.is_some_and(is_included) && (size.width > 0.0 || size.height > 0.0),
        }
    }

    fn assign(self, counter: &mut usize) -> Option<usize> {
        if self.emits_ref {
            *counter += 1;
            Some(*counter)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod ref_projection_tests {
    use super::{CGSize, RefProjection};

    #[test]
    fn only_included_positive_geometry_consumes_a_ref() {
        let mut counter = 0;
        assert_eq!(
            RefProjection::from_parts(
                Some("AXButton"),
                CGSize {
                    width: 10.0,
                    height: 0.0
                }
            )
            .assign(&mut counter),
            Some(1)
        );
        assert_eq!(
            RefProjection::from_parts(
                Some("AXGroup"),
                CGSize {
                    width: 10.0,
                    height: 10.0
                }
            )
            .assign(&mut counter),
            None
        );
        assert_eq!(
            RefProjection::from_parts(
                Some("AXButton"),
                CGSize {
                    width: 0.0,
                    height: 0.0
                }
            )
            .assign(&mut counter),
            None
        );
        assert_eq!(counter, 1);
    }

    #[test]
    fn preorder_counter_skips_static_and_zero_size_nodes_without_gaps() {
        let nodes = [
            (Some("AXGroup"), 100.0, 100.0),
            (Some("AXButton"), 0.0, 0.0),
            (Some("AXStaticText"), 8.0, 8.0),
            (Some("AXButton"), 20.0, 10.0),
        ];
        let mut counter = 0;
        let refs: Vec<_> = nodes
            .iter()
            .filter_map(|(role, width, height)| {
                RefProjection::from_parts(
                    *role,
                    CGSize {
                        width: *width,
                        height: *height,
                    },
                )
                .assign(&mut counter)
            })
            .collect();
        assert_eq!(refs, [1, 2]);
        assert_eq!(counter, 2);
    }
}

/// Attributes needed by the canonical traversal. `children` is borrowed from
/// one of the pointers in `owned`; those pointers are released only after the
/// node's descendants have been visited.
struct RefNodeRead {
    role: Option<String>,
    title: Option<String>,
    value: Option<String>,
    position: Option<CGPoint>,
    size: CGSize,
    children: Option<CFArrayRef>,
    owned: Vec<CFTypeRef>,
}

impl RefNodeRead {
    unsafe fn release(self) {
        for pointer in self.owned {
            if !pointer.is_null() {
                CFRelease(pointer);
            }
        }
    }
}

/// Read an array-valued AXChildren attribute and retain it for the caller.
/// Non-array values are released immediately and treated as unavailable.
unsafe fn retained_children(element: CFTypeRef) -> (Option<CFArrayRef>, Vec<CFTypeRef>) {
    let Some(children) = ax_attr(element, "AXChildren") else {
        return (None, Vec::new());
    };
    if CFGetTypeID(children) == CFArrayGetTypeID() {
        (Some(children), vec![children])
    } else {
        CFRelease(children);
        (None, Vec::new())
    }
}

/// Read the fields that action and inspection consumers need. All of those
/// consumers now use this same reader and traversal, so unreadable/static
/// nodes are treated identically for click, find, set-value, perform, and why.
unsafe fn read_single_ref_node(element: CFTypeRef) -> RefNodeRead {
    let role = ax_string(element, "AXRole");
    let projected = role.as_deref().is_some_and(is_included);
    let size = if projected {
        ax_size(element).unwrap_or_default()
    } else {
        CGSize::default()
    };
    let (children, owned) = retained_children(element);
    RefNodeRead {
        role,
        title: None,
        value: None,
        position: None,
        size,
        children,
        owned,
    }
}

/// Read a batch field, retrying the individual attribute when the batch slot
/// is absent or contains an AX error marker. This keeps a partial batch read
/// from changing the ref projection or dropping a whole child subtree.
unsafe fn batch_string_with_fallback(
    values: CFArrayRef,
    index: usize,
    element: CFTypeRef,
    name: &str,
) -> Option<String> {
    batch_string(values, index).or_else(|| ax_string(element, name))
}

unsafe fn batch_position_with_fallback(values: CFArrayRef, element: CFTypeRef) -> Option<CGPoint> {
    batch_position(values, BA_POS).or_else(|| ax_position(element))
}

unsafe fn batch_size_with_fallback(values: CFArrayRef, element: CFTypeRef) -> CGSize {
    batch_size(values, BA_SIZE)
        .or_else(|| ax_size(element))
        .unwrap_or_default()
}

/// Snapshot reader. The batch array remains in `owned` while its borrowed
/// AXChildren array is traversed; if that slot is unreadable, AXChildren is
/// fetched separately and retained instead.
unsafe fn read_batch_ref_node(element: CFTypeRef, batch_keys: CFArrayRef) -> RefNodeRead {
    let values = batch_read(element, batch_keys);
    if values.is_null() {
        let role = ax_string(element, "AXRole");
        let projected = role.as_deref().is_some_and(is_included);
        let size = if projected {
            ax_size(element).unwrap_or_default()
        } else {
            CGSize::default()
        };
        let title = if projected {
            ax_string(element, "AXTitle")
                .or_else(|| ax_string(element, "AXDescription"))
                .or_else(|| ax_string(element, "AXHelp"))
                .or_else(|| ax_string(element, "AXIdentifier"))
                .filter(|s| !s.is_empty())
        } else {
            None
        };
        let value = if projected {
            ax_string(element, "AXValue").filter(|s| !s.is_empty())
        } else {
            None
        };
        let position = if projected {
            ax_position(element)
        } else {
            None
        };
        let (children, owned) = retained_children(element);
        return RefNodeRead {
            role,
            title,
            value,
            position,
            size,
            children,
            owned,
        };
    }

    let role = batch_string_with_fallback(values, BA_ROLE, element, "AXRole");
    let projected = role.as_deref().is_some_and(is_included);
    let size = if projected {
        batch_size_with_fallback(values, element)
    } else {
        CGSize::default()
    };
    let title = if projected {
        batch_string_with_fallback(values, BA_TITLE, element, "AXTitle")
            .or_else(|| batch_string_with_fallback(values, BA_DESC, element, "AXDescription"))
            .or_else(|| batch_string_with_fallback(values, BA_HELP, element, "AXHelp"))
            .or_else(|| batch_string_with_fallback(values, BA_IDENTIFIER, element, "AXIdentifier"))
            .filter(|s| !s.is_empty())
    } else {
        None
    };
    let value = if projected {
        batch_string_with_fallback(values, BA_VALUE, element, "AXValue").filter(|s| !s.is_empty())
    } else {
        None
    };
    let position = if projected {
        batch_position_with_fallback(values, element)
    } else {
        None
    };

    // `values` owns/retains its child array. When the slot is valid, keeping
    // only `values` alive is sufficient; when it is an error marker, the
    // separately fetched children array is added to the ownership chain.
    let mut owned = vec![values];
    let fallback_role = test_ax_faults().children_fallback_role.as_deref();
    let force_children_fallback = fallback_role.is_some() && fallback_role == role.as_deref();
    let children =
        if !force_children_fallback && let Some(children) = batch_children(values, BA_CHILDREN) {
            Some(children)
        } else {
            let (children, mut child_owned) = retained_children(element);
            owned.append(&mut child_owned);
            children
        };

    RefNodeRead {
        role,
        title,
        value,
        position,
        size,
        children,
        owned,
    }
}

/// Visit control for the canonical DFS. `Break` returns the consumer's result
/// and stops traversal immediately (used by both limits and ref lookups).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefVisit<T> {
    Continue,
    Break(T),
}

/// One preorder traversal for every numeric-ref producer and consumer.
/// `track_paths` is false for action-only consumers to avoid path-label IPC;
/// it does not affect inclusion or ref numbering.
#[allow(clippy::too_many_arguments)]
unsafe fn traverse_ref_tree<T, Reader, Visitor>(
    element: CFTypeRef,
    counter: &mut usize,
    depth: usize,
    track_paths: bool,
    my_segment: &str,
    parent_path: &str,
    depth_limited: &mut bool,
    reader: &mut Reader,
    visitor: &mut Visitor,
) -> Option<T>
where
    Reader: FnMut(CFTypeRef) -> RefNodeRead,
    Visitor: FnMut(CFTypeRef, &RefNodeRead, Option<usize>, &str) -> RefVisit<T>,
{
    if depth > MAX_DEPTH {
        *depth_limited = true;
        return None;
    }

    let node = reader(element);
    let ref_id = RefProjection::from_parts(node.role.as_deref(), node.size).assign(counter);
    let self_path = if track_paths {
        if parent_path.is_empty() {
            my_segment.to_string()
        } else {
            format!("{parent_path}/{my_segment}")
        }
    } else {
        String::new()
    };

    if let RefVisit::Break(result) = visitor(element, &node, ref_id, &self_path) {
        node.release();
        return Some(result);
    }

    if let Some(children) = node.children {
        let count = CFArrayGetCount(children);
        let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for i in 0..count {
            let child = CFArrayGetValueAtIndex(children, i);
            if child.is_null() {
                continue;
            }
            let child_segment = if track_paths {
                compute_child_segment(child, &mut seen)
            } else {
                String::new()
            };
            if let Some(result) = traverse_ref_tree(
                child,
                counter,
                depth + 1,
                track_paths,
                &child_segment,
                &self_path,
                depth_limited,
                reader,
                visitor,
            ) {
                node.release();
                return Some(result);
            }
        }
    }

    node.release();
    None
}

/// Compute the path segment for `child` (Role[Title]:N). `seen` tracks how
/// many earlier siblings produced the same `Role[Title]` so we can append
/// the `:N` disambiguator (omitted when `N == 0`). Mutates `seen` in place.
unsafe fn compute_child_segment(
    child: CFTypeRef,
    seen: &mut std::collections::HashMap<String, usize>,
) -> String {
    let role = ax_string(child, "AXRole").unwrap_or_default();
    let title = ax_string(child, "AXTitle")
        .or_else(|| ax_string(child, "AXDescription"))
        .filter(|s| !s.is_empty());
    let base = build_path_segment(&role, title.as_deref());
    let idx_ref = seen.entry(base.clone()).or_insert(0);
    let idx = *idx_ref;
    *idx_ref += 1;
    if idx == 0 {
        base
    } else {
        format!("{base}:{idx}")
    }
}

// ── AX action helpers ───────────────────────────────────────────────────────

/// 15-step AX click chain (learned from agent-desktop):
///  1-4:  Direct actions (AXPress, AXConfirm, AXOpen, AXPick)
///  5:    ShowAlternateUI (menus, dock items)
///  6:    Child element actions (container buttons)
///  7:    Set AXValue directly (checkboxes, sliders)
///  8:    Set AXSelected=true (list items)
///  9-10: Parent row/table selection
///  11:   Custom actions
///  12:   Focus + press/confirm
///  13:   Keyboard spacebar (universal button trigger)
///  14:   Ancestor press/confirm (walk up)
///  15:   CGEvent mouse click (handled by caller)
unsafe fn try_ax_actions(element: CFTypeRef) -> Option<&'static str> {
    // Text inputs are focus targets, not buttons. Chromium exposes AXPress on
    // contenteditable elements and returns AX_OK even when that action does
    // not move AXFocusedUIElement. Prefer the semantic AXFocused write and
    // stop once it succeeds; typing is only allowed after a fresh snapshot
    // confirms the same element is focused.
    let role = ax_string(element, "AXRole").unwrap_or_default();
    if matches!(role.as_str(), "AXTextField" | "AXTextArea" | "AXComboBox")
        && try_set_bool(element, "AXFocused", true)
    {
        return Some("ax-focus");
    }

    // Steps 1-4: Direct actions
    for action in &["AXPress", "AXConfirm", "AXOpen", "AXPick"] {
        if try_action(element, action) {
            return Some("ax-action");
        }
    }

    // Step 5: ShowAlternateUI
    if try_action(element, "AXShowAlternateUI") {
        return Some("ax-alt-ui");
    }

    // Step 6: Child element actions (try press/confirm on first child)
    if let Some(children) = ax_attr(element, "AXChildren") {
        if CFGetTypeID(children) == CFArrayGetTypeID() && CFArrayGetCount(children) > 0 {
            let child = CFArrayGetValueAtIndex(children, 0);
            if !child.is_null() {
                for action in &["AXPress", "AXConfirm", "AXOpen"] {
                    if try_action(child, action) {
                        CFRelease(children);
                        return Some("ax-child-action");
                    }
                }
            }
        }
        CFRelease(children);
    }

    // Step 7: Toggle AXValue for checkboxes/switches.
    //
    // We read the current value and write the opposite, so an "uncheck"
    // request actually unchecks. The previous version forced AXValue=true
    // first; on an already-checked control that would silently succeed
    // (no UI change) and the action would falsely report "clicked". A
    // type check guards against the rare cases where AXValue isn't a
    // CFBoolean — in that case we fall through to the CGEvent click
    // path so the toggle still happens, just via a real mouse event.
    if (role == "AXCheckBox" || role == "AXSwitch")
        && let Some(current) = ax_attr(element, "AXValue")
    {
        let is_bool = CFGetTypeID(current) == CFBooleanGetTypeID();
        if is_bool {
            let is_on = std::ptr::eq(current, kCFBooleanTrue);
            CFRelease(current);
            let new_val = if is_on {
                kCFBooleanFalse
            } else {
                kCFBooleanTrue
            };
            if try_set_value(element, "AXValue", new_val) {
                return Some("ax-toggle");
            }
        } else {
            CFRelease(current);
        }
    }

    // Step 8: Set AXSelected=true
    if try_set_bool(element, "AXSelected", true) {
        return Some("ax-selected");
    }

    // Steps 9-10: Parent row/table selection
    if let Some(parent) = ax_attr(element, "AXParent") {
        let parent_role = ax_string(parent, "AXRole").unwrap_or_default();
        if parent_role == "AXRow" || parent_role == "AXCell" {
            if try_set_bool(parent, "AXSelected", true) {
                CFRelease(parent);
                return Some("ax-parent-select");
            }
            // Try grandparent (table)
            if let Some(grandparent) = ax_attr(parent, "AXParent") {
                if try_set_bool(grandparent, "AXSelected", true) {
                    CFRelease(grandparent);
                    CFRelease(parent);
                    return Some("ax-table-select");
                }
                CFRelease(grandparent);
            }
        }
        CFRelease(parent);
    }

    // Step 11: Custom actions (try all available actions)
    // (covered by steps 1-5 which already try the standard actions)

    // Step 12: Focus element, then try press/confirm
    if try_set_bool(element, "AXFocused", true) {
        for action in &["AXPress", "AXConfirm"] {
            if try_action(element, action) {
                return Some("ax-focus-press");
            }
        }
    }

    // Step 13: Keyboard spacebar (handled by caller via CGEvent if needed)
    // Step 14: Ancestor press/confirm
    if let Some(parent) = ax_attr(element, "AXParent") {
        for action in &["AXPress", "AXConfirm"] {
            if try_action(parent, action) {
                CFRelease(parent);
                return Some("ax-ancestor-press");
            }
        }
        CFRelease(parent);
    }

    // Step 15: CGEvent — handled by caller
    None
}

unsafe fn try_action(element: CFTypeRef, action: &str) -> bool {
    let Some(action_str) = cfstr(action) else {
        return false;
    };
    let err = AXUIElementPerformAction(element, action_str);
    CFRelease(action_str);
    err == AX_OK
}

unsafe fn try_set_value(element: CFTypeRef, attr: &str, val: CFTypeRef) -> bool {
    let Some(key) = cfstr(attr) else { return false };
    let err = AXUIElementSetAttributeValue(element, key, val);
    CFRelease(key);
    err == AX_OK
}

unsafe fn try_set_bool(element: CFTypeRef, attr: &str, val: bool) -> bool {
    let Some(key) = cfstr(attr) else { return false };
    // Create CFBoolean
    let cf_bool: CFTypeRef = if val { kCFBooleanTrue } else { kCFBooleanFalse };
    let err = AXUIElementSetAttributeValue(element, key, cf_bool);
    CFRelease(key);
    err == AX_OK
}

/// List the AX actions an element supports (e.g. ["AXPress", "AXShowMenu"]).
/// Empty vec means the element exposes no actions or the call failed.
unsafe fn copy_action_names(element: CFTypeRef) -> Vec<String> {
    let mut names: CFArrayRef = std::ptr::null();
    let err = AXUIElementCopyActionNames(element, &mut names);
    if err != AX_OK || names.is_null() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if CFGetTypeID(names) == CFArrayGetTypeID() {
        let count = CFArrayGetCount(names);
        for i in 0..count {
            let item = CFArrayGetValueAtIndex(names, i);
            if !item.is_null()
                && let Some(s) = cfstring_to_string(item)
            {
                out.push(s);
            }
        }
    }
    CFRelease(names);
    out
}

/// Resolve a numeric ref through the canonical traversal and run a callback
/// when that ref is reached. The callback owns only the duration of the call;
/// AX children remain alive until the traversal unwinds.
unsafe fn resolve_ref_with<T, F>(root: CFTypeRef, ref_id: usize, perform: F) -> (Option<T>, usize)
where
    F: FnOnce(CFTypeRef, &RefNodeRead, usize) -> T,
{
    let mut counter = 0usize;
    let mut depth_limited = false;
    let mut perform = Some(perform);
    let mut reader = |element| read_single_ref_node(element);
    let mut visitor = |element: CFTypeRef,
                       node: &RefNodeRead,
                       candidate: Option<usize>,
                       _path: &str|
     -> RefVisit<T> {
        if candidate == Some(ref_id) {
            let callback = perform
                .take()
                .expect("canonical ref callback invoked more than once");
            RefVisit::Break(callback(element, node, ref_id))
        } else {
            RefVisit::Continue
        }
    };
    let result = traverse_ref_tree(
        root,
        &mut counter,
        0,
        false,
        "",
        "",
        &mut depth_limited,
        &mut reader,
        &mut visitor,
    );
    (result, counter)
}

// ── Public API ──────────────────────────────────────────────────────────────

fn resolve_ref(pid: i32, ref_id: usize, perform_actions: bool) -> Result<(bool, f64, f64), String> {
    unsafe {
        let app_el = create_app_element(pid);
        if app_el.is_null() {
            return Err("failed to create AX element for application".into());
        }

        let window_el =
            ax_attr(app_el, "AXFocusedWindow").or_else(|| ax_attr(app_el, "AXMainWindow"));
        if let Some(w) = window_el {
            set_element_timeout(w);
        }

        let walk_root = window_el.unwrap_or(app_el);

        let (result, counter) = resolve_ref_with(walk_root, ref_id, |element, node, _| {
            let pos = node
                .position
                .or_else(|| ax_position(element))
                .unwrap_or_default();
            let cx = pos.x + node.size.width / 2.0;
            let cy = pos.y + node.size.height / 2.0;
            let acted = if perform_actions {
                try_ax_actions(element).is_some()
            } else {
                false
            };
            (acted, cx, cy)
        });

        if let Some(w) = window_el {
            CFRelease(w);
        }
        CFRelease(app_el);

        match result {
            Some((acted, cx, cy)) => Ok((acted, cx, cy)),
            None => Err(format!(
                "element [{}] not found in AX tree (scanned {} elements)",
                ref_id, counter
            )),
        }
    }
}

/// Find element by ref and try AX actions. Returns (ax_acted, center_x, center_y).
pub fn ax_click(pid: i32, ref_id: usize, _limit: usize) -> Result<(bool, f64, f64), String> {
    resolve_ref(pid, ref_id, true)
}

// ── axPath resolution (A2) ──────────────────────────────────────────────────

/// Parse a path segment into (role, optional title, sibling index).
/// Format: `Role[Title]:N`. `[Title]` and `:N` are both optional.
fn parse_path_segment(seg: &str) -> (String, Option<String>, usize) {
    // Split off `:N` suffix if present (only at the end, after any `]`).
    let (head, idx) = if let Some(colon_pos) = seg.rfind(':') {
        let after = &seg[colon_pos + 1..];
        if !after.is_empty() && after.chars().all(|c| c.is_ascii_digit()) {
            (&seg[..colon_pos], after.parse::<usize>().unwrap_or(0))
        } else {
            (seg, 0)
        }
    } else {
        (seg, 0)
    };

    // Split off `[Title]` if present.
    if let Some(open) = head.find('[')
        && head.ends_with(']')
    {
        let role = &head[..open];
        let title = &head[open + 1..head.len() - 1];
        return (role.to_string(), Some(title.to_string()), idx);
    }
    (head.to_string(), None, idx)
}

// ── axPath descent: shared implementation ───────────────────────────────────

/// Returns `true` when this child's role+title pair matches the segment
/// (`role` + optional `title`). The `:N` index is handled by the caller.
unsafe fn child_matches_segment(child: CFTypeRef, role: &str, title: Option<&str>) -> bool {
    let child_role = ax_string(child, "AXRole").unwrap_or_default();
    if normalize_role(&child_role) != role {
        return false;
    }
    let child_title = ax_string(child, "AXTitle")
        .or_else(|| ax_string(child, "AXDescription"))
        .filter(|s| !s.is_empty());
    match (title, child_title.as_deref()) {
        (Some(want), Some(got)) => {
            // Compare via the same sanitization the writer used so titles
            // containing `/`, `[`, `]` round-trip correctly.
            build_path_segment(role, Some(want)) == build_path_segment(&child_role, Some(got))
        }
        // Path segment had no [Title]: child must also have no title to match.
        (None, None) => true,
        _ => false,
    }
}

/// Result of a successful axPath descent. Holds the matched element pointer
/// plus the chain of `AXChildren` arrays that must outlive `element` (children
/// own the references the descent used). Caller drops via `release()`.
struct AxPathMatch {
    element: CFTypeRef,
    /// AXChildren arrays, ordered from shallowest to deepest. They keep
    /// `element` alive until cleanup.
    owned: Vec<CFTypeRef>,
}

impl AxPathMatch {
    /// Release the children arrays we kept alive during descent. Must be
    /// called exactly once. Intentionally not `Drop` — we want the unsafety
    /// to be visible at the call site.
    unsafe fn release(self) {
        for r in &self.owned {
            if !r.is_null() {
                CFRelease(*r);
            }
        }
    }
}

/// Walk `walk_root` top-down, matching each path segment against the element
/// (depth 0) or its `AXChildren` (depth ≥ 1). Sibling disambiguation honors
/// the `:N` index. Returns the matched element on success.
unsafe fn descend_to_ax_path(
    walk_root: CFTypeRef,
    segments: &[(String, Option<String>, usize)],
) -> Result<AxPathMatch, String> {
    if segments.is_empty() {
        return Err("axPath is empty".into());
    }

    let mut current = walk_root;
    let mut owned: Vec<CFTypeRef> = Vec::new();

    for (depth, (role, title, idx)) in segments.iter().enumerate() {
        if depth == 0 {
            // The root segment must match walk_root itself (window or app);
            // we never descend at this depth. Sibling indices > 0 are nonsense
            // for the root.
            if !child_matches_segment(current, role, title.as_deref()) {
                for r in &owned {
                    if !r.is_null() {
                        CFRelease(*r);
                    }
                }
                return Err(format!("axPath root did not match (expected '{role}')"));
            }
            if *idx != 0 {
                for r in &owned {
                    if !r.is_null() {
                        CFRelease(*r);
                    }
                }
                return Err("axPath root cannot have a :N suffix > 0".into());
            }
            continue;
        }

        let Some(children) = ax_attr(current, "AXChildren") else {
            for r in &owned {
                if !r.is_null() {
                    CFRelease(*r);
                }
            }
            return Err(format!("axPath has no children at depth {depth}"));
        };

        let mut found: Option<CFTypeRef> = None;
        let mut match_count: usize = 0;
        if CFGetTypeID(children) == CFArrayGetTypeID() {
            let count = CFArrayGetCount(children);
            for i in 0..count {
                let child = CFArrayGetValueAtIndex(children, i);
                if child.is_null() || !child_matches_segment(child, role, title.as_deref()) {
                    continue;
                }
                if match_count == *idx {
                    found = Some(child);
                    break;
                }
                match_count += 1;
            }
        }

        match found {
            Some(child) => {
                // Keep `children` alive — `child` is borrowed from it.
                owned.push(children);
                current = child;
            }
            None => {
                CFRelease(children);
                for r in &owned {
                    if !r.is_null() {
                        CFRelease(*r);
                    }
                }
                return Err(format!(
                    "no match for axPath segment '{role}' at depth {depth}"
                ));
            }
        }
    }

    Ok(AxPathMatch {
        element: current,
        owned,
    })
}

/// Parse, descend, and run `f` on the matched element. Encapsulates the
/// app/window resolution + cleanup so callers focus on the action they want
/// to perform on the matched element.
fn with_ax_path<F, R>(pid: i32, ax_path: &str, f: F) -> Result<R, String>
where
    F: FnOnce(CFTypeRef) -> R,
{
    let segments: Vec<_> = ax_path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(parse_path_segment)
        .collect();
    if segments.is_empty() {
        return Err("axPath is empty or all-slash".into());
    }

    unsafe {
        let app_el = create_app_element(pid);
        if app_el.is_null() {
            return Err("failed to create AX element for application".into());
        }

        // axPath is rooted at AXWindow (matches snapshot's `walk_root`), so
        // resolve via window when the first segment is a window role.
        let walk_root = if segments[0].0 == "window" {
            ax_attr(app_el, "AXFocusedWindow").or_else(|| ax_attr(app_el, "AXMainWindow"))
        } else {
            None
        };
        let root = walk_root.unwrap_or(app_el);
        if walk_root.is_some() {
            set_element_timeout(root);
        }

        let descent = descend_to_ax_path(root, &segments);
        let result = match descent {
            Ok(m) => {
                let r = f(m.element);
                m.release();
                Ok(r)
            }
            Err(e) => Err(e),
        };

        if let Some(w) = walk_root {
            CFRelease(w);
        }
        CFRelease(app_el);
        result
    }
}

/// Public entry: resolve an axPath against the app's AX tree and (optionally)
/// fire the AX action chain. Returns `(acted, center_x, center_y)`.
pub fn resolve_by_ax_path(
    pid: i32,
    ax_path: &str,
    perform_actions: bool,
) -> Result<(bool, f64, f64), String> {
    with_ax_path(pid, ax_path, |element| unsafe {
        let pos = ax_position(element).unwrap_or_default();
        let size = ax_size(element).unwrap_or_default();
        let acted = if perform_actions {
            try_ax_actions(element).is_some()
        } else {
            false
        };
        (acted, pos.x + size.width / 2.0, pos.y + size.height / 2.0)
    })
    .map_err(|e| {
        if e.starts_with("axPath") {
            e
        } else {
            format!("element not found at axPath '{ax_path}': {e}")
        }
    })
}

/// `cu perform --ax-path X` — fire the named AX action on the matched element.
pub fn ax_perform_by_path(pid: i32, ax_path: &str, action: &str) -> Result<(), CuError> {
    let ok = with_ax_path(pid, ax_path, |element| unsafe {
        try_action(element, action)
    })
    .map_err(|e| {
        CuError::msg(e).with_hint("axPath did not resolve — re-snapshot to refresh paths")
    })?;
    if ok {
        Ok(())
    } else {
        Err(format!("AX action '{action}' failed or not supported by element").into())
    }
}

/// `cu set-value --ax-path X` — write `value` to the matched element's AXValue.
pub fn ax_set_value_by_path(pid: i32, ax_path: &str, value: &str) -> Result<(), CuError> {
    let ok = with_ax_path(pid, ax_path, |element| unsafe {
        match cfstr(value) {
            None => false,
            Some(value_cf) => {
                let result = try_set_value(element, "AXValue", value_cf);
                CFRelease(value_cf);
                result
            }
        }
    })
    .map_err(|e| {
        CuError::msg(e).with_hint("axPath did not resolve — re-snapshot to refresh paths")
    })?;
    if ok {
        Ok(())
    } else {
        Err(CuError::msg("AXValue write was rejected by the element")
            .with_hint("the element may be read-only or not a value-bearing role"))
    }
}

/// Find element by ref — coordinate lookup only, no AX actions triggered.
pub fn ax_find_element(pid: i32, ref_id: usize, _limit: usize) -> Result<(bool, f64, f64), String> {
    resolve_ref(pid, ref_id, false)
}

/// Find element by ref and write `value` to its AXValue attribute.
/// This is the fastest path to populate text fields — no focus, no IME,
/// no clipboard. Returns Ok(()) when the write succeeded; Err with a hint
/// when the element is missing or refused the write.
pub fn ax_set_value(pid: i32, ref_id: usize, _limit: usize, value: &str) -> Result<(), CuError> {
    unsafe {
        let value_cf =
            cfstr(value).ok_or_else(|| CuError::msg("failed to create CFString for value"))?;

        let app_el = create_app_element(pid);
        if app_el.is_null() {
            CFRelease(value_cf);
            return Err(CuError::msg("failed to create AX element for application"));
        }

        let window_el =
            ax_attr(app_el, "AXFocusedWindow").or_else(|| ax_attr(app_el, "AXMainWindow"));
        if let Some(w) = window_el {
            set_element_timeout(w);
        }
        let walk_root = window_el.unwrap_or(app_el);

        let (result, counter) = resolve_ref_with(walk_root, ref_id, |element, _, _| {
            try_set_value(element, "AXValue", value_cf)
        });

        if let Some(w) = window_el {
            CFRelease(w);
        }
        CFRelease(app_el);
        CFRelease(value_cf);

        match result {
            Some(true) => Ok(()),
            Some(false) => Err(CuError::msg(format!(
                "element [{ref_id}] rejected AXValue write"
            ))
            .with_hint("Element exists but is not settable. Common reasons: the control is disabled, the value is computed, or the field requires keyboard input.")
            .with_next(format!("cu click {ref_id} --app <name>"))
            .with_next(format!("cu type \"{value}\" --app <name>"))),
            None => Err(CuError::msg(format!(
                "element [{ref_id}] not found in AX tree (scanned {counter} elements)"
            ))
            .with_hint(
                "Refs are ephemeral and refresh on every action. Re-snapshot to find the current ref.",
            )
            .with_next("cu snapshot <app>")),
        }
    }
}

/// Find element by ref and perform a named AX action (e.g. AXShowMenu,
/// AXIncrement, AXScrollToVisible). On failure, the hint includes the list
/// of actions the element actually supports — feed that back to the agent.
pub fn ax_perform(
    pid: i32,
    ref_id: usize,
    _limit: usize,
    action: &str,
) -> Result<Vec<String>, CuError> {
    unsafe {
        let app_el = create_app_element(pid);
        if app_el.is_null() {
            return Err(CuError::msg("failed to create AX element for application"));
        }

        let window_el =
            ax_attr(app_el, "AXFocusedWindow").or_else(|| ax_attr(app_el, "AXMainWindow"));
        if let Some(w) = window_el {
            set_element_timeout(w);
        }
        let walk_root = window_el.unwrap_or(app_el);

        let (result, counter) = resolve_ref_with(walk_root, ref_id, |element, _, _| {
            let success = try_action(element, action);
            let available = copy_action_names(element);
            (success, available)
        });

        if let Some(w) = window_el {
            CFRelease(w);
        }
        CFRelease(app_el);

        match result {
            Some((true, available)) => Ok(available),
            Some((false, available)) => {
                let mut err = CuError::msg(format!(
                    "element [{ref_id}] does not support {action}"
                ));
                if available.is_empty() {
                    err = err.with_hint(
                        "Element exposes no AX actions. It may be a static container — try clicking a child instead.",
                    );
                } else {
                    err = err
                        .with_hint(format!("Available actions: {}", available.join(", ")))
                        .with_diagnostics(serde_json::json!({
                            "available_actions": available,
                        }));
                    for a in &available {
                        err = err.with_next(format!("cu perform {ref_id} {a} --app <name>"));
                    }
                }
                Err(err)
            }
            None => Err(CuError::msg(format!(
                "element [{ref_id}] not found in AX tree (scanned {counter} elements)"
            ))
            .with_hint(
                "Refs are ephemeral and refresh on every action. Re-snapshot to find the current ref.",
            )
            .with_next("cu snapshot <app>")),
        }
    }
}

/// Diagnostic info for a single ref — used by `cu why` (B7) to explain
/// why a click might have failed. Walks the tree to find the element,
/// then returns its supported AX actions + AXEnabled flag.
pub struct RefInspection {
    pub actions: Vec<String>,
    pub enabled: Option<bool>,
    pub focused: Option<bool>,
    pub subrole: Option<String>,
}

/// Walk the tree to find a ref and return its supported actions + enabled state.
/// Returns None if the ref is out of range. Used by `cu why`.
pub fn inspect_ref(pid: i32, ref_id: usize) -> Option<RefInspection> {
    unsafe {
        let app_el = create_app_element(pid);
        if app_el.is_null() {
            return None;
        }
        let window_el =
            ax_attr(app_el, "AXFocusedWindow").or_else(|| ax_attr(app_el, "AXMainWindow"));
        if let Some(w) = window_el {
            set_element_timeout(w);
        }
        let walk_root = window_el.unwrap_or(app_el);
        let (result, _) = resolve_ref_with(walk_root, ref_id, |element, _, _| {
            let actions = copy_action_names(element);
            let enabled = ax_bool(element, "AXEnabled");
            let focused = ax_bool(element, "AXFocused");
            let subrole = ax_string(element, "AXSubrole").filter(|s| !s.is_empty());
            RefInspection {
                actions,
                enabled,
                focused,
                subrole,
            }
        });
        if let Some(w) = window_el {
            CFRelease(w);
        }
        CFRelease(app_el);
        result
    }
}

/// Resolve the currently focused UI element of the app and summarize it.
/// `elements` is the snapshot's element list — used to look up the matching
/// ref by (role, x, y). Match on (x,y) is enough in practice — two elements
/// with the same role and identical screen position would be a UI bug.
unsafe fn detect_focused(app_el: CFTypeRef, elements: &[Element]) -> Option<FocusedSummary> {
    let fel = ax_attr(app_el, "AXFocusedUIElement")?;
    let role = ax_string(fel, "AXRole").unwrap_or_default();
    if role.is_empty() {
        CFRelease(fel);
        return None;
    }
    let title = ax_string(fel, "AXTitle")
        .or_else(|| ax_string(fel, "AXDescription"))
        .filter(|s| !s.is_empty());
    let value = ax_string(fel, "AXValue").filter(|s| !s.is_empty());
    let pos = ax_position(fel);
    CFRelease(fel);

    let normalized = normalize_role(&role);
    let matched = pos.and_then(|p| {
        let (px, py) = (p.x.round(), p.y.round());
        elements
            .iter()
            .find(|e| e.role == normalized && (e.x - px).abs() < 1.0 && (e.y - py).abs() < 1.0)
    });

    Some(FocusedSummary {
        ref_id: matched.map(|element| element.ref_id),
        role: normalized,
        title,
        value,
        ax_path: matched.and_then(|element| element.ax_path.clone()),
    })
}

/// Detect whether a modal (AXSheet) or system dialog is blocking the window.
/// Checks the window itself first, then its direct children.
unsafe fn detect_modal(window_el: CFTypeRef) -> Option<ModalSummary> {
    let win_role = ax_string(window_el, "AXRole").unwrap_or_default();
    let win_subrole = ax_string(window_el, "AXSubrole").unwrap_or_default();
    if win_role == "AXSheet"
        || win_subrole == "AXSystemDialog"
        || win_subrole == "AXSheet"
        || win_subrole == "AXDialog"
    {
        return Some(ModalSummary {
            role: win_role,
            subrole: (!win_subrole.is_empty()).then_some(win_subrole),
            title: ax_string(window_el, "AXTitle").filter(|s| !s.is_empty()),
        });
    }

    // Look one level down — modal sheets typically attach as direct children.
    let children = ax_attr(window_el, "AXChildren")?;
    let mut found = None;
    if CFGetTypeID(children) == CFArrayGetTypeID() {
        let count = CFArrayGetCount(children);
        for i in 0..count {
            let child = CFArrayGetValueAtIndex(children, i);
            if child.is_null() {
                continue;
            }
            let crole = ax_string(child, "AXRole").unwrap_or_default();
            if crole == "AXSheet" {
                let csubrole = ax_string(child, "AXSubrole").unwrap_or_default();
                let ctitle = ax_string(child, "AXTitle").filter(|s| !s.is_empty());
                found = Some(ModalSummary {
                    role: crole,
                    subrole: (!csubrole.is_empty()).then_some(csubrole),
                    title: ctitle,
                });
                break;
            }
        }
    }
    CFRelease(children);
    found
}

/// Returns the number of standard windows reported by the app element.
/// Uses `AXWindows` (all windows) attribute. Returns 0 on failure.
pub fn window_count(pid: i32) -> usize {
    unsafe {
        let app_el = create_app_element(pid);
        if app_el.is_null() {
            return 0;
        }
        let count = if let Some(arr) = ax_attr(app_el, "AXWindows") {
            let n = if CFGetTypeID(arr) == CFArrayGetTypeID() {
                CFArrayGetCount(arr) as usize
            } else {
                0
            };
            CFRelease(arr);
            n
        } else {
            0
        };
        CFRelease(app_el);
        count
    }
}

/// Get the frontmost window bounds (x, y, width, height) for an app.
#[allow(dead_code)]
pub fn window_bounds(pid: i32) -> Option<(f64, f64, f64, f64)> {
    unsafe {
        let app_el = create_app_element(pid);
        if app_el.is_null() {
            return None;
        }

        let window = ax_attr(app_el, "AXFocusedWindow").or_else(|| ax_attr(app_el, "AXMainWindow"));
        if let Some(w) = window {
            set_element_timeout(w);
        }

        let result = window.and_then(|w| {
            let pos = ax_position(w)?;
            let size = ax_size(w)?;
            Some((pos.x, pos.y, size.width, size.height))
        });

        if let Some(w) = window {
            CFRelease(w);
        }
        CFRelease(app_el);
        result
    }
}

/// Take an accessibility snapshot of the app identified by `pid`.
pub fn snapshot(pid: i32, app_name: &str, limit: usize) -> SnapshotResult {
    unsafe {
        let app_el = create_app_element(pid);
        if app_el.is_null() {
            return SnapshotResult {
                ok: false,
                app: app_name.to_string(),
                window: String::new(),
                window_frame: None,
                elements: vec![],
                limit,
                truncated: false,
                truncation_hint: None,
                depth_limited: false,
                focused: None,
                modal: None,
                error: Some("failed to create AX element for application".into()),
            };
        }

        // Probe: try to read AXRole to check accessibility permission
        let (probe_err, probe_val) = ax_attr_with_err(app_el, "AXRole");
        if !probe_val.is_null() {
            CFRelease(probe_val);
        }
        if probe_err != AX_OK {
            let msg = match probe_err {
                -25211 => "accessibility permission denied — open System Settings → Privacy & Security → Accessibility and grant access to this terminal".into(),
                -25204 => "cannot communicate with the application — it may not support accessibility".into(),
                -25205 => "accessibility not enabled for this application".into(),
                code => format!("accessibility error (code {code})"),
            };
            CFRelease(app_el);
            return SnapshotResult {
                ok: false,
                app: app_name.to_string(),
                window: String::new(),
                window_frame: None,
                elements: vec![],
                limit,
                truncated: false,
                truncation_hint: None,
                depth_limited: false,
                focused: None,
                modal: None,
                error: Some(msg),
            };
        }

        // Resolve the target window
        let window_el =
            ax_attr(app_el, "AXFocusedWindow").or_else(|| ax_attr(app_el, "AXMainWindow"));
        if let Some(w) = window_el {
            set_element_timeout(w);
        }

        let window_title = window_el
            .and_then(|w| ax_string(w, "AXTitle"))
            .unwrap_or_default();

        // Extract window frame (position + size) for navigation context
        let window_frame = window_el.and_then(|w| {
            let pos = ax_position(w)?;
            let size = ax_size(w)?;
            Some(WindowFrame {
                x: pos.x,
                y: pos.y,
                width: size.width,
                height: size.height,
            })
        });

        let walk_root = window_el.unwrap_or(app_el);

        // Walk the element tree with batch attribute reading
        let batch_keys = create_batch_keys();
        if batch_keys.is_null() {
            if let Some(w) = window_el {
                CFRelease(w);
            }
            CFRelease(app_el);
            return SnapshotResult {
                ok: false,
                app: app_name.to_string(),
                window: window_title,
                window_frame: None,
                elements: vec![],
                limit,
                truncated: false,
                truncation_hint: None,
                depth_limited: false,
                focused: None,
                modal: None,
                error: Some("failed to create AX batch attribute keys".into()),
            };
        }
        let mut elements = Vec::new();
        let mut counter = 0usize;
        let mut depth_limited = false;
        // Compute the root's own path segment so descendants get full paths.
        let root_role = ax_string(walk_root, "AXRole").unwrap_or_default();
        let root_title = ax_string(walk_root, "AXTitle")
            .or_else(|| ax_string(walk_root, "AXDescription"))
            .filter(|s| !s.is_empty());
        let root_segment = build_path_segment(&root_role, root_title.as_deref());
        let mut reader = |element| read_batch_ref_node(element, batch_keys);
        let mut visitor = |element: CFTypeRef,
                           node: &RefNodeRead,
                           ref_id: Option<usize>,
                           self_path: &str|
         -> RefVisit<()> {
            if elements.len() >= limit {
                return RefVisit::Break(());
            }
            if let Some(ref_id) = ref_id {
                let pos = node.position.unwrap_or_default();
                elements.push(Element {
                    ref_id,
                    role: normalize_role(node.role.as_deref().unwrap_or_default()),
                    title: node.title.clone(),
                    value: node.value.clone(),
                    x: pos.x.round(),
                    y: pos.y.round(),
                    width: node.size.width.round(),
                    height: node.size.height.round(),
                    ax_path: Some(self_path.to_string()),
                });
            }
            if elements.len() >= limit {
                RefVisit::Break(())
            } else {
                // `element` is intentionally consumed by the visitor type so
                // the callback remains compatible with action consumers; the
                // snapshot projection itself only needs the decoded fields.
                let _ = element;
                RefVisit::Continue
            }
        };
        let _ = traverse_ref_tree(
            walk_root,
            &mut counter,
            0,
            true,
            &root_segment,
            "",
            &mut depth_limited,
            &mut reader,
            &mut visitor,
        );
        CFRelease(batch_keys);

        let truncated = elements.len() >= limit;
        let truncation_hint = if truncated {
            Some(format!(
                "snapshot stopped at {limit} elements — there are MORE elements past this point. \
                 Re-run with --limit {} (or higher) if the element you need isn't in this batch.",
                limit * 2
            ))
        } else {
            None
        };

        // A4: surface the currently focused UI element so the agent can skip
        //     a redundant click on a field that's already focused.
        let focused = detect_focused(app_el, &elements);

        // A6: surface a modal/sheet warning so the agent dismisses it first
        //     instead of fruitlessly clicking on the (now-blocked) main window.
        let modal = window_el.and_then(|w| detect_modal(w));

        // Clean up
        if let Some(w) = window_el {
            CFRelease(w);
        }
        CFRelease(app_el);

        SnapshotResult {
            ok: true,
            app: app_name.to_string(),
            window: window_title,
            window_frame,
            focused,
            modal,
            elements,
            limit,
            truncated,
            truncation_hint,
            depth_limited,
            error: None,
        }
    }
}
