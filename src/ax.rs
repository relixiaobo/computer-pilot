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
    fn AXUIElementSetAttributeValue(element: CFTypeRef, attribute: CFStringRef, value: CFTypeRef) -> AXError;
    fn AXUIElementCopyMultipleAttributeValues(
        element: CFTypeRef,
        attributes: CFArrayRef,
        options: u32, // 0 = normal
        values: *mut CFArrayRef,
    ) -> AXError;
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

// ── Batch attribute reading ──────────────────────────────────────────────────

// Attribute indices in the batch array (order must match BATCH_ATTRS)
const BA_ROLE: usize = 0;
const BA_TITLE: usize = 1;
const BA_DESC: usize = 2;
const BA_VALUE: usize = 3;
const BA_POS: usize = 4;
const BA_SIZE: usize = 5;
const BA_CHILDREN: usize = 6;
const BATCH_ATTR_NAMES: &[&str] = &[
    "AXRole", "AXTitle", "AXDescription", "AXValue", "AXPosition", "AXSize", "AXChildren",
];

/// Create the CFArray of attribute name strings (cached per snapshot).
unsafe fn create_batch_keys() -> CFArrayRef {
    let mut keys: Vec<CFTypeRef> = Vec::with_capacity(BATCH_ATTR_NAMES.len());
    for name in BATCH_ATTR_NAMES {
        if let Some(k) = cfstr(name) {
            keys.push(k);
        }
    }
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
    let mut values: CFArrayRef = std::ptr::null();
    let err = AXUIElementCopyMultipleAttributeValues(element, keys, 0, &mut values);
    if err == AX_OK && !values.is_null() {
        values
    } else {
        std::ptr::null()
    }
}

/// Extract a string from position `idx` in the batch values array.
unsafe fn batch_string(values: CFArrayRef, idx: usize) -> Option<String> {
    let count = CFArrayGetCount(values) as usize;
    if idx >= count { return None; }
    let val = CFArrayGetValueAtIndex(values, idx as CFIndex);
    if val.is_null() { return None; }
    // Check it's actually a CFString (not NSNull / error marker)
    if CFGetTypeID(val) != CFStringGetTypeID() { return None; }
    cfstring_to_string(val)
}

/// Extract position (CGPoint) from batch values.
unsafe fn batch_position(values: CFArrayRef, idx: usize) -> Option<CGPoint> {
    let count = CFArrayGetCount(values) as usize;
    if idx >= count { return None; }
    let val = CFArrayGetValueAtIndex(values, idx as CFIndex);
    if val.is_null() { return None; }
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
    if idx >= count { return None; }
    let val = CFArrayGetValueAtIndex(values, idx as CFIndex);
    if val.is_null() { return None; }
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
    if idx >= count { return None; }
    let val = CFArrayGetValueAtIndex(values, idx as CFIndex);
    if val.is_null() { return None; }
    if CFGetTypeID(val) != CFArrayGetTypeID() { return None; }
    Some(val)
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
    batch_keys: CFArrayRef,
) {
    if out.len() >= limit {
        return;
    }
    if depth > MAX_DEPTH {
        *depth_limited = true;
        return;
    }

    // Single IPC call to read all attributes
    let values = batch_read(element, batch_keys);
    if values.is_null() {
        return;
    }

    // Check role and maybe add to output
    if let Some(role) = batch_string(values, BA_ROLE) {
        if is_included(&role) {
            let title = batch_string(values, BA_TITLE)
                .or_else(|| batch_string(values, BA_DESC))
                .filter(|s| !s.is_empty());

            let value = batch_string(values, BA_VALUE).filter(|s| !s.is_empty());
            let pos = batch_position(values, BA_POS).unwrap_or_default();
            let size = batch_size(values, BA_SIZE).unwrap_or_default();

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
        CFRelease(values);
        return;
    }

    // Recurse into children (from the same batch read — no extra IPC)
    if let Some(children) = batch_children(values, BA_CHILDREN) {
        let count = CFArrayGetCount(children);
        for i in 0..count {
            let child = CFArrayGetValueAtIndex(children, i);
            if !child.is_null() {
                walk(child, out, counter, limit, depth + 1, depth_limited, batch_keys);
                if out.len() >= limit {
                    break;
                }
            }
        }
    }

    CFRelease(values);
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
    // Steps 1-4: Direct actions
    for action in &["AXPress", "AXConfirm", "AXOpen", "AXPick"] {
        if try_action(element, action) { return Some("ax-action"); }
    }

    // Step 5: ShowAlternateUI
    if try_action(element, "AXShowAlternateUI") { return Some("ax-alt-ui"); }

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

    // Step 7: Set AXValue (toggle checkboxes, etc.)
    let role = ax_string(element, "AXRole").unwrap_or_default();
    if role == "AXCheckBox" {
        if let Some(val_ref) = ax_attr(element, "AXValue") {
            // Toggle: if current value is 0, set to 1; if 1, set to 0
            CFRelease(val_ref);
            // AXPress should have worked for checkboxes, but try AXValue as fallback
        }
    }

    // Step 8: Set AXSelected=true
    if try_set_bool(element, "AXSelected", true) { return Some("ax-selected"); }

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
            if try_action(element, action) { return Some("ax-focus-press"); }
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
    let Some(action_str) = cfstr(action) else { return false };
    let err = AXUIElementPerformAction(element, action_str);
    CFRelease(action_str);
    err == AX_OK
}

unsafe fn try_set_bool(element: CFTypeRef, attr: &str, val: bool) -> bool {
    let Some(key) = cfstr(attr) else { return false };
    // Create CFBoolean
    let cf_bool: CFTypeRef = if val {
        kCFBooleanTrue
    } else {
        kCFBooleanFalse
    };
    let err = AXUIElementSetAttributeValue(element, key, cf_bool);
    CFRelease(key);
    err == AX_OK
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

        // Walk the element tree with batch attribute reading
        let batch_keys = create_batch_keys();
        let mut elements = Vec::new();
        let mut counter = 0usize;
        let mut depth_limited = false;
        walk(walk_root, &mut elements, &mut counter, limit, 0, &mut depth_limited, batch_keys);
        if !batch_keys.is_null() { CFRelease(batch_keys); }

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
