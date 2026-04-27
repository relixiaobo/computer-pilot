//! macOS Accessibility (AX) snapshot — walks the UI element tree of a target application.
#![allow(unsafe_op_in_unsafe_fn)]

use crate::error::CuError;
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, c_char, c_long, c_void};

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
    pub depth_limited: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct FocusedSummary {
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<usize>,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
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
    "AXRole",
    "AXTitle",
    "AXDescription",
    "AXValue",
    "AXPosition",
    "AXSize",
    "AXChildren",
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

// ── Tree walk ───────────────────────────────────────────────────────────────

const MAX_DEPTH: usize = 30;

/// Recursive AX tree walker. `my_segment` is the path segment that identifies
/// this element relative to its parent (already includes any `:N` sibling
/// disambiguator). `parent_path` is the slash-joined path from the root to
/// the parent. `self_path = parent_path + "/" + my_segment`.
#[allow(clippy::too_many_arguments)]
unsafe fn walk(
    element: CFTypeRef,
    out: &mut Vec<Element>,
    counter: &mut usize,
    limit: usize,
    depth: usize,
    depth_limited: &mut bool,
    batch_keys: CFArrayRef,
    my_segment: &str,
    parent_path: &str,
) {
    if out.len() >= limit {
        return;
    }
    if depth > MAX_DEPTH {
        *depth_limited = true;
        return;
    }

    let self_path = if parent_path.is_empty() {
        my_segment.to_string()
    } else {
        format!("{parent_path}/{my_segment}")
    };

    // Single IPC call to read all attributes. On failure, fall back to
    // individual reads for AXChildren so we don't lose entire subtrees.
    let values = batch_read(element, batch_keys);
    if values.is_null() {
        if let Some(children) = ax_attr(element, "AXChildren") {
            if CFGetTypeID(children) == CFArrayGetTypeID() {
                let count = CFArrayGetCount(children);
                let mut seen: std::collections::HashMap<String, usize> =
                    std::collections::HashMap::new();
                for i in 0..count {
                    let child = CFArrayGetValueAtIndex(children, i);
                    if child.is_null() {
                        continue;
                    }
                    let child_segment = compute_child_segment(child, &mut seen);
                    walk(
                        child,
                        out,
                        counter,
                        limit,
                        depth + 1,
                        depth_limited,
                        batch_keys,
                        &child_segment,
                        &self_path,
                    );
                    if out.len() >= limit {
                        break;
                    }
                }
            }
            CFRelease(children);
        }
        return;
    }

    // Check role and maybe add to output
    if let Some(role) = batch_string(values, BA_ROLE)
        && is_included(&role)
    {
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
                ax_path: Some(self_path.clone()),
            });
        }
    }

    if out.len() >= limit {
        CFRelease(values);
        return;
    }

    // Recurse into children (from the same batch read — no extra IPC).
    // Track sibling segments to assign `:N` disambiguators when multiple
    // children produce the same `Role[Title]` segment.
    if let Some(children) = batch_children(values, BA_CHILDREN) {
        let count = CFArrayGetCount(children);
        let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for i in 0..count {
            let child = CFArrayGetValueAtIndex(children, i);
            if child.is_null() {
                continue;
            }
            let child_segment = compute_child_segment(child, &mut seen);
            walk(
                child,
                out,
                counter,
                limit,
                depth + 1,
                depth_limited,
                batch_keys,
                &child_segment,
                &self_path,
            );
            if out.len() >= limit {
                break;
            }
        }
    }

    CFRelease(values);
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

    // Step 7: Toggle AXValue for checkboxes/switches
    let role = ax_string(element, "AXRole").unwrap_or_default();
    if role == "AXCheckBox" || role == "AXSwitch" {
        // Try setting to 1 (checked), then 0 (unchecked) — one will be the toggle
        if try_set_value(element, "AXValue", kCFBooleanTrue)
            || try_set_value(element, "AXValue", kCFBooleanFalse)
        {
            return Some("ax-toggle");
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

/// Walk tree to find element by ref and perform the named AX action.
/// Returns Some((success, available_actions)) on hit, None on miss.
/// `available_actions` is always populated when the element is found, so
/// callers can include it in error hints regardless of success.
unsafe fn find_and_perform_action(
    element: CFTypeRef,
    target_ref: usize,
    counter: &mut usize,
    depth: usize,
    action: &str,
) -> Option<(bool, Vec<String>)> {
    if depth > MAX_DEPTH {
        return None;
    }

    if let Some(role) = ax_string(element, "AXRole")
        && is_included(&role)
    {
        let size = ax_size(element).unwrap_or_default();
        if size.width > 0.0 || size.height > 0.0 {
            *counter += 1;
            if *counter == target_ref {
                let success = try_action(element, action);
                let available = copy_action_names(element);
                return Some((success, available));
            }
        }
    }

    if let Some(children) = ax_attr(element, "AXChildren") {
        if CFGetTypeID(children) == CFArrayGetTypeID() {
            let count = CFArrayGetCount(children);
            for i in 0..count {
                let child = CFArrayGetValueAtIndex(children, i);
                if !child.is_null()
                    && let Some(result) =
                        find_and_perform_action(child, target_ref, counter, depth + 1, action)
                {
                    CFRelease(children);
                    return Some(result);
                }
            }
        }
        CFRelease(children);
    }
    None
}

/// Walk tree to find element by ref and write `value_cf` to its AXValue.
/// Returns Some(true) on successful write, Some(false) if the element rejected
/// the write, None if the ref was not found.
unsafe fn find_and_set_value(
    element: CFTypeRef,
    target_ref: usize,
    counter: &mut usize,
    depth: usize,
    value_cf: CFTypeRef,
) -> Option<bool> {
    if depth > MAX_DEPTH {
        return None;
    }

    if let Some(role) = ax_string(element, "AXRole")
        && is_included(&role)
    {
        let size = ax_size(element).unwrap_or_default();
        if size.width > 0.0 || size.height > 0.0 {
            *counter += 1;
            if *counter == target_ref {
                return Some(try_set_value(element, "AXValue", value_cf));
            }
        }
    }

    if let Some(children) = ax_attr(element, "AXChildren") {
        if CFGetTypeID(children) == CFArrayGetTypeID() {
            let count = CFArrayGetCount(children);
            for i in 0..count {
                let child = CFArrayGetValueAtIndex(children, i);
                if !child.is_null()
                    && let Some(result) =
                        find_and_set_value(child, target_ref, counter, depth + 1, value_cf)
                {
                    CFRelease(children);
                    return Some(result);
                }
            }
        }
        CFRelease(children);
    }
    None
}

/// Walk tree to find element by ref. If `perform_actions` is true, tries AX actions.
/// Returns (action_performed, x_center, y_center).
unsafe fn find_element_by_ref(
    element: CFTypeRef,
    target_ref: usize,
    counter: &mut usize,
    depth: usize,
    perform_actions: bool,
) -> Option<(bool, f64, f64)> {
    if depth > MAX_DEPTH {
        return None;
    }

    if let Some(role) = ax_string(element, "AXRole")
        && is_included(&role)
    {
        let size = ax_size(element).unwrap_or_default();
        if size.width > 0.0 || size.height > 0.0 {
            *counter += 1;
            if *counter == target_ref {
                let pos = ax_position(element).unwrap_or_default();
                let cx = pos.x + size.width / 2.0;
                let cy = pos.y + size.height / 2.0;
                let acted = if perform_actions {
                    try_ax_actions(element).is_some()
                } else {
                    false
                };
                return Some((acted, cx, cy));
            }
        }
    }

    if let Some(children) = ax_attr(element, "AXChildren") {
        if CFGetTypeID(children) == CFArrayGetTypeID() {
            let count = CFArrayGetCount(children);
            for i in 0..count {
                let child = CFArrayGetValueAtIndex(children, i);
                if !child.is_null()
                    && let Some(result) =
                        find_element_by_ref(child, target_ref, counter, depth + 1, perform_actions)
                {
                    CFRelease(children);
                    return Some(result);
                }
            }
        }
        CFRelease(children);
    }
    None
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

        let mut counter = 0usize;
        let result = find_element_by_ref(walk_root, ref_id, &mut counter, 0, perform_actions);

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

        let mut counter = 0usize;
        let result = find_and_set_value(walk_root, ref_id, &mut counter, 0, value_cf);

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

        let mut counter = 0usize;
        let result = find_and_perform_action(walk_root, ref_id, &mut counter, 0, action);

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
    let ref_id = pos.and_then(|p| {
        let (px, py) = (p.x.round(), p.y.round());
        elements
            .iter()
            .find(|e| e.role == normalized && (e.x - px).abs() < 1.0 && (e.y - py).abs() < 1.0)
            .map(|e| e.ref_id)
    });

    Some(FocusedSummary {
        ref_id,
        role: normalized,
        title,
        value,
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

/// Raise (focus) the app's main window via direct AX, no AppleScript.
///
/// Sets `AXMain=true` and performs `AXRaise` on the main/focused window.
/// Returns `true` on success. This is the non-disruptive equivalent of
/// `tell application "X" to activate` — it brings the window forward without
/// going through the global activation path. (B6)
pub fn raise_window(pid: i32) -> bool {
    unsafe {
        let app_el = create_app_element(pid);
        if app_el.is_null() {
            return false;
        }
        let window = ax_attr(app_el, "AXMainWindow").or_else(|| ax_attr(app_el, "AXFocusedWindow"));
        let mut ok = false;
        if let Some(w) = window {
            set_element_timeout(w);
            // AXMain=true marks this window as the app's main; AXRaise brings it forward.
            let set_main = try_set_bool(w, "AXMain", true);
            let raised = try_action(w, "AXRaise");
            ok = set_main || raised;
            CFRelease(w);
        }
        CFRelease(app_el);
        ok
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
                x: pos.x as f64,
                y: pos.y as f64,
                width: size.width as f64,
                height: size.height as f64,
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
        walk(
            walk_root,
            &mut elements,
            &mut counter,
            limit,
            0,
            &mut depth_limited,
            batch_keys,
            &root_segment,
            "",
        );
        CFRelease(batch_keys);

        let truncated = elements.len() >= limit;

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
            depth_limited,
            error: None,
        }
    }
}
