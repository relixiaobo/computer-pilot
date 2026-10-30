//! Optional, click-through software cursor owned by the private Broker.
//!
//! This module is presentation only. It never creates input events, never
//! activates an application, and is deliberately fail-open: if AppKit or the
//! target window is unavailable, the action continues without a cursor.

#![allow(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};
use std::ffi::{CStr, c_char, c_void};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Event {
    pub pid: i32,
    pub window_id: u32,
    pub x: f64,
    pub y: f64,
    pub window_x: f64,
    pub window_y: f64,
    pub pulse: bool,
}

pub fn enabled() -> bool {
    std::env::var("COMPUTER_PILOT_VISUAL_CURSOR")
        .ok()
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "on"))
}

pub fn channel() -> (Sender<Event>, Receiver<Event>) {
    mpsc::channel()
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    type Obj = *mut c_void;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Point {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Size {
        width: f64,
        height: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Rect {
        origin: Point,
        size: Size,
    }

    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}

    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Obj;
        fn sel_registerName(name: *const c_char) -> Obj;
        fn objc_msgSend();
        fn objc_autoreleasePoolPush() -> Obj;
        fn objc_autoreleasePoolPop(pool: Obj);
        static NSDefaultRunLoopMode: Obj;
    }

    type SendId = unsafe extern "C" fn(Obj, Obj) -> Obj;
    type SendVoidObj = unsafe extern "C" fn(Obj, Obj, Obj);
    type SendVoid = unsafe extern "C" fn(Obj, Obj);
    type SendBool = unsafe extern "C" fn(Obj, Obj, bool);
    type SendUsize = unsafe extern "C" fn(Obj, Obj, usize);
    type SendCount = unsafe extern "C" fn(Obj, Obj) -> usize;
    type SendIndex = unsafe extern "C" fn(Obj, Obj, usize) -> Obj;
    type SendDouble = unsafe extern "C" fn(Obj, Obj, f64) -> Obj;
    type SendNextEvent = unsafe extern "C" fn(Obj, Obj, usize, Obj, Obj, bool) -> Obj;
    type SendI64 = unsafe extern "C" fn(Obj, Obj, i64);
    type SendRect = unsafe extern "C" fn(Obj, Obj) -> Rect;
    type SendRectArgs = unsafe extern "C" fn(Obj, Obj, Rect, usize, usize, bool) -> Obj;
    type SendRectArg = unsafe extern "C" fn(Obj, Obj, Rect) -> Obj;
    type SendVoidRect = unsafe extern "C" fn(Obj, Obj, Rect);
    type SendRectBool = unsafe extern "C" fn(Obj, Obj, Rect, bool);
    type SendRelative = unsafe extern "C" fn(Obj, Obj, i64, i64);

    const NS_NONACTIVATING_PANEL_MASK: usize = 1 << 7;
    const NS_BACKING_STORE_BUFFERED: usize = 2;
    const NS_WINDOW_COLLECTION_CAN_JOIN_ALL_SPACES: usize = 1 << 0;
    const NS_WINDOW_COLLECTION_STATIONARY: usize = 1 << 4;
    const NS_WINDOW_COLLECTION_IGNORES_CYCLE: usize = 1 << 6;
    const NS_WINDOW_COLLECTION_FULL_SCREEN_AUXILIARY: usize = 1 << 8;
    const NS_WINDOW_SHARING_NONE: usize = 0;
    const CURSOR_SIZE: f64 = 22.0;
    const HIDE_AFTER: Duration = Duration::from_secs(12);

    unsafe fn send_id(receiver: Obj, selector: &CStr) -> Obj {
        let send: SendId = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel_registerName(selector.as_ptr()))
    }

    unsafe fn send_bool(receiver: Obj, selector: &CStr, value: bool) {
        let send: SendBool = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel_registerName(selector.as_ptr()), value);
    }

    unsafe fn send_void_obj(receiver: Obj, selector: &CStr, value: Obj) {
        let send: SendVoidObj = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel_registerName(selector.as_ptr()), value);
    }

    unsafe fn send_void(receiver: Obj, selector: &CStr) {
        let send: SendVoid = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel_registerName(selector.as_ptr()));
    }

    unsafe fn send_usize(receiver: Obj, selector: &CStr, value: usize) {
        let send: SendUsize = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel_registerName(selector.as_ptr()), value);
    }

    unsafe fn send_count(receiver: Obj, selector: &CStr) -> usize {
        let send: SendCount = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel_registerName(selector.as_ptr()))
    }

    unsafe fn send_index(receiver: Obj, selector: &CStr, index: usize) -> Obj {
        let send: SendIndex = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel_registerName(selector.as_ptr()), index)
    }

    unsafe fn send_double(receiver: Obj, selector: &CStr, value: f64) -> Obj {
        let send: SendDouble = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel_registerName(selector.as_ptr()), value)
    }

    unsafe fn send_i64(receiver: Obj, selector: &CStr, value: i64) {
        let send: SendI64 = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel_registerName(selector.as_ptr()), value);
    }

    unsafe fn send_rect(receiver: Obj, selector: &CStr) -> Rect {
        let send: SendRect = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel_registerName(selector.as_ptr()))
    }

    unsafe fn send_rect_args(
        receiver: Obj,
        selector: &CStr,
        rect: Rect,
        mask: usize,
        backing: usize,
        defer: bool,
    ) -> Obj {
        let send: SendRectArgs = std::mem::transmute(objc_msgSend as *const ());
        send(
            receiver,
            sel_registerName(selector.as_ptr()),
            rect,
            mask,
            backing,
            defer,
        )
    }

    unsafe fn send_rect_arg(receiver: Obj, selector: &CStr, rect: Rect) -> Obj {
        let send: SendRectArg = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel_registerName(selector.as_ptr()), rect)
    }

    unsafe fn send_void_rect(receiver: Obj, selector: &CStr, rect: Rect) {
        let send: SendVoidRect = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel_registerName(selector.as_ptr()), rect);
    }

    unsafe fn send_rect_bool(receiver: Obj, selector: &CStr, rect: Rect, display: bool) {
        let send: SendRectBool = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel_registerName(selector.as_ptr()), rect, display);
    }

    struct Overlay {
        panel: Obj,
        view: Obj,
        last_move: Instant,
        pulse_until: Option<Instant>,
        center: Point,
        visible: bool,
    }

    impl Overlay {
        unsafe fn create() -> Option<Self> {
            let panel_class = objc_getClass(c"NSPanel".as_ptr());
            let view_class = objc_getClass(c"NSView".as_ptr());
            if panel_class.is_null() || view_class.is_null() {
                return None;
            }
            let panel = send_rect_args(
                send_id(panel_class, c"alloc"),
                c"initWithContentRect:styleMask:backing:defer:",
                Rect {
                    origin: Point {
                        x: -1000.0,
                        y: -1000.0,
                    },
                    size: Size {
                        width: CURSOR_SIZE,
                        height: CURSOR_SIZE,
                    },
                },
                NS_NONACTIVATING_PANEL_MASK,
                NS_BACKING_STORE_BUFFERED,
                false,
            );
            if panel.is_null() {
                return None;
            }
            send_bool(panel, c"setOpaque:", false);
            send_bool(panel, c"setIgnoresMouseEvents:", true);
            send_bool(panel, c"setHasShadow:", false);
            send_usize(panel, c"setSharingType:", NS_WINDOW_SHARING_NONE);
            send_usize(
                panel,
                c"setCollectionBehavior:",
                NS_WINDOW_COLLECTION_CAN_JOIN_ALL_SPACES
                    | NS_WINDOW_COLLECTION_STATIONARY
                    | NS_WINDOW_COLLECTION_IGNORES_CYCLE
                    | NS_WINDOW_COLLECTION_FULL_SCREEN_AUXILIARY,
            );
            send_i64(panel, c"setLevel:", 0);

            let view = send_rect_arg(
                send_id(view_class, c"alloc"),
                c"initWithFrame:",
                Rect {
                    origin: Point { x: 0.0, y: 0.0 },
                    size: Size {
                        width: CURSOR_SIZE,
                        height: CURSOR_SIZE,
                    },
                },
            );
            if view.is_null() {
                return None;
            }
            send_bool(view, c"setWantsLayer:", true);
            let layer = send_id(view, c"layer");
            if !layer.is_null() {
                let color_class = objc_getClass(c"NSColor".as_ptr());
                if !color_class.is_null() {
                    let blue = send_id(color_class, c"systemBlueColor");
                    let white = send_id(color_class, c"whiteColor");
                    if !blue.is_null() {
                        let cg = send_id(blue, c"CGColor");
                        if !cg.is_null() {
                            send_void_obj(layer, c"setBackgroundColor:", cg);
                        }
                    }
                    if !white.is_null() {
                        let cg = send_id(white, c"CGColor");
                        if !cg.is_null() {
                            send_void_obj(layer, c"setBorderColor:", cg);
                        }
                    }
                }
                let send_float: unsafe extern "C" fn(Obj, Obj, f64) =
                    std::mem::transmute(objc_msgSend as *const ());
                send_float(
                    layer,
                    sel_registerName(c"setCornerRadius:".as_ptr()),
                    CURSOR_SIZE / 2.0,
                );
                send_float(layer, sel_registerName(c"setBorderWidth:".as_ptr()), 2.0);
            }
            send_void_obj(panel, c"setContentView:", view);
            Some(Self {
                panel,
                view,
                last_move: Instant::now(),
                pulse_until: None,
                center: Point::default(),
                visible: false,
            })
        }

        unsafe fn move_to(&mut self, event: &Event) {
            let frame = crate::screenshot::find_window(event.pid)
                .filter(|window| window.window_id == event.window_id);
            let (screen_x, screen_y) = frame
                .map(|window| (window.x + event.window_x, window.y + event.window_y))
                .unwrap_or((event.x, event.y));
            let (screen, display_y) = screen_for_point(screen_x, screen_y);
            let origin = Point {
                x: screen_x,
                y: screen.origin.y + screen.size.height - (screen_y - display_y),
            };
            let size = if event.pulse {
                CURSOR_SIZE * 1.6
            } else {
                CURSOR_SIZE
            };
            let panel_frame = Rect {
                origin: Point {
                    x: origin.x - size / 2.0,
                    y: origin.y - size / 2.0,
                },
                size: Size {
                    width: size,
                    height: size,
                },
            };
            send_rect_bool(self.panel, c"setFrame:display:", panel_frame, true);
            send_void_rect(
                self.view,
                c"setFrame:",
                Rect {
                    origin: Point { x: 0.0, y: 0.0 },
                    size: panel_frame.size,
                },
            );
            let layer = send_id(self.view, c"layer");
            if !layer.is_null() {
                let send_float: unsafe extern "C" fn(Obj, Obj, f64) =
                    std::mem::transmute(objc_msgSend as *const ());
                send_float(
                    layer,
                    sel_registerName(c"setCornerRadius:".as_ptr()),
                    size / 2.0,
                );
            }
            self.center = origin;
            let send: SendRelative = std::mem::transmute(objc_msgSend as *const ());
            send(
                self.panel,
                sel_registerName(c"orderWindow:relativeTo:".as_ptr()),
                1,
                event.window_id as i64,
            );
            if !self.visible {
                send_void(self.panel, c"orderFrontRegardless");
                self.visible = true;
            }
            send_void(self.panel, c"displayIfNeeded");
            self.last_move = Instant::now();
            self.pulse_until = event
                .pulse
                .then(|| Instant::now() + Duration::from_millis(160));
        }

        unsafe fn hide(&mut self) {
            if self.visible {
                send_void_obj(self.panel, c"orderOut:", std::ptr::null_mut());
                self.visible = false;
            }
        }

        unsafe fn tick(&mut self) {
            if self
                .pulse_until
                .is_some_and(|until| Instant::now() >= until)
            {
                self.pulse_until = None;
                send_rect_bool(
                    self.panel,
                    c"setFrame:display:",
                    Rect {
                        origin: Point {
                            x: self.center.x - CURSOR_SIZE / 2.0,
                            y: self.center.y - CURSOR_SIZE / 2.0,
                        },
                        size: Size {
                            width: CURSOR_SIZE,
                            height: CURSOR_SIZE,
                        },
                    },
                    true,
                );
                send_void_rect(
                    self.view,
                    c"setFrame:",
                    Rect {
                        origin: Point { x: 0.0, y: 0.0 },
                        size: Size {
                            width: CURSOR_SIZE,
                            height: CURSOR_SIZE,
                        },
                    },
                );
                let layer = send_id(self.view, c"layer");
                if !layer.is_null() {
                    let send_float: unsafe extern "C" fn(Obj, Obj, f64) =
                        std::mem::transmute(objc_msgSend as *const ());
                    send_float(
                        layer,
                        sel_registerName(c"setCornerRadius:".as_ptr()),
                        CURSOR_SIZE / 2.0,
                    );
                }
            }
            if self.visible && self.last_move.elapsed() >= HIDE_AFTER {
                self.hide();
            }
        }
    }

    unsafe fn screen_for_point(x: f64, y: f64) -> (Rect, f64) {
        let displays = crate::display::list();
        let display = displays.iter().find(|display| {
            x >= display.x
                && x < display.x + display.width
                && y >= display.y
                && y < display.y + display.height
        });
        let screens = send_id(objc_getClass(c"NSScreen".as_ptr()), c"screens");
        if let Some(display) = display
            && !screens.is_null()
        {
            let count = send_count(screens, c"count");
            for index in 0..count {
                let screen = send_index(screens, c"objectAtIndex:", index);
                let frame = send_rect(screen, c"frame");
                if (frame.origin.x - display.x).abs() < 1.0
                    && (frame.size.width - display.width).abs() < 1.0
                    && (frame.size.height - display.height).abs() < 1.0
                {
                    return (frame, display.y);
                }
            }
        }
        let main = send_rect(
            send_id(objc_getClass(c"NSScreen".as_ptr()), c"mainScreen"),
            c"frame",
        );
        (main, 0.0)
    }

    unsafe fn pump_events(app: Obj) {
        let date_class = objc_getClass(c"NSDate".as_ptr());
        if date_class.is_null() {
            return;
        }
        let date = send_double(date_class, c"dateWithTimeIntervalSinceNow:", 0.0);
        if date.is_null() {
            return;
        }
        let next_event: SendNextEvent = std::mem::transmute(objc_msgSend as *const ());
        let event = next_event(
            app,
            sel_registerName(c"nextEventMatchingMask:untilDate:inMode:dequeue:".as_ptr()),
            usize::MAX,
            date,
            NSDefaultRunLoopMode,
            true,
        );
        if !event.is_null() {
            send_void_obj(app, c"sendEvent:", event);
            send_void(app, c"updateWindows");
        }
    }

    pub fn run_loop(receiver: Receiver<Event>) {
        unsafe {
            let pool = objc_autoreleasePoolPush();
            let app_class = objc_getClass(c"NSApplication".as_ptr());
            let app = if !app_class.is_null() {
                let app = send_id(app_class, c"sharedApplication");
                if !app.is_null() {
                    let send_bool_return: unsafe extern "C" fn(Obj, Obj, usize) -> bool =
                        std::mem::transmute(objc_msgSend as *const ());
                    let _ = send_bool_return(
                        app,
                        sel_registerName(c"setActivationPolicy:".as_ptr()),
                        2,
                    );
                }
                (!app.is_null()).then_some(app)
            } else {
                None
            };
            let mut overlay = Overlay::create();
            objc_autoreleasePoolPop(pool);

            loop {
                match receiver.recv_timeout(Duration::from_millis(16)) {
                    Ok(event) => {
                        let pool = objc_autoreleasePoolPush();
                        if let Some(current) = overlay.as_mut() {
                            current.move_to(&event);
                        }
                        objc_autoreleasePoolPop(pool);
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
                if let Some(current) = overlay.as_mut() {
                    current.tick();
                }
                if let Some(app) = app {
                    let pool = objc_autoreleasePoolPush();
                    pump_events(app);
                    objc_autoreleasePoolPop(pool);
                }
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod macos {
    use super::*;

    pub fn run_loop(receiver: Receiver<Event>) {
        while receiver.recv().is_ok() {}
    }
}

pub fn run_loop(receiver: Receiver<Event>) {
    macos::run_loop(receiver);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_event_preserves_window_anchor_and_pulse() {
        let event = Event {
            pid: 42,
            window_id: 7,
            x: 120.0,
            y: 80.0,
            window_x: 20.0,
            window_y: 30.0,
            pulse: true,
        };
        let json = serde_json::to_value(event).expect("cursor event is serializable");
        assert_eq!(json["window_id"], 7);
        assert_eq!(json["window_x"], 20.0);
        assert_eq!(json["pulse"], true);
    }
}
