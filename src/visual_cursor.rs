//! Optional, click-through software cursor owned by the private Broker.
//!
//! This module is presentation only. It never creates input events, never
//! activates an application, and is deliberately fail-open: if AppKit or the
//! target window is unavailable, the action continues without a cursor.

#![allow(unsafe_op_in_unsafe_fn)]

#[cfg(target_os = "macos")]
mod glyph;
#[cfg(any(target_os = "macos", test))]
mod motion;

use serde::{Deserialize, Serialize};
use std::ffi::{CStr, c_char, c_void};
use std::sync::mpsc::Receiver;
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

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    use super::motion::{Bounds, Path, Point, Pose, RenderPose, Spring};

    type Obj = *mut c_void;

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
    type SendObjArg = unsafe extern "C" fn(Obj, Obj, Obj) -> Obj;
    type SendCString = unsafe extern "C" fn(Obj, Obj, *const c_char) -> Obj;
    type SendU32 = unsafe extern "C" fn(Obj, Obj) -> u32;
    type SendCount = unsafe extern "C" fn(Obj, Obj) -> usize;
    type SendIndex = unsafe extern "C" fn(Obj, Obj, usize) -> Obj;
    type SendDouble = unsafe extern "C" fn(Obj, Obj, f64) -> Obj;
    type SendNextEvent = unsafe extern "C" fn(Obj, Obj, usize, Obj, Obj, bool) -> Obj;
    type SendI64 = unsafe extern "C" fn(Obj, Obj, i64);
    type SendRect = unsafe extern "C" fn(Obj, Obj) -> Rect;
    type SendRectArgs = unsafe extern "C" fn(Obj, Obj, Rect, usize, usize, bool) -> Obj;
    type SendRectArg = unsafe extern "C" fn(Obj, Obj, Rect) -> Obj;
    type SendVoidRect = unsafe extern "C" fn(Obj, Obj, Rect);

    type SendRelative = unsafe extern "C" fn(Obj, Obj, i64, i64);

    const NS_NONACTIVATING_PANEL_MASK: usize = 1 << 7;
    const NS_BACKING_STORE_BUFFERED: usize = 2;
    const NS_WINDOW_COLLECTION_CAN_JOIN_ALL_SPACES: usize = 1 << 0;
    const NS_WINDOW_COLLECTION_STATIONARY: usize = 1 << 4;
    const NS_WINDOW_COLLECTION_IGNORES_CYCLE: usize = 1 << 6;
    const NS_WINDOW_COLLECTION_FULL_SCREEN_AUXILIARY: usize = 1 << 8;
    const NS_WINDOW_SHARING_NONE: usize = 0;
    const CURSOR_SIZE: Size = Size {
        width: glyph::SIZE,
        height: glyph::SIZE,
    };
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

    unsafe fn send_obj_arg(receiver: Obj, selector: &CStr, value: Obj) -> Obj {
        let send: SendObjArg = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel_registerName(selector.as_ptr()), value)
    }

    unsafe fn send_cstring(receiver: Obj, selector: &CStr, value: *const c_char) -> Obj {
        let send: SendCString = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel_registerName(selector.as_ptr()), value)
    }

    unsafe fn send_u32(receiver: Obj, selector: &CStr) -> u32 {
        let send: SendU32 = std::mem::transmute(objc_msgSend as *const ());
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

    unsafe fn glyph_layer() -> Option<Obj> {
        let layer = send_id(objc_getClass(c"CALayer".as_ptr()), c"layer");
        if layer.is_null() {
            return None;
        }
        let data_send: unsafe extern "C" fn(Obj, Obj, *const u8, usize) -> Obj =
            std::mem::transmute(objc_msgSend as *const ());
        let data = data_send(
            objc_getClass(c"NSData".as_ptr()),
            sel_registerName(c"dataWithBytes:length:".as_ptr()),
            glyph::IMAGE.as_ptr(),
            glyph::IMAGE.len(),
        );
        let image = send_obj_arg(
            send_id(objc_getClass(c"NSImage".as_ptr()), c"alloc"),
            c"initWithData:",
            data,
        );
        if image.is_null() {
            return None;
        }
        let cg_send: unsafe extern "C" fn(Obj, Obj, *mut Rect, Obj, Obj) -> Obj =
            std::mem::transmute(objc_msgSend as *const ());
        let cg = cg_send(
            image,
            sel_registerName(c"CGImageForProposedRect:context:hints:".as_ptr()),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        if !cg.is_null() {
            send_void_obj(layer, c"setContents:", cg);
        }
        send_void(image, c"release");
        if cg.is_null() {
            return None;
        }
        send_void_rect(
            layer,
            c"setFrame:",
            Rect {
                origin: Point::default(),
                size: CURSOR_SIZE,
            },
        );
        Some(layer)
    }

    unsafe fn draw_glyph(layer: Obj, pose: &RenderPose, pulse: f64) {
        let send: unsafe extern "C" fn(Obj, Obj, glyph::Transform) =
            std::mem::transmute(objc_msgSend as *const ());
        send(
            layer,
            sel_registerName(c"setAffineTransform:".as_ptr()),
            glyph::transform(pose.rotation, pose.body, pulse),
        );
    }

    const PULSE_TIME: Duration = Duration::from_millis(160);

    struct Motion {
        path: Path,
        progress: Spring,
    }

    struct Overlay {
        panel: Obj,
        arrow: Obj,
        pose: Pose,
        target: Point,
        epoch: Instant,
        motion: Option<Motion>,
        last_move: Instant,
        pulse_start: Option<Instant>,
        window_id: u32,
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
                    size: CURSOR_SIZE,
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
            send_bool(panel, c"setHidesOnDeactivate:", false);
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
            let colors = objc_getClass(c"NSColor".as_ptr());
            send_void_obj(
                panel,
                c"setBackgroundColor:",
                send_id(colors, c"clearColor"),
            );

            let view = send_rect_arg(
                send_id(view_class, c"alloc"),
                c"initWithFrame:",
                Rect {
                    origin: Point::default(),
                    size: CURSOR_SIZE,
                },
            );
            if view.is_null() {
                return None;
            }
            let layer = send_id(objc_getClass(c"CALayer".as_ptr()), c"layer");
            if layer.is_null() {
                return None;
            }
            let arrow = glyph_layer()?;
            send_void_obj(layer, c"addSublayer:", arrow);
            send_void_obj(view, c"setLayer:", layer);
            send_bool(view, c"setWantsLayer:", true);
            send_void_obj(panel, c"setContentView:", view);
            Some(Self {
                panel,
                arrow,
                pose: Pose::new(Point::default(), 0.0),
                target: Point::default(),
                epoch: Instant::now(),
                motion: None,
                last_move: Instant::now(),
                pulse_start: None,
                window_id: 0,
                visible: false,
            })
        }

        // Constant-size local fog and pointer: never a path/trail layer.
        unsafe fn render(&self, pose: &RenderPose, pulse: f64) {
            let transaction = objc_getClass(c"CATransaction".as_ptr());
            send_void(transaction, c"begin");
            send_bool(transaction, c"setDisableActions:", true);
            let move_panel: unsafe extern "C" fn(Obj, Obj, Point) =
                std::mem::transmute(objc_msgSend as *const ());
            move_panel(
                self.panel,
                sel_registerName(c"setFrameOrigin:".as_ptr()),
                pose.tip - glyph::ANCHOR,
            );
            draw_glyph(self.arrow, pose, pulse);
            send_void(transaction, c"commit");
        }

        unsafe fn order_above_target(&self) {
            let send: SendRelative = std::mem::transmute(objc_msgSend as *const ());
            send(
                self.panel,
                sel_registerName(c"orderWindow:relativeTo:".as_ptr()),
                1,
                self.window_id as i64,
            );
        }

        unsafe fn move_to(&mut self, event: &Event) {
            let frame = crate::ax::focused_window_geom(event.pid)
                .filter(|window| window.window_id == event.window_id);
            let (screen_x, screen_y) = frame
                .map(|window| (window.x + event.window_x, window.y + event.window_y))
                .unwrap_or((event.x, event.y));
            let (screen, display_y) = screen_for_point(screen_x, screen_y);
            let destination = Point {
                x: screen_x,
                y: screen.origin.y + screen.size.height - (screen_y - display_y),
            };
            let now = Instant::now();
            // The first event has no prior agent position. Never invent a route
            // from the user's hardware pointer, or across unrelated windows.
            let tip = self.pose.tip();
            let animate = self.window_id == event.window_id && (tip - destination).length() > 1.0;
            self.motion = animate.then(|| Motion {
                path: motion::select_path(
                    tip,
                    destination,
                    Bounds {
                        min: screen.origin,
                        max: screen.origin
                            + Point {
                                x: screen.size.width,
                                y: screen.size.height,
                            },
                    },
                ),
                progress: Spring::default(),
            });
            if !animate {
                self.pose = Pose::new(destination, self.epoch.elapsed().as_secs_f64());
            }
            self.target = destination;
            self.window_id = event.window_id;
            self.last_move = now;
            self.pulse_start = event.pulse.then_some(
                now + if animate {
                    Duration::from_secs_f64(motion::TRAVEL_TIME)
                } else {
                    Duration::ZERO
                },
            );
            self.visible = true;
        }

        unsafe fn tick(&mut self) {
            if !self.visible {
                return;
            }
            if self.last_move.elapsed() >= HIDE_AFTER {
                send_void_obj(self.panel, c"orderOut:", std::ptr::null_mut());
                self.visible = false;
                self.motion = None;
                self.pulse_start = None;
                return;
            }
            let now = Instant::now();
            let mut target = self.target;
            if let Some(motion) = self.motion.as_mut() {
                let elapsed = now.duration_since(self.last_move).as_secs_f64();
                motion
                    .progress
                    .advance(elapsed.min(motion::TRAVEL_TIME), 1.0, 1.4, 0.9);
                target = motion.path.sample(motion.progress.value);
                if elapsed >= motion::TRAVEL_TIME || motion.progress.close_enough() {
                    target = self.target;
                    self.motion = None;
                }
            }
            let mut pulse = 0.0;
            if let Some(start) = self.pulse_start
                && let Some(elapsed) = now.checked_duration_since(start)
            {
                if elapsed < PULSE_TIME {
                    pulse = (std::f64::consts::PI * elapsed.as_secs_f64()
                        / PULSE_TIME.as_secs_f64())
                    .sin();
                } else {
                    self.pulse_start = None;
                }
            }
            let time = self.epoch.elapsed().as_secs_f64();
            let pose = self.pose.advance(target, time);
            self.render(&pose, pulse);
            self.order_above_target();
        }
    }

    #[test]
    fn reference_glyph_rasterizes() {
        type CF = *const c_void;
        #[link(name = "CoreGraphics", kind = "framework")]
        unsafe extern "C" {
            fn CGColorSpaceCreateDeviceRGB() -> CF;
            fn CGBitmapContextCreate(
                data: *mut c_void,
                width: usize,
                height: usize,
                bits: usize,
                row: usize,
                space: CF,
                info: u32,
            ) -> CF;
            fn CGContextScaleCTM(context: CF, x: f64, y: f64);
        }
        #[link(name = "CoreFoundation", kind = "framework")]
        unsafe extern "C" {
            fn CFRelease(value: CF);
        }
        unsafe {
            let pool = objc_autoreleasePoolPush();
            let layer = glyph_layer().unwrap();
            let pose = Pose::new(Point::default(), 0.0).advance(Point::default(), 0.0);
            draw_glyph(layer, &pose, 0.0);
            let mut pixels = vec![0u8; 252 * 252 * 4];
            let space = CGColorSpaceCreateDeviceRGB();
            let context =
                CGBitmapContextCreate(pixels.as_mut_ptr().cast(), 252, 252, 8, 252 * 4, space, 1);
            assert!(!context.is_null());
            CGContextScaleCTM(context, 2.0, 2.0);
            send_void_obj(layer, c"renderInContext:", context.cast_mut());
            let visible = pixels.chunks_exact(4).filter(|p| p[3] > 0).count();
            let opaque = pixels.chunks_exact(4).filter(|p| p[3] > 220).count();
            assert!(visible > 1000, "fog must rasterize: {visible}");
            assert!(opaque > 200, "pointer must rasterize: {opaque}");
            CFRelease(context);
            CFRelease(space);
            objc_autoreleasePoolPop(pool);
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
            let string_class = objc_getClass(c"NSString".as_ptr());
            let screen_number_key = if string_class.is_null() {
                std::ptr::null_mut()
            } else {
                send_cstring(
                    string_class,
                    c"stringWithUTF8String:",
                    c"NSScreenNumber".as_ptr(),
                )
            };
            let count = send_count(screens, c"count");
            for index in 0..count {
                let screen = send_index(screens, c"objectAtIndex:", index);
                let description = send_id(screen, c"deviceDescription");
                let number = if description.is_null() || screen_number_key.is_null() {
                    0
                } else {
                    let value = send_obj_arg(description, c"objectForKey:", screen_number_key);
                    if value.is_null() {
                        0
                    } else {
                        send_u32(value, c"unsignedIntValue")
                    }
                };
                if number != display.id {
                    continue;
                }
                let frame = send_rect(screen, c"frame");
                return (frame, display.y);
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
                let event = match receiver.recv_timeout(Duration::from_millis(16)) {
                    Ok(event) => Some(event),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                };
                let pool = objc_autoreleasePoolPush();
                if let (Some(event), Some(current)) = (event, overlay.as_mut()) {
                    current.move_to(&event);
                }
                if let Some(current) = overlay.as_mut() {
                    current.tick();
                }
                if let Some(app) = app {
                    pump_events(app);
                }
                objc_autoreleasePoolPop(pool);
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
