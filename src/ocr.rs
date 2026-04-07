//! OCR via macOS Vision framework using objc2 Rust bindings.
//! Zero external runtime dependencies — uses built-in on-device text recognition.
#![allow(unsafe_op_in_unsafe_fn)]

use crate::screenshot;
use objc2::AllocAnyThread;
use objc2::rc::Retained;
use objc2_core_graphics::CGImage;
use objc2_foundation::{NSArray, NSDictionary};
use objc2_vision::{
    VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel,
};
use serde::Serialize;
use std::ffi::c_void;

type CFTypeRef = *const c_void;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: CFTypeRef);
}

#[derive(Serialize)]
pub struct OcrResult {
    pub ok: bool,
    pub texts: Vec<OcrText>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct OcrText {
    pub text: String,
    pub confidence: f64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn recognize(pid: i32) -> OcrResult {
    let win = match screenshot::find_window(pid) {
        Some(w) => w,
        None => return err("no on-screen window found"),
    };

    let cg_image = screenshot::capture_window_raw(win.window_id);
    if cg_image.is_null() {
        return err("failed to capture window image");
    }

    let result = unsafe { run_ocr(cg_image, &win) };
    unsafe {
        CFRelease(cg_image);
    }
    result
}

unsafe fn run_ocr(cg_image: CFTypeRef, win: &screenshot::WindowInfo) -> OcrResult {
    // Cast CGImageRef (raw pointer) to objc2's CGImage reference
    let image_ref: &CGImage = &*(cg_image as *const CGImage);

    // Create handler from CGImage
    let options: Retained<NSDictionary<objc2_foundation::NSString, objc2::runtime::AnyObject>> =
        NSDictionary::new();
    let handler = VNImageRequestHandler::initWithCGImage_options(
        <VNImageRequestHandler as AllocAnyThread>::alloc(),
        image_ref,
        &options,
    );

    // Create text recognition request
    let request = VNRecognizeTextRequest::init(<VNRecognizeTextRequest as AllocAnyThread>::alloc());
    request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);

    // Perform
    let request_as_base: Retained<VNRequest> = Retained::cast_unchecked(request.clone());
    let requests = NSArray::from_retained_slice(&[request_as_base]);

    if let Err(e) = handler.performRequests_error(&requests) {
        return err(&format!("OCR failed: {}", e));
    }

    // Extract results
    let mut texts = Vec::new();
    if let Some(observations) = request.results() {
        for obs in observations.iter() {
            let candidates = obs.topCandidates(1);
            let count = candidates.count();
            if count == 0 {
                continue;
            }

            let candidate = candidates.objectAtIndex(0);
            let text = candidate.string().to_string();
            let confidence = candidate.confidence() as f64;

            let bbox = obs.boundingBox();
            let nx = bbox.origin.x;
            let ny = bbox.origin.y;
            let nw = bbox.size.width;
            let nh = bbox.size.height;

            texts.push(OcrText {
                text,
                confidence,
                x: (win.x + nx * win.width).round(),
                y: (win.y + (1.0 - ny - nh) * win.height).round(),
                width: (nw * win.width).round(),
                height: (nh * win.height).round(),
            });
        }
    }

    OcrResult {
        ok: true,
        texts,
        error: None,
    }
}

fn err(msg: &str) -> OcrResult {
    OcrResult {
        ok: false,
        texts: vec![],
        error: Some(msg.to_string()),
    }
}
