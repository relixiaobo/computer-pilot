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
    fn CFStringGetLength(theString: CFTypeRef) -> c_long;
    fn CFStringGetCString(
        theString: CFTypeRef,
        buffer: *mut std::ffi::c_char,
        buffer_size: c_long,
        encoding: u32,
    ) -> u8;
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

// Additional CoreGraphics FFI for annotation
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGImageGetWidth(image: CFTypeRef) -> usize;
    fn CGImageGetHeight(image: CFTypeRef) -> usize;
    fn CGColorSpaceCreateDeviceRGB() -> CFTypeRef;
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits_per_component: usize,
        bytes_per_row: usize,
        color_space: CFTypeRef,
        bitmap_info: u32,
    ) -> CFTypeRef;
    fn CGBitmapContextCreateImage(ctx: CFTypeRef) -> CFTypeRef;
    fn CGContextDrawImage(ctx: CFTypeRef, rect: CGRect, image: CFTypeRef);
    fn CGContextSetRGBStrokeColor(ctx: CFTypeRef, r: f64, g: f64, b: f64, a: f64);
    fn CGContextSetRGBFillColor(ctx: CFTypeRef, r: f64, g: f64, b: f64, a: f64);
    fn CGContextSetLineWidth(ctx: CFTypeRef, w: f64);
    fn CGContextStrokeRect(ctx: CFTypeRef, rect: CGRect);
    fn CGContextFillRect(ctx: CFTypeRef, rect: CGRect);
    fn CGContextSetTextPosition(ctx: CFTypeRef, x: f64, y: f64);
    fn CGContextSetTextMatrix(ctx: CFTypeRef, transform: CGAffineTransform);
}

// CoreText FFI for digit rendering
#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    fn CTFontCreateWithName(name: CFTypeRef, size: f64, matrix: *const c_void) -> CFTypeRef;
    fn CTLineCreateWithAttributedString(string: CFTypeRef) -> CFTypeRef;
    fn CTLineDraw(line: CFTypeRef, ctx: CFTypeRef);
    static kCTFontAttributeName: CFTypeRef;
}

// Additional CoreFoundation FFI for attributed strings
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFAttributedStringCreate(alloc: CFTypeRef, str: CFTypeRef, attrs: CFTypeRef) -> CFTypeRef;
    fn CFDictionaryCreate(
        alloc: CFTypeRef,
        keys: *const CFTypeRef,
        values: *const CFTypeRef,
        count: c_long,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> CFTypeRef;
    static kCFTypeDictionaryKeyCallBacks: c_void;
    static kCFTypeDictionaryValueCallBacks: c_void;
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGAffineTransform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    tx: f64,
    ty: f64,
}

const CG_AT_IDENTITY: CGAffineTransform = CGAffineTransform {
    a: 1.0,
    b: 0.0,
    c: 0.0,
    d: 1.0,
    tx: 0.0,
    ty: 0.0,
};

// kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little — BGRA, native Apple format
const CG_BITMAP_BGRA: u32 = 2 | (2 << 12);

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
    if ptr.is_null() { None } else { Some(ptr) }
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

unsafe fn dict_string(dict: CFTypeRef, key: &str) -> Option<String> {
    let k = cfstr(key)?;
    let v = CFDictionaryGetValue(dict, k);
    CFRelease(k);
    if v.is_null() {
        return None;
    }
    let len = CFStringGetLength(v);
    if len == 0 {
        return Some(String::new());
    }
    let buf_size = len * 4 + 1;
    let mut buf: Vec<u8> = vec![0; buf_size as usize];
    if CFStringGetCString(
        v,
        buf.as_mut_ptr() as *mut std::ffi::c_char,
        buf_size,
        CF_STRING_ENCODING_UTF8,
    ) != 0
    {
        std::ffi::CStr::from_ptr(buf.as_ptr() as *const std::ffi::c_char)
            .to_str()
            .ok()
            .map(|s| s.to_owned())
    } else {
        None
    }
}

/// Save an arbitrary `CGImageRef` (passed as `*const c_void`) to a PNG file.
/// CFReleases the image after writing — caller must hand off a +1 reference.
pub fn save_image_ptr(image: *const c_void, path: &str) -> Result<(), String> {
    if image.is_null() {
        return Err("save_image_ptr: null image".into());
    }
    unsafe {
        let result = save_cgimage_as_png(image, path);
        CFRelease(image);
        result
    }
}

unsafe fn save_cgimage_as_png(image: CFTypeRef, path: &str) -> Result<(), String> {
    let path_cf = cfstr(path).ok_or("failed to create path string")?;
    let url = CFURLCreateWithFileSystemPath(std::ptr::null(), path_cf, CF_URL_POSIX_PATH_STYLE, 0);
    CFRelease(path_cf);

    if url.is_null() {
        return Err("failed to create file URL".into());
    }

    let png_type = match cfstr("public.png") {
        Some(t) => t,
        None => {
            CFRelease(url);
            return Err("failed to create type string".into());
        }
    };
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
    /// kCGWindowSharingState: 0=None, 1=ReadOnly, 2=ReadWrite. When 0, the
    /// window opted out of being captured by other processes (deliberate
    /// privacy choice — common in messaging apps like WeChat). Both
    /// CGWindowListCreateImage and ScreenCaptureKit honor this; capture
    /// returns transparent pixels. Detect upfront so we can return a
    /// structured error instead of a useless blank PNG.
    pub sharing_state: i64,
}

/// Find the primary window of a process, returning CGWindowID + bounds.
///
/// Strategy: ask AX for the authoritative window (AXFocusedWindow →
/// AXMainWindow) and convert to CGWindowID via `_AXUIElementGetWindow`.
/// AX is the only source that knows which window is "the real one" — the
/// layer-0 list returned by CGWindowList includes menu-bar proxies, AX
/// helpers, palette stubs, and minimized stand-ins, with no semantic way
/// to distinguish them.
///
/// Fallback (AX unavailable / no a11y / app exposes no AX windows): try
/// CGWindowList with OnScreenOnly first, then unfiltered + largest area.
/// The fallback is a heuristic — but every command that uses ref/coords
/// (snapshot/click/find) goes through AX too, so when AX is unavailable
/// those commands fail anyway, and the fallback is just a best-effort.
pub fn find_window(pid: i32) -> Option<WindowInfo> {
    if let Some(geom) = crate::ax::focused_window_geom(pid) {
        // sharing_state isn't an AX attribute — read it from CGWindowList
        // by ID. If the lookup fails we assume sharable (1) since that's
        // the common case and capture will produce a real error if not.
        let sharing_state = sharing_state_for_window_id(geom.window_id).unwrap_or(1);
        return Some(WindowInfo {
            window_id: geom.window_id,
            x: geom.x,
            y: geom.y,
            width: geom.width,
            height: geom.height,
            sharing_state,
        });
    }

    if let Some(w) =
        find_window_with_options(pid, CG_WINDOW_LIST_ON_SCREEN_ONLY | CG_WINDOW_LIST_EXCLUDE_DESKTOP)
    {
        return Some(w);
    }
    find_window_with_options(pid, CG_WINDOW_LIST_EXCLUDE_DESKTOP)
}

/// Enumerate currently-running apps that own at least one capture-protected
/// window (`kCGWindowSharingState=0`). Surface candidates for `cu setup` so
/// agents know upfront which running apps will refuse `cu screenshot` —
/// WeChat is the canonical example, but Signal, password managers, banking
/// apps, and some Mac App Store Office builds set the same flag.
pub fn capture_protected_apps() -> Vec<String> {
    unsafe {
        let list = CGWindowListCopyWindowInfo(
            CG_WINDOW_LIST_ON_SCREEN_ONLY | CG_WINDOW_LIST_EXCLUDE_DESKTOP,
            0,
        );
        if list.is_null() {
            return Vec::new();
        }
        let count = CFArrayGetCount(list);
        let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for i in 0..count {
            let w = CFArrayGetValueAtIndex(list, i);
            if dict_i64(w, "kCGWindowSharingState") != Some(0) {
                continue;
            }
            // Skip layer != 0 windows (menu-bar proxies, palettes, etc.) —
            // we only care about user-visible content windows.
            if dict_i64(w, "kCGWindowLayer") != Some(0) {
                continue;
            }
            if let Some(name) = dict_string(w, "kCGWindowOwnerName")
                && !name.is_empty()
            {
                seen.insert(name);
            }
        }
        CFRelease(list);
        seen.into_iter().collect()
    }
}

/// Look up `kCGWindowSharingState` for a single CGWindowID. Used by
/// `find_window` after AX gives us the authoritative window — we only
/// need this one extra attribute that AX doesn't expose.
fn sharing_state_for_window_id(window_id: u32) -> Option<i64> {
    unsafe {
        let list = CGWindowListCopyWindowInfo(CG_WINDOW_LIST_INCLUDING_WINDOW, window_id);
        if list.is_null() {
            return None;
        }
        let count = CFArrayGetCount(list);
        let mut result = None;
        for i in 0..count {
            let w = CFArrayGetValueAtIndex(list, i);
            if let Some(id) = dict_i64(w, "kCGWindowNumber")
                && id == window_id as i64
            {
                result = Some(dict_i64(w, "kCGWindowSharingState").unwrap_or(1));
                break;
            }
        }
        CFRelease(list);
        result
    }
}

fn find_window_with_options(pid: i32, options: u32) -> Option<WindowInfo> {
    unsafe {
        let list = CGWindowListCopyWindowInfo(options, 0);
        if list.is_null() {
            return None;
        }

        let count = CFArrayGetCount(list);

        let bounds_key = match cfstr("kCGWindowBounds") {
            Some(k) => k,
            None => {
                CFRelease(list);
                return None;
            }
        };

        // Collect every layer-0 PID match, then pick the largest by area —
        // many apps own multiple layer-0 windows (full-width menu-bar
        // stubs, palettes, dropper helpers) at typical heights of 30pt or
        // less. The actual content window is the one with real area.
        let mut best: Option<(WindowInfo, f64)> = None;

        for i in 0..count {
            let w = CFArrayGetValueAtIndex(list, i);
            let w_pid = dict_i64(w, "kCGWindowOwnerPID");
            let layer = dict_i64(w, "kCGWindowLayer");
            let wid = dict_i64(w, "kCGWindowNumber");

            if w_pid != Some(pid as i64) || layer != Some(0) {
                continue;
            }
            let Some(id) = wid else { continue };

            let bounds_dict = CFDictionaryGetValue(w, bounds_key);
            if bounds_dict.is_null() {
                continue;
            }
            let x = dict_f64(bounds_dict, "X").unwrap_or(0.0);
            let y = dict_f64(bounds_dict, "Y").unwrap_or(0.0);
            let width = dict_f64(bounds_dict, "Width").unwrap_or(0.0);
            let height = dict_f64(bounds_dict, "Height").unwrap_or(0.0);

            if width <= 1.0 || height <= 1.0 {
                continue;
            }

            let area = width * height;
            let sharing_state = dict_i64(w, "kCGWindowSharingState").unwrap_or(1);
            let candidate = WindowInfo {
                window_id: id as u32,
                x,
                y,
                width,
                height,
                sharing_state,
            };

            best = match best {
                Some((cur, cur_area)) if cur_area >= area => Some((cur, cur_area)),
                _ => Some((candidate, area)),
            };
        }

        CFRelease(bounds_key);
        CFRelease(list);
        best.map(|(w, _)| w)
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

/// Capture a window and return the pixel-to-point Retina scale (typically 2.0 on Apple Silicon
/// displays, 1.0 on standard). Like `capture_window` but exposes the scale so the caller can
/// translate image pixels back to screen-space points.
/// Pre-capture sanity check: if the target window has opted out of being
/// captured (sharing state = None), refuse with a structured error rather
/// than producing a blank PNG that misleads the agent. Common case: WeChat,
/// some Microsoft Office Mac App Store builds, and other privacy-conscious apps.
fn capture_protected_check(window: &WindowInfo) -> Result<(), String> {
    if window.sharing_state == 0 {
        return Err(
            "window is capture-protected (kCGWindowSharingState=None) — \
             the target app explicitly disables screen capture as a privacy \
             measure. CGWindowListCreateImage and ScreenCaptureKit both honor \
             this; cu cannot bypass it. Use cu snapshot/find for AX-based \
             interaction; visual verification will require manual inspection."
                .into(),
        );
    }
    Ok(())
}

pub fn capture_window_with_scale(window: &WindowInfo, path: &str) -> Result<f64, String> {
    capture_protected_check(window)?;

    // Primary path: ScreenCaptureKit (cross-Space capable; CGWindowList
    // returns blank for windows on a non-active Space on macOS 14+).
    match crate::sck::capture_window_to_png(window.window_id, window.width, window.height, path) {
        Ok(px_w) => {
            let scale = if window.width > 0.0 { px_w as f64 / window.width } else { 1.0 };
            return Ok(scale);
        }
        Err(_sck_err) => {
            // Fall through to CGWindowListCreateImage. SCK can fail on
            // pre-13 macOS, when the window is gone, or when SCK's TCC
            // bucket isn't authorized (separate from regular Screen
            // Recording, in some configurations).
        }
    }

    unsafe {
        let image = CGWindowListCreateImage(
            CG_RECT_NULL,
            CG_WINDOW_LIST_INCLUDING_WINDOW,
            window.window_id,
            CG_WINDOW_IMAGE_BOUNDS_IGNORE_FRAMING,
        );
        if image.is_null() {
            return Err("failed to capture window image (SCK + CGWindowList both failed)".into());
        }
        let img_w = CGImageGetWidth(image);
        let scale = if window.width > 0.0 {
            img_w as f64 / window.width
        } else {
            1.0
        };
        let result = save_cgimage_as_png(image, path);
        CFRelease(image);
        result.map(|_| scale)
    }
}

/// Capture a specific window to a PNG file. No activation needed. Refuses
/// when the target opted out of capture (`sharing_state=0`).
pub fn capture_window(window: &WindowInfo, path: &str) -> Result<(), String> {
    capture_window_with_scale(window, path).map(|_| ())
}

/// One element to annotate. Coordinates are in screen space (same as snapshot output).
pub struct Annotation {
    pub ref_id: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Capture a window and overlay each ref's bounding box + number label, save as PNG.
/// `window_offset` is the window's screen-space origin (from `find_window`); annotations
/// are translated into image-pixel space using image_width / window_width as the scale
/// factor (handles Retina automatically).
pub fn annotate_window(
    window: &WindowInfo,
    annotations: &[Annotation],
    path: &str,
) -> Result<f64, String> {
    capture_protected_check(window)?;
    unsafe {
        // Try SCK first so cross-Space windows annotate correctly. SCK
        // returns a +1 retained CGImage; if it fails, fall back to
        // CGWindowListCreateImage.
        let image: CFTypeRef = match crate::sck::capture_window_to_cgimage(
            window.window_id,
            window.width,
            window.height,
        ) {
            Ok((ptr, _)) => ptr as CFTypeRef,
            Err(_) => {
                let img = CGWindowListCreateImage(
                    CG_RECT_NULL,
                    CG_WINDOW_LIST_INCLUDING_WINDOW,
                    window.window_id,
                    CG_WINDOW_IMAGE_BOUNDS_IGNORE_FRAMING,
                );
                if img.is_null() {
                    return Err(
                        "failed to capture window image for annotation (SCK + CGWindowList both failed)".into()
                    );
                }
                img
            }
        };

        let img_w = CGImageGetWidth(image);
        let img_h = CGImageGetHeight(image);
        if img_w == 0 || img_h == 0 {
            CFRelease(image);
            return Err("captured image has zero dimensions".into());
        }
        // Retina ratio. window.width is reported in points; image is in pixels.
        let scale = if window.width > 0.0 {
            img_w as f64 / window.width
        } else {
            1.0
        };

        let color_space = CGColorSpaceCreateDeviceRGB();
        if color_space.is_null() {
            CFRelease(image);
            return Err("failed to create RGB color space".into());
        }

        let bytes_per_row = img_w * 4;
        let ctx = CGBitmapContextCreate(
            std::ptr::null_mut(), // CG allocates
            img_w,
            img_h,
            8, // 8 bits per component
            bytes_per_row,
            color_space,
            CG_BITMAP_BGRA,
        );
        if ctx.is_null() {
            CFRelease(color_space);
            CFRelease(image);
            return Err("failed to create bitmap context".into());
        }

        // Paint the screenshot into the context (CG places bottom-left of image at bottom-left of rect)
        let full_rect = CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize {
                width: img_w as f64,
                height: img_h as f64,
            },
        };
        CGContextDrawImage(ctx, full_rect, image);
        CFRelease(image);

        // Prepare a font for the labels. Helvetica-Bold is universally available.
        let font_size = 14.0 * scale; // scale to look 14pt regardless of Retina
        let font_name = cfstr("Helvetica-Bold").ok_or_else(|| {
            CFRelease(ctx);
            CFRelease(color_space);
            "failed to create font name"
        })?;
        let font = CTFontCreateWithName(font_name, font_size, std::ptr::null());
        CFRelease(font_name);
        if font.is_null() {
            CFRelease(ctx);
            CFRelease(color_space);
            return Err("failed to create CTFont".into());
        }

        // Identity text matrix so digits aren't rotated/flipped.
        CGContextSetTextMatrix(ctx, CG_AT_IDENTITY);
        CGContextSetLineWidth(ctx, 2.0 * scale);

        for ann in annotations {
            // Translate from screen coords to image-pixel coords (top-left origin)
            let img_x = (ann.x - window.x) * scale;
            let img_y_top = (ann.y - window.y) * scale;
            let w = ann.width * scale;
            let h = ann.height * scale;

            if w < 2.0 || h < 2.0 {
                continue;
            }

            // CG y-axis is bottom-up; flip
            let cg_y = img_h as f64 - img_y_top - h;

            // Stroke red rectangle around the element
            CGContextSetRGBStrokeColor(ctx, 1.0, 0.2, 0.2, 0.95);
            let rect = CGRect {
                origin: CGPoint { x: img_x, y: cg_y },
                size: CGSize {
                    width: w,
                    height: h,
                },
            };
            CGContextStrokeRect(ctx, rect);

            // Draw label background (top-left of element, in CG that's higher y)
            let label_text = format!("{}", ann.ref_id);
            let label_w = (10.0 + 8.5 * label_text.len() as f64) * scale;
            let label_h = 18.0 * scale;
            let label_x = img_x;
            let label_y_top = img_y_top.max(0.0); // clamp so it stays in image
            let label_cg_y = img_h as f64 - label_y_top - label_h;

            CGContextSetRGBFillColor(ctx, 1.0, 0.2, 0.2, 0.95);
            let label_rect = CGRect {
                origin: CGPoint {
                    x: label_x,
                    y: label_cg_y,
                },
                size: CGSize {
                    width: label_w,
                    height: label_h,
                },
            };
            CGContextFillRect(ctx, label_rect);

            // Draw the digit(s) in white on top of the label background
            CGContextSetRGBFillColor(ctx, 1.0, 1.0, 1.0, 1.0);
            if let Some(line) = build_text_line(&label_text, font) {
                let baseline_x = label_x + 5.0 * scale;
                let baseline_y_top = label_y_top + label_h - 4.0 * scale;
                let baseline_cg_y = img_h as f64 - baseline_y_top;
                CGContextSetTextPosition(ctx, baseline_x, baseline_cg_y);
                CTLineDraw(line, ctx);
                CFRelease(line);
            }
        }

        CFRelease(font);

        // Extract the rendered image and save as PNG
        let out_image = CGBitmapContextCreateImage(ctx);
        CFRelease(ctx);
        CFRelease(color_space);

        if out_image.is_null() {
            return Err("failed to extract image from bitmap context".into());
        }

        let result = save_cgimage_as_png(out_image, path);
        CFRelease(out_image);
        result.map(|_| scale)
    }
}

/// Build a CTLine for the given digits in the given font. Caller must CFRelease.
unsafe fn build_text_line(text: &str, font: CFTypeRef) -> Option<CFTypeRef> {
    let cf_str = cfstr(text)?;

    // attrs = { kCTFontAttributeName: font }
    let keys = [kCTFontAttributeName];
    let values = [font];
    let attrs = CFDictionaryCreate(
        std::ptr::null(),
        keys.as_ptr(),
        values.as_ptr(),
        1,
        &kCFTypeDictionaryKeyCallBacks as *const _,
        &kCFTypeDictionaryValueCallBacks as *const _,
    );
    if attrs.is_null() {
        CFRelease(cf_str);
        return None;
    }

    let astr = CFAttributedStringCreate(std::ptr::null(), cf_str, attrs);
    CFRelease(attrs);
    CFRelease(cf_str);
    if astr.is_null() {
        return None;
    }

    let line = CTLineCreateWithAttributedString(astr);
    CFRelease(astr);
    if line.is_null() { None } else { Some(line) }
}

/// Capture an arbitrary rectangle of the screen (in screen-space points) to PNG.
/// Coordinates are the same space as `cu snapshot` element x/y.
pub fn capture_region(x: f64, y: f64, width: f64, height: f64, path: &str) -> Result<(), String> {
    if !x.is_finite() || !y.is_finite() || !width.is_finite() || !height.is_finite() {
        return Err("region coordinates must be finite numbers".into());
    }
    if width <= 0.0 || height <= 0.0 {
        return Err("region width and height must be > 0".into());
    }
    unsafe {
        let rect = CGRect {
            origin: CGPoint { x, y },
            size: CGSize { width, height },
        };
        // kCGNullWindowID = 0; OnScreenOnly | ExcludeDesktop matches `find_window` filter
        let image = CGWindowListCreateImage(
            rect,
            CG_WINDOW_LIST_ON_SCREEN_ONLY | CG_WINDOW_LIST_EXCLUDE_DESKTOP,
            0,
            0, // kCGWindowImageDefault
        );
        if image.is_null() {
            return Err("failed to capture screen region (off-screen, or no permission?)".into());
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
