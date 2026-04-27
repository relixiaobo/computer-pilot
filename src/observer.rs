//! Single-shot AXObserver wait — replaces unconditional 500ms POST_ACTION_DELAY
//! with "wait for the first relevant AX notification, or `max_ms` ms, whichever
//! comes first" (D7).
//!
//! The agent triggers an action (click / type / set-value) and we then attach
//! an AXObserver to the app, listen for any of:
//!
//! - `AXValueChanged` (text fields update, checkboxes flip, ...)
//! - `AXFocusedUIElementChanged` (focus shifted to a new field)
//! - `AXMainWindowChanged` (a new window opened or main window switched)
//! - `AXSelectedChildrenChanged` (lists / outlines / popups opened)
//!
//! When the first notification fires, we tear down and return immediately.
//!
//! This is *not* a daemon — the observer only lives for the duration of one
//! `wait_for_settle` call. ~50ms median vs the old 500ms sleep.

#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

type Boolean = u8;
// Match the i64 width used by the FFI declaration in src/ax.rs so the linker
// sees one consistent CFStringCreateWithBytes signature.
type CFIndex = i64;
type CFTypeRef = *const c_void;
type CFStringRef = CFTypeRef;
type CFRunLoopRef = CFTypeRef;
type CFRunLoopSourceRef = CFTypeRef;
type CFRunLoopMode = CFStringRef;
type CFAllocatorRef = CFTypeRef;
type CFTimeInterval = f64;
type AXObserverRef = CFTypeRef;
type AXUIElementRef = CFTypeRef;
type AXError = i32;
const AX_OK: AXError = 0;

type AXObserverCallback = unsafe extern "C" fn(
    observer: AXObserverRef,
    element: AXUIElementRef,
    notification: CFStringRef,
    refcon: *mut c_void,
);

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: CFTypeRef);
    fn CFStringCreateWithBytes(
        alloc: CFAllocatorRef,
        bytes: *const u8,
        num_bytes: CFIndex,
        encoding: u32,
        is_external_representation: Boolean,
    ) -> CFStringRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, src: CFRunLoopSourceRef, mode: CFRunLoopMode);
    fn CFRunLoopRemoveSource(rl: CFRunLoopRef, src: CFRunLoopSourceRef, mode: CFRunLoopMode);
    fn CFRunLoopRunInMode(
        mode: CFRunLoopMode,
        seconds: CFTimeInterval,
        return_after_source_handled: Boolean,
    ) -> i32;
    static kCFRunLoopDefaultMode: CFRunLoopMode;
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXObserverCreate(
        app_pid: i32,
        callback: AXObserverCallback,
        out_observer: *mut AXObserverRef,
    ) -> AXError;
    fn AXObserverAddNotification(
        observer: AXObserverRef,
        element: AXUIElementRef,
        notification: CFStringRef,
        refcon: *mut c_void,
    ) -> AXError;
    fn AXObserverRemoveNotification(
        observer: AXObserverRef,
        element: AXUIElementRef,
        notification: CFStringRef,
    ) -> AXError;
    fn AXObserverGetRunLoopSource(observer: AXObserverRef) -> CFRunLoopSourceRef;
}

const K_CFSTRING_ENCODING_UTF8: u32 = 0x08000100;

unsafe fn cfstr(s: &str) -> CFStringRef {
    CFStringCreateWithBytes(
        std::ptr::null(),
        s.as_ptr(),
        s.len() as CFIndex,
        K_CFSTRING_ENCODING_UTF8,
        0,
    )
}

unsafe extern "C" fn fired_callback(
    _observer: AXObserverRef,
    _element: AXUIElementRef,
    _notification: CFStringRef,
    refcon: *mut c_void,
) {
    if !refcon.is_null() {
        let flag = &*(refcon as *const AtomicBool);
        flag.store(true, Ordering::SeqCst);
    }
}

/// Wait up to `max_ms` for any "UI-changed" AX notification from the app.
/// Returns the actual elapsed milliseconds. If the notification fires earlier,
/// returns immediately; if subscription fails or the runloop times out, returns
/// `max_ms`. Always safe to call — never panics, never lingers past `max_ms`.
pub fn wait_for_settle(pid: i32, max_ms: u64) -> u64 {
    use std::time::Instant;

    let start = Instant::now();
    let fired = Box::new(AtomicBool::new(false));
    let fired_ptr: *const AtomicBool = &*fired;

    unsafe {
        let mut observer: AXObserverRef = std::ptr::null();
        let err = AXObserverCreate(pid, fired_callback, &mut observer);
        if err != AX_OK || observer.is_null() {
            // Fall back to the legacy fixed sleep.
            std::thread::sleep(std::time::Duration::from_millis(max_ms));
            return start.elapsed().as_millis() as u64;
        }

        let app_el = AXUIElementCreateApplication(pid);
        if app_el.is_null() {
            CFRelease(observer);
            std::thread::sleep(std::time::Duration::from_millis(max_ms));
            return start.elapsed().as_millis() as u64;
        }

        // Subscribe to notifications likely to fire after a click/type/set-value.
        let notifs = [
            "AXValueChanged",
            "AXFocusedUIElementChanged",
            "AXMainWindowChanged",
            "AXSelectedChildrenChanged",
        ];
        let mut subscribed: Vec<CFStringRef> = Vec::new();
        for name in &notifs {
            let cf = cfstr(name);
            if !cf.is_null() {
                let e = AXObserverAddNotification(observer, app_el, cf, fired_ptr as *mut c_void);
                if e == AX_OK {
                    subscribed.push(cf);
                } else {
                    CFRelease(cf);
                }
            }
        }

        if subscribed.is_empty() {
            CFRelease(app_el);
            CFRelease(observer);
            std::thread::sleep(std::time::Duration::from_millis(max_ms));
            return start.elapsed().as_millis() as u64;
        }

        let src = AXObserverGetRunLoopSource(observer);
        let rl = CFRunLoopGetCurrent();
        if !src.is_null() {
            CFRunLoopAddSource(rl, src, kCFRunLoopDefaultMode);
        }

        // Run the runloop in 50ms slices so we can poll the fired flag and
        // bail out as soon as it flips. Cap the total at `max_ms`.
        let slice_secs: CFTimeInterval = 0.05;
        let max_loops = (max_ms / 50).max(1);
        for _ in 0..max_loops {
            let _ = CFRunLoopRunInMode(kCFRunLoopDefaultMode, slice_secs, 1);
            if fired.load(Ordering::SeqCst) {
                break;
            }
        }

        if !src.is_null() {
            CFRunLoopRemoveSource(rl, src, kCFRunLoopDefaultMode);
        }
        for cf in &subscribed {
            AXObserverRemoveNotification(observer, app_el, *cf);
            CFRelease(*cf);
        }
        CFRelease(app_el);
        CFRelease(observer);
    }

    start.elapsed().as_millis() as u64
}
