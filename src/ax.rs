//! macOS Accessibility (AX) snapshot — walks the UI element tree of a target application.
#![allow(unsafe_op_in_unsafe_fn)]

use serde::Serialize;
use std::ffi::{c_char, c_long, c_void, CStr};

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
    fn AXUIElementPerformAction(element: CFTypeRef, action: CFStringRef) -> AXError;
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
    pub elements: Vec<Element>,
    pub limit: usize,
    pub truncated: bool,
    pub depth_limited: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct Element {
    #[serde(rename = "ref")]
    pub ref_id: usize,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
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
    let ok = AXValueGetValue(value, AX_VALUE_CG_POINT, &mut point as *mut _ as *mut c_void);
    CFRelease(value);
    if ok != 0 {
        Some(point)
    } else {
        None
    }
}

/// Get the size (AXSize → CGSize).
unsafe fn ax_size(element: CFTypeRef) -> Option<CGSize> {
    let value = ax_attr(element, "AXSize")?;
    let mut size = CGSize::default();
    let ok = AXValueGetValue(value, AX_VALUE_CG_SIZE, &mut size as *mut _ as *mut c_void);
    CFRelease(value);
    if ok != 0 {
        Some(size)
    } else {
        None
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

// ── Tree walk ───────────────────────────────────────────────────────────────

const MAX_DEPTH: usize = 30;

unsafe fn walk(
    element: CFTypeRef,
    out: &mut Vec<Element>,
    counter: &mut usize,
    limit: usize,
    depth: usize,
    depth_limited: &mut bool,
) {
    if out.len() >= limit {
        return;
    }
    if depth > MAX_DEPTH {
        *depth_limited = true;
        return;
    }

    // Inspect this element
    if let Some(role) = ax_string(element, "AXRole") {
        if is_included(&role) {
            let title = ax_string(element, "AXTitle")
                .or_else(|| ax_string(element, "AXDescription"))
                .filter(|s| !s.is_empty());

            let value = ax_string(element, "AXValue").filter(|s| !s.is_empty());

            let pos = ax_position(element).unwrap_or_default();
            let size = ax_size(element).unwrap_or_default();

            // Skip invisible (zero-area) elements
            if size.width > 0.0 || size.height > 0.0 {
                *counter += 1;
                out.push(Element {
                    ref_id: *counter,
                    role: normalize_role(&role),
                    title,
                    value,
                    x: pos.x.round(),
                    y: pos.y.round(),
                    width: size.width.round(),
                    height: size.height.round(),
                });
            }
        }
    }

    if out.len() >= limit {
        return;
    }

    // Recurse into children (the array stays alive while we iterate)
    if let Some(children) = ax_attr(element, "AXChildren") {
        if CFGetTypeID(children) == CFArrayGetTypeID() {
            let count = CFArrayGetCount(children);
            for i in 0..count {
                let child = CFArrayGetValueAtIndex(children, i);
                if !child.is_null() {
                    walk(child, out, counter, limit, depth + 1, depth_limited);
                    if out.len() >= limit {
                        break;
                    }
                }
            }
        }
        CFRelease(children);
    }
}

// ── AX action helpers ───────────────────────────────────────────────────────

const AX_ACTIONS: &[&str] = &["AXPress", "AXConfirm", "AXOpen", "AXPick"];

/// Try AX actions on an element. Returns Ok(action_name) on success.
unsafe fn try_ax_actions(element: CFTypeRef) -> Option<&'static str> {
    for action in AX_ACTIONS {
        let Some(action_str) = cfstr(action) else { continue };
        let err = AXUIElementPerformAction(element, action_str);
        CFRelease(action_str);
        if err == AX_OK {
            return Some(action);
        }
    }
    None
}

/// Walk tree to find element by ref counter, then try AX actions on it.
/// Returns (found, action_performed, x_center, y_center).
unsafe fn find_and_act(
    element: CFTypeRef,
    target_ref: usize,
    counter: &mut usize,
    depth: usize,
) -> Option<(bool, f64, f64)> {
    if depth > MAX_DEPTH {
        return None;
    }

    // Check this element's role
    if let Some(role) = ax_string(element, "AXRole") {
        if is_included(&role) {
            let size = ax_size(element).unwrap_or_default();
            if size.width > 0.0 || size.height > 0.0 {
                *counter += 1;
                if *counter == target_ref {
                    let pos = ax_position(element).unwrap_or_default();
                    let cx = pos.x + size.width / 2.0;
                    let cy = pos.y + size.height / 2.0;

                    // Try AX actions first
                    let acted = try_ax_actions(element).is_some();
                    return Some((acted, cx, cy));
                }
            }
        }
    }

    // Recurse into children
    if let Some(children) = ax_attr(element, "AXChildren") {
        if CFGetTypeID(children) == CFArrayGetTypeID() {
            let count = CFArrayGetCount(children);
            for i in 0..count {
                let child = CFArrayGetValueAtIndex(children, i);
                if !child.is_null() {
                    if let Some(result) = find_and_act(child, target_ref, counter, depth + 1) {
                        CFRelease(children);
                        return Some(result);
                    }
                }
            }
        }
        CFRelease(children);
    }
    None
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Click an element by ref: try AX actions first, return center coords for CGEvent fallback.
/// Returns Ok((ax_action_succeeded, center_x, center_y)) or Err.
pub fn ax_click(pid: i32, ref_id: usize, _limit: usize) -> Result<(bool, f64, f64), String> {
    unsafe {
        let app_el = AXUIElementCreateApplication(pid);
        if app_el.is_null() {
            return Err("failed to create AX element for application".into());
        }

        let window_el = ax_attr(app_el, "AXFocusedWindow")
            .or_else(|| ax_attr(app_el, "AXMainWindow"));

        let walk_root = window_el.unwrap_or(app_el);

        let mut counter = 0usize;
        let result = find_and_act(walk_root, ref_id, &mut counter, 0);

        if let Some(w) = window_el {
            CFRelease(w);
        }
        CFRelease(app_el);

        match result {
            Some((acted, cx, cy)) => Ok((acted, cx, cy)),
            None => Err(format!("element [{}] not found in AX tree (scanned {} elements)", ref_id, counter)),
        }
    }
}

/// Get the frontmost window bounds (x, y, width, height) for an app.
#[allow(dead_code)]
pub fn window_bounds(pid: i32) -> Option<(f64, f64, f64, f64)> {
    unsafe {
        let app_el = AXUIElementCreateApplication(pid);
        if app_el.is_null() {
            return None;
        }

        let window = ax_attr(app_el, "AXFocusedWindow")
            .or_else(|| ax_attr(app_el, "AXMainWindow"));

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
        let app_el = AXUIElementCreateApplication(pid);
        if app_el.is_null() {
            return SnapshotResult {
                ok: false,
                app: app_name.to_string(),
                window: String::new(),
                elements: vec![],
                limit,
                truncated: false,
                depth_limited: false,
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
                elements: vec![],
                limit,
                truncated: false,
                depth_limited: false,
                error: Some(msg),
            };
        }

        // Resolve the target window
        let window_el = ax_attr(app_el, "AXFocusedWindow")
            .or_else(|| ax_attr(app_el, "AXMainWindow"));

        let window_title = window_el
            .and_then(|w| ax_string(w, "AXTitle"))
            .unwrap_or_default();

        let walk_root = window_el.unwrap_or(app_el);

        // Walk the element tree
        let mut elements = Vec::new();
        let mut counter = 0usize;
        let mut depth_limited = false;
        walk(walk_root, &mut elements, &mut counter, limit, 0, &mut depth_limited);

        let truncated = elements.len() >= limit;

        // Clean up
        if let Some(w) = window_el {
            CFRelease(w);
        }
        CFRelease(app_el);

        SnapshotResult {
            ok: true,
            app: app_name.to_string(),
            window: window_title,
            elements,
            limit,
            truncated,
            depth_limited,
            error: None,
        }
    }
}
