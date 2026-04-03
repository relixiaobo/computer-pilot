//! Mouse operations via CGEvent — click, double-click, right-click, scroll, hover, drag.
//! All operations support modifier keys (shift, cmd, alt, ctrl) for shift+click, cmd+drag, etc.
#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::c_void;

type CFTypeRef = *const c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

const CG_EVENT_LEFT_MOUSE_DOWN: u32 = 1;
const CG_EVENT_LEFT_MOUSE_UP: u32 = 2;
const CG_EVENT_RIGHT_MOUSE_DOWN: u32 = 3;
const CG_EVENT_RIGHT_MOUSE_UP: u32 = 4;
const CG_EVENT_MOUSE_MOVED: u32 = 5;
const CG_EVENT_LEFT_MOUSE_DRAGGED: u32 = 6;
const CG_MOUSE_BUTTON_LEFT: u32 = 0;
const CG_MOUSE_BUTTON_RIGHT: u32 = 1;
const CG_HID_EVENT_TAP: u32 = 0;
const K_CG_MOUSE_EVENT_CLICK_STATE: u32 = 1;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreateMouseEvent(source: CFTypeRef, ty: u32, pos: CGPoint, btn: u32) -> CFTypeRef;
    fn CGEventCreateScrollWheelEvent(source: CFTypeRef, units: u32, count: u32, w1: i32) -> CFTypeRef;
    fn CGEventPost(tap: u32, event: CFTypeRef);
    fn CGEventSetIntegerValueField(event: CFTypeRef, field: u32, value: i64);
    fn CGEventSetFlags(event: CFTypeRef, flags: u64);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: CFTypeRef);
}

// ── Modifiers ───────────────────────────────────────────────────────────────

/// Modifier keys to hold during a mouse operation (shift+click, cmd+drag, etc.)
#[derive(Default, Clone, Copy)]
pub struct Modifiers {
    pub shift: bool,
    pub cmd: bool,
    pub alt: bool,
    pub ctrl: bool,
}

impl Modifiers {
    fn to_flags(self) -> u64 {
        let mut f = 0u64;
        if self.shift { f |= 0x0002_0000; }
        if self.ctrl  { f |= 0x0004_0000; }
        if self.alt   { f |= 0x0008_0000; }
        if self.cmd   { f |= 0x0010_0000; }
        f
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn pt(x: f64, y: f64) -> Result<CGPoint, String> {
    if !x.is_finite() || !y.is_finite() {
        return Err(format!("coordinates must be finite, got ({x}, {y})"));
    }
    Ok(CGPoint { x, y })
}

fn post(event: CFTypeRef, mods: Modifiers) -> Result<(), String> {
    if event.is_null() {
        return Err("failed to create CGEvent".into());
    }
    unsafe {
        let flags = mods.to_flags();
        if flags != 0 {
            CGEventSetFlags(event, flags);
        }
        CGEventPost(CG_HID_EVENT_TAP, event);
        CFRelease(event);
    }
    Ok(())
}

fn post_plain(event: CFTypeRef) -> Result<(), String> {
    post(event, Modifiers::default())
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Single left or right click, optionally with modifier keys.
pub fn click(x: f64, y: f64, right: bool, mods: Modifiers) -> Result<(), String> {
    let p = pt(x, y)?;
    let (dt, ut, btn) = if right {
        (CG_EVENT_RIGHT_MOUSE_DOWN, CG_EVENT_RIGHT_MOUSE_UP, CG_MOUSE_BUTTON_RIGHT)
    } else {
        (CG_EVENT_LEFT_MOUSE_DOWN, CG_EVENT_LEFT_MOUSE_UP, CG_MOUSE_BUTTON_LEFT)
    };
    unsafe {
        post(CGEventCreateMouseEvent(std::ptr::null(), dt, p, btn), mods)?;
        post(CGEventCreateMouseEvent(std::ptr::null(), ut, p, btn), mods)?;
    }
    Ok(())
}

/// Double-click at coordinates.
pub fn double_click(x: f64, y: f64, mods: Modifiers) -> Result<(), String> {
    let p = pt(x, y)?;
    unsafe {
        for count in [1i64, 2] {
            let down = CGEventCreateMouseEvent(std::ptr::null(), CG_EVENT_LEFT_MOUSE_DOWN, p, CG_MOUSE_BUTTON_LEFT);
            let up = CGEventCreateMouseEvent(std::ptr::null(), CG_EVENT_LEFT_MOUSE_UP, p, CG_MOUSE_BUTTON_LEFT);
            if !down.is_null() { CGEventSetIntegerValueField(down, K_CG_MOUSE_EVENT_CLICK_STATE, count); }
            if !up.is_null() { CGEventSetIntegerValueField(up, K_CG_MOUSE_EVENT_CLICK_STATE, count); }
            post(down, mods)?;
            post(up, mods)?;
        }
    }
    Ok(())
}

/// Scroll. `dy`: positive = up, negative = down. `dx`: positive = right, negative = left.
pub fn scroll(x: f64, y: f64, dy: i32, dx: i32) -> Result<(), String> {
    let p = pt(x, y)?;
    unsafe {
        post_plain(CGEventCreateMouseEvent(std::ptr::null(), CG_EVENT_MOUSE_MOVED, p, CG_MOUSE_BUTTON_LEFT))?;
        if dy != 0 {
            post_plain(CGEventCreateScrollWheelEvent(std::ptr::null(), 0, 1, dy))?;
        }
        if dx != 0 {
            let h = CGEventCreateScrollWheelEvent(std::ptr::null(), 0, 1, 0);
            if !h.is_null() {
                CGEventSetIntegerValueField(h, 96, dx as i64);
            }
            post_plain(h)?;
        }
    }
    Ok(())
}

/// Move mouse to coordinates (hover / trigger tooltips).
pub fn hover(x: f64, y: f64) -> Result<(), String> {
    let p = pt(x, y)?;
    unsafe { post_plain(CGEventCreateMouseEvent(std::ptr::null(), CG_EVENT_MOUSE_MOVED, p, CG_MOUSE_BUTTON_LEFT)) }
}

/// Drag from (x1,y1) to (x2,y2) with smooth interpolation.
/// Guarantees mouseUp even if intermediate steps fail.
pub fn drag(x1: f64, y1: f64, x2: f64, y2: f64, mods: Modifiers) -> Result<(), String> {
    let from = pt(x1, y1)?;
    let to = pt(x2, y2)?;

    unsafe {
        post(CGEventCreateMouseEvent(std::ptr::null(), CG_EVENT_LEFT_MOUSE_DOWN, from, CG_MOUSE_BUTTON_LEFT), mods)?;
        std::thread::sleep(std::time::Duration::from_millis(30));

        // Drag steps — if any fail, still release the mouse
        let mut drag_err = None;
        let steps = 10;
        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let mid = CGPoint {
                x: from.x + (to.x - from.x) * t,
                y: from.y + (to.y - from.y) * t,
            };
            if let Err(e) = post(CGEventCreateMouseEvent(std::ptr::null(), CG_EVENT_LEFT_MOUSE_DRAGGED, mid, CG_MOUSE_BUTTON_LEFT), mods) {
                drag_err = Some(e);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Always release mouse
        let _ = post(CGEventCreateMouseEvent(std::ptr::null(), CG_EVENT_LEFT_MOUSE_UP, to, CG_MOUSE_BUTTON_LEFT), mods);

        if let Some(e) = drag_err {
            return Err(e);
        }
    }
    Ok(())
}
