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
    /// Aggregate confidence stats so the agent doesn't have to walk the
    /// array to know "is this OCR result trustworthy". Populated only on
    /// success and only when at least one text was recognized.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_confidence: Option<f64>,
    /// Count of recognitions below 0.5 confidence — Apple Vision returns
    /// dubious matches with confidence in the 0.2-0.4 range that look
    /// real but are often hallucinated. Agents should treat them as hints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub low_confidence_count: Option<usize>,
    /// Attached when at least one recognition is low-confidence. Tells
    /// the agent to verify visually rather than acting on the OCR alone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence_hint: Option<String>,
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

    let (min_conf, mean_conf, low_count) = if texts.is_empty() {
        (None, None, None)
    } else {
        let mut min = f64::INFINITY;
        let mut sum = 0.0;
        let mut low = 0usize;
        for t in &texts {
            if t.confidence < min {
                min = t.confidence;
            }
            sum += t.confidence;
            if t.confidence < 0.5 {
                low += 1;
            }
        }
        (Some(min), Some(sum / texts.len() as f64), Some(low))
    };

    let confidence_hint = match low_count {
        Some(n) if n > 0 => Some(format!(
            "{n} of {total} recognitions are below 0.5 confidence — Vision returns plausible-looking hallucinations in this range. Verify visually before acting on these results.",
            total = texts.len()
        )),
        _ => None,
    };

    OcrResult {
        ok: true,
        texts,
        min_confidence: min_conf,
        mean_confidence: mean_conf,
        low_confidence_count: low_count,
        confidence_hint,
        error: None,
    }
}

fn err(msg: &str) -> OcrResult {
    OcrResult {
        ok: false,
        texts: vec![],
        min_confidence: None,
        mean_confidence: None,
        low_confidence_count: None,
        confidence_hint: None,
        error: Some(msg.to_string()),
    }
}
