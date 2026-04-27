//! Multi-display awareness (D1).
//!
//! Lists active displays with their global-coordinate bounds, and finds which
//! display contains a given point. The agent can use this to: ① know the
//! geometry of all attached screens, ② verify a click target is on a real
//! screen, ③ resolve which screen owns a window's frame.

#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::c_void;

type CGDirectDisplayID = u32;
type CGError = i32;
const K_CG_ERROR_SUCCESS: CGError = 0;

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
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGGetActiveDisplayList(
        max_displays: u32,
        active_displays: *mut CGDirectDisplayID,
        display_count: *mut u32,
    ) -> CGError;
    fn CGDisplayBounds(display: CGDirectDisplayID) -> CGRect;
    fn CGMainDisplayID() -> CGDirectDisplayID;
}

#[derive(serde::Serialize, Clone)]
pub struct DisplayInfo {
    pub id: u32,
    pub main: bool,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Enumerate active displays in global coordinates. Returns an empty vec on
/// failure; callers should treat that as "single display, unknown bounds".
pub fn list() -> Vec<DisplayInfo> {
    unsafe {
        let mut ids: [CGDirectDisplayID; 16] = [0; 16];
        let mut count: u32 = 0;
        let err = CGGetActiveDisplayList(16, ids.as_mut_ptr(), &mut count);
        if err != K_CG_ERROR_SUCCESS {
            return Vec::new();
        }
        let main = CGMainDisplayID();
        let mut out = Vec::with_capacity(count as usize);
        for &id in &ids[..count as usize] {
            let r = CGDisplayBounds(id);
            out.push(DisplayInfo {
                id,
                main: id == main,
                x: r.origin.x,
                y: r.origin.y,
                width: r.size.width,
                height: r.size.height,
            });
        }
        out
    }
}

/// Find the display id containing `(x, y)` in global coordinates. Returns
/// the main display id if no display contains the point — points slightly
/// outside any screen bounds are common for AppKit-flipped y values, so a
/// fallback is safer than `None`.
#[allow(dead_code)]
pub fn display_for_point(x: f64, y: f64, displays: &[DisplayInfo]) -> Option<u32> {
    for d in displays {
        if x >= d.x && x < d.x + d.width && y >= d.y && y < d.y + d.height {
            return Some(d.id);
        }
    }
    displays.iter().find(|d| d.main).map(|d| d.id)
}

/// Suppress dead_code warning for the c_void import on platforms that don't
/// reach the FFI block (kept for future expansion).
#[allow(dead_code)]
fn _unused() -> *const c_void {
    std::ptr::null()
}
