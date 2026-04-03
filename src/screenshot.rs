//! Screenshot via CGWindowListCreateImage — captures window content without activation.
#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::{c_long, c_void};

type CFTypeRef = *const c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

/// CGRectNull — tells CGWindowListCreateImage to use the window's own bounds.
const CG_RECT_NULL: CGRect = CGRect {
    origin: CGPoint {
        x: f64::INFINITY,
        y: f64::INFINITY,
    },
    size: CGSize {
        width: 0.0,
        height: 0.0,
    },
};

const CG_WINDOW_LIST_ON_SCREEN_ONLY: u32 = 1;
const CG_WINDOW_LIST_EXCLUDE_DESKTOP: u32 = 16;
const CG_WINDOW_LIST_INCLUDING_WINDOW: u32 = 8;
const CG_WINDOW_IMAGE_BOUNDS_IGNORE_FRAMING: u32 = 1;

const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const CF_URL_POSIX_PATH_STYLE: isize = 0;
const CF_NUMBER_SINT64_TYPE: isize = 4;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGWindowListCopyWindowInfo(option: u32, relativeToWindow: u32) -> CFTypeRef;
    fn CGWindowListCreateImage(
        screenBounds: CGRect,
        listOption: u32,
        windowID: u32,
        imageOption: u32,
    ) -> CFTypeRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: CFTypeRef);
    fn CFArrayGetCount(array: CFTypeRef) -> c_long;
    fn CFArrayGetValueAtIndex(array: CFTypeRef, index: c_long) -> CFTypeRef;
    fn CFDictionaryGetValue(dict: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
    fn CFNumberGetValue(number: CFTypeRef, the_type: isize, value_ptr: *mut c_void) -> u8;
    fn CFStringCreateWithBytes(
        alloc: CFTypeRef,
        bytes: *const u8,
        num_bytes: c_long,
        encoding: u32,
        is_external_rep: u8,
    ) -> CFTypeRef;
    fn CFURLCreateWithFileSystemPath(
        alloc: CFTypeRef,
        file_path: CFTypeRef,
        path_style: isize,
        is_directory: u8,
    ) -> CFTypeRef;
}

#[link(name = "ImageIO", kind = "framework")]
unsafe extern "C" {
    fn CGImageDestinationCreateWithURL(
        url: CFTypeRef,
        type_: CFTypeRef,
        count: usize,
        options: CFTypeRef,
    ) -> CFTypeRef;
    fn CGImageDestinationAddImage(idst: CFTypeRef, image: CFTypeRef, properties: CFTypeRef);
    fn CGImageDestinationFinalize(idst: CFTypeRef) -> u8;
}

// ── Helpers ─────────────────────────────────────────────────────────────────

unsafe fn cfstr(s: &str) -> Option<CFTypeRef> {
    let ptr = CFStringCreateWithBytes(
        std::ptr::null(),
        s.as_ptr(),
        s.len() as c_long,
        CF_STRING_ENCODING_UTF8,
        0,
    );
    if ptr.is_null() {
        None
    } else {
        Some(ptr)
    }
}

unsafe fn dict_f64(dict: CFTypeRef, key: &str) -> Option<f64> {
    let k = cfstr(key)?;
    let v = CFDictionaryGetValue(dict, k);
    CFRelease(k);
    if v.is_null() {
        return None;
    }
    let mut val: f64 = 0.0;
    if CFNumberGetValue(v, 6, &mut val as *mut _ as *mut c_void) != 0 {
        // kCFNumberFloat64Type = 6
        Some(val)
    } else {
        None
    }
}

unsafe fn dict_i64(dict: CFTypeRef, key: &str) -> Option<i64> {
    let k = cfstr(key)?;
    let v = CFDictionaryGetValue(dict, k);
    CFRelease(k);
    if v.is_null() {
        return None;
    }
    let mut val: i64 = 0;
    if CFNumberGetValue(v, CF_NUMBER_SINT64_TYPE, &mut val as *mut _ as *mut c_void) != 0 {
        Some(val)
    } else {
        None
    }
}

unsafe fn save_cgimage_as_png(image: CFTypeRef, path: &str) -> Result<(), String> {
    let path_cf = cfstr(path).ok_or("failed to create path string")?;
    let url = CFURLCreateWithFileSystemPath(std::ptr::null(), path_cf, CF_URL_POSIX_PATH_STYLE, 0);
    CFRelease(path_cf);

    if url.is_null() {
        return Err("failed to create file URL".into());
    }

    let png_type = cfstr("public.png").ok_or("failed to create type string")?;
    let dest = CGImageDestinationCreateWithURL(url, png_type, 1, std::ptr::null());
    CFRelease(png_type);
    CFRelease(url);

    if dest.is_null() {
        return Err("failed to create image destination".into());
    }

    CGImageDestinationAddImage(dest, image, std::ptr::null());
    let ok = CGImageDestinationFinalize(dest);
    CFRelease(dest);

    if ok != 0 {
        Ok(())
    } else {
        Err("failed to write PNG file".into())
    }
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Window info from a single CGWindowList query — ensures capture and offset refer to the same window.
pub struct WindowInfo {
    pub window_id: u32,
    pub x: f64,
    pub y: f64,
    #[allow(dead_code)]
    pub width: f64,
    #[allow(dead_code)]
    pub height: f64,
}

/// Find the main (layer 0) window of a process, returning ID + bounds from the same source.
pub fn find_window(pid: i32) -> Option<WindowInfo> {
    unsafe {
        let list = CGWindowListCopyWindowInfo(
            CG_WINDOW_LIST_ON_SCREEN_ONLY | CG_WINDOW_LIST_EXCLUDE_DESKTOP,
            0,
        );
        if list.is_null() {
            return None;
        }

        let count = CFArrayGetCount(list);
        let mut result = None;

        let bounds_key = cfstr("kCGWindowBounds")?;

        for i in 0..count {
            let w = CFArrayGetValueAtIndex(list, i);
            let w_pid = dict_i64(w, "kCGWindowOwnerPID");
            let layer = dict_i64(w, "kCGWindowLayer");
            let wid = dict_i64(w, "kCGWindowNumber");

            if w_pid == Some(pid as i64) && layer == Some(0) {
                if let Some(id) = wid {
                    // Read bounds from the same window dict
                    let bounds_dict = CFDictionaryGetValue(w, bounds_key);
                    let (x, y, width, height) = if !bounds_dict.is_null() {
                        (
                            dict_f64(bounds_dict, "X").unwrap_or(0.0),
                            dict_f64(bounds_dict, "Y").unwrap_or(0.0),
                            dict_f64(bounds_dict, "Width").unwrap_or(0.0),
                            dict_f64(bounds_dict, "Height").unwrap_or(0.0),
                        )
                    } else {
                        (0.0, 0.0, 0.0, 0.0)
                    };

                    result = Some(WindowInfo {
                        window_id: id as u32,
                        x,
                        y,
                        width,
                        height,
                    });
                    break;
                }
            }
        }

        CFRelease(bounds_key);
        CFRelease(list);
        result
    }
}


/// Capture a window as a raw CGImageRef (caller must CFRelease).
#[allow(dead_code)]
pub fn capture_window_raw(window_id: u32) -> CFTypeRef {
    unsafe {
        CGWindowListCreateImage(
            CG_RECT_NULL,
            CG_WINDOW_LIST_INCLUDING_WINDOW,
            window_id,
            CG_WINDOW_IMAGE_BOUNDS_IGNORE_FRAMING,
        )
    }
}

/// Capture a specific window by ID to a PNG file. No activation needed.
pub fn capture_window(window_id: u32, path: &str) -> Result<(), String> {
    unsafe {
        let image = CGWindowListCreateImage(
            CG_RECT_NULL,
            CG_WINDOW_LIST_INCLUDING_WINDOW,
            window_id,
            CG_WINDOW_IMAGE_BOUNDS_IGNORE_FRAMING,
        );
        if image.is_null() {
            return Err("failed to capture window image".into());
        }

        let result = save_cgimage_as_png(image, path);
        CFRelease(image);
        result
    }
}

/// Capture the full virtual desktop (all monitors) to a PNG file.
pub fn capture_full_screen(path: &str) -> Result<(), String> {
    unsafe {
        // CGRectNull captures the entire virtual desktop across all monitors.
        let image = CGWindowListCreateImage(
            CG_RECT_NULL,
            CG_WINDOW_LIST_ON_SCREEN_ONLY,
            0,
            0, // kCGWindowImageDefault
        );
        if image.is_null() {
            return Err("failed to capture screen image".into());
        }

        let result = save_cgimage_as_png(image, path);
        CFRelease(image);
        result
    }
}
