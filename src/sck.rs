//! ScreenCaptureKit window capture (macOS 13+).
//!
//! `CGWindowListCreateImage` is deprecated on macOS 14 and returns a blank
//! image for windows on a non-active Mission Control Space. ScreenCaptureKit
//! is Apple's replacement and captures cross-Space cleanly. This module
//! exposes a synchronous wrapper around SCK's async API; `screenshot.rs`
//! tries SCK first and falls back to `CGWindowListCreateImage` on failure
//! (older OS, SCK errors, etc.).
//!
//! SCK invokes completion handlers on its own internal dispatch queue, so
//! we bridge async→sync with `Arc<(Mutex, Condvar)>` and a wall-clock
//! timeout.

#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::c_void;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use block2::RcBlock;
use objc2::AllocAnyThread;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2_core_graphics::CGImage;
use objc2_foundation::NSError;
use objc2_screen_capture_kit::{
    SCContentFilter, SCScreenshotManager, SCShareableContent, SCStreamConfiguration, SCWindow,
};

const SHAREABLE_CONTENT_TIMEOUT: Duration = Duration::from_secs(5);
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRetain(cf: *const c_void) -> *const c_void;
}

/// Wrap `Retained<SCShareableContent>` so it crosses the bridge as `Send`.
/// SCK's content object is an immutable read-model; reading it on another
/// thread is safe in practice.
struct SafeContent(Retained<SCShareableContent>);
unsafe impl Send for SafeContent {}

/// Capture a window via SCK and return a +1 retained CGImageRef plus its
/// pixel width. Caller must `CFRelease` the returned pointer (or hand it
/// off to `screenshot::save_image_ptr`, which does the release).
///
/// Used by both the plain capture path (`capture_window_to_png`) and the
/// annotation path (`screenshot::annotate_window`) — same SCK pipeline,
/// different downstream image processing.
pub fn capture_window_to_cgimage(
    window_id: u32,
    target_pts_w: f64,
    target_pts_h: f64,
) -> Result<(*const c_void, usize), String> {
    let content = get_shareable_content()?;

    let target = unsafe {
        let windows = content.windows();
        let count = windows.count();
        let mut found: Option<Retained<SCWindow>> = None;
        for i in 0..count {
            let w = windows.objectAtIndex(i);
            if w.windowID() == window_id {
                found = Some(w);
                break;
            }
        }
        found.ok_or_else(|| format!("window_id {window_id} not visible to ScreenCaptureKit"))?
    };

    unsafe {
        let alloc = SCContentFilter::alloc();
        let filter = SCContentFilter::initWithDesktopIndependentWindow(alloc, &target);

        let cfg_alloc = SCStreamConfiguration::alloc();
        let config: Retained<SCStreamConfiguration> = msg_send![cfg_alloc, init];

        let scale = 2.0_f64;
        let px_w = (target_pts_w * scale).round().max(1.0) as usize;
        let px_h = (target_pts_h * scale).round().max(1.0) as usize;
        config.setWidth(px_w);
        config.setHeight(px_h);
        config.setShowsCursor(false);

        let image_ptr = capture_image(&filter, &config)?;
        Ok((image_ptr as *const c_void, px_w))
    }
}

/// Capture a window via SCK and write it to `path`. Returns the pixel
/// width of the saved image.
pub fn capture_window_to_png(
    window_id: u32,
    target_pts_w: f64,
    target_pts_h: f64,
    path: &str,
) -> Result<usize, String> {
    let (image_ptr, px_w) = capture_window_to_cgimage(window_id, target_pts_w, target_pts_h)?;
    crate::screenshot::save_image_ptr(image_ptr, path)?;
    Ok(px_w)
}

fn get_shareable_content() -> Result<Retained<SCShareableContent>, String> {
    let pair: Arc<(Mutex<Option<Result<SafeContent, String>>>, Condvar)> =
        Arc::new((Mutex::new(None), Condvar::new()));
    let pair_cb = Arc::clone(&pair);

    let block = RcBlock::new(move |content: *mut SCShareableContent, error: *mut NSError| {
        let result: Result<SafeContent, String> = unsafe {
            if !error.is_null() {
                let err_ref = &*error;
                Err(format!(
                    "SCShareableContent error: {}",
                    err_ref.localizedDescription()
                ))
            } else if let Some(retained) = Retained::retain(content) {
                Ok(SafeContent(retained))
            } else {
                Err("SCShareableContent returned null".into())
            }
        };
        let (lock, cvar) = &*pair_cb;
        *lock.lock().unwrap() = Some(result);
        cvar.notify_one();
    });

    unsafe {
        SCShareableContent::getShareableContentWithCompletionHandler(&block);
    }

    let (lock, cvar) = &*pair;
    let slot = lock.lock().unwrap();
    let (mut slot_after, wait_result) = cvar
        .wait_timeout_while(slot, SHAREABLE_CONTENT_TIMEOUT, |s| s.is_none())
        .map_err(|e| format!("condvar poisoned: {e}"))?;
    if wait_result.timed_out() {
        return Err("SCShareableContent timed out (5s)".into());
    }
    slot_after
        .take()
        .unwrap_or_else(|| Err("SCShareableContent: no result".into()))
        .map(|sc| sc.0)
}

fn capture_image(
    filter: &SCContentFilter,
    config: &SCStreamConfiguration,
) -> Result<usize, String> {
    let pair: Arc<(Mutex<Option<Result<usize, String>>>, Condvar)> =
        Arc::new((Mutex::new(None), Condvar::new()));
    let pair_cb = Arc::clone(&pair);

    let block = RcBlock::new(move |image: *mut CGImage, error: *mut NSError| {
        let result: Result<usize, String> = unsafe {
            if !error.is_null() {
                let err_ref = &*error;
                Err(format!(
                    "captureImage error: {}",
                    err_ref.localizedDescription()
                ))
            } else if !image.is_null() {
                CFRetain(image as *const c_void);
                Ok(image as usize)
            } else {
                Err("captureImage returned null".into())
            }
        };
        let (lock, cvar) = &*pair_cb;
        *lock.lock().unwrap() = Some(result);
        cvar.notify_one();
    });

    unsafe {
        SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(
            filter,
            config,
            Some(&block),
        );
    }

    let (lock, cvar) = &*pair;
    let slot = lock.lock().unwrap();
    let (mut slot_after, wait_result) = cvar
        .wait_timeout_while(slot, CAPTURE_TIMEOUT, |s| s.is_none())
        .map_err(|e| format!("condvar poisoned: {e}"))?;
    if wait_result.timed_out() {
        return Err("captureImage timed out (10s)".into());
    }
    slot_after
        .take()
        .unwrap_or_else(|| Err("captureImage: no result".into()))
}
