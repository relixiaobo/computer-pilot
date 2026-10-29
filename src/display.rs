//! Multi-display awareness (D1).
//!
//! Lists active displays with their global-coordinate bounds so agents can
//! reason about multi-monitor geometry and window frames.

#![allow(unsafe_op_in_unsafe_fn)]

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
