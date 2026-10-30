//! Single-action AX notification subscription, armed before dispatch so synchronous
//! changes are not missed. The observer is removed on success and on early errors.
//! Unsupported apps keep the bounded post-action fallback delay.

#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

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

pub struct ActionObserver {
    observer: AXObserverRef,
    app: AXUIElementRef,
    subscribed: Vec<CFStringRef>,
    fired: Box<AtomicBool>,
}

impl ActionObserver {
    /// Register before dispatch, on the same thread that will wait and drop.
    pub fn new(pid: i32) -> Option<Self> {
        unsafe {
            let mut observer = std::ptr::null();
            if AXObserverCreate(pid, fired_callback, &mut observer) != AX_OK || observer.is_null() {
                return None;
            }
            let mut pending = Self {
                observer,
                app: AXUIElementCreateApplication(pid),
                subscribed: Vec::new(),
                fired: Box::new(AtomicBool::new(false)),
            };
            if pending.app.is_null() {
                return None;
            }
            for name in [
                "AXValueChanged",
                "AXFocusedUIElementChanged",
                "AXMainWindowChanged",
                "AXSelectedChildrenChanged",
            ] {
                let cf = cfstr(name);
                if cf.is_null() {
                    continue;
                }
                let refcon = &*pending.fired as *const AtomicBool as *mut c_void;
                if AXObserverAddNotification(observer, pending.app, cf, refcon) == AX_OK {
                    pending.subscribed.push(cf);
                } else {
                    CFRelease(cf);
                }
            }
            let source = AXObserverGetRunLoopSource(observer);
            if pending.subscribed.is_empty() || source.is_null() {
                return None;
            }
            CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopDefaultMode);
            Some(pending)
        }
    }
}

impl Drop for ActionObserver {
    fn drop(&mut self) {
        unsafe {
            let source = AXObserverGetRunLoopSource(self.observer);
            if !source.is_null() {
                CFRunLoopRemoveSource(CFRunLoopGetCurrent(), source, kCFRunLoopDefaultMode);
            }
            for cf in &self.subscribed {
                AXObserverRemoveNotification(self.observer, self.app, *cf);
                CFRelease(*cf);
            }
            if !self.app.is_null() {
                CFRelease(self.app);
            }
            CFRelease(self.observer);
        }
    }
}

/// Wait for the already-armed notification, bounded by an actual deadline.
/// A queued notification from a synchronous action is handled immediately.
pub fn wait_for_settle(observer: Option<ActionObserver>, max_ms: u64) -> u64 {
    let start = Instant::now();
    let budget = Duration::from_millis(max_ms);
    if let Some(observer) = observer {
        while !observer.fired.load(Ordering::SeqCst) {
            let remaining = budget.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                break;
            }
            unsafe {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, remaining.as_secs_f64(), 1);
            }
        }
    } else {
        std::thread::sleep(budget);
    }
    start.elapsed().as_millis() as u64
}
