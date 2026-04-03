//! OCR via macOS Vision framework.
//! Uses a tiny Swift helper that gets compiled once and cached.

use crate::screenshot;
use serde::Serialize;
use std::process::{Command, Stdio};
use std::path::PathBuf;

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

const SWIFT_OCR_SRC: &str = r#"
import Vision
import Foundation
import ImageIO

guard CommandLine.arguments.count > 1 else { print("[]"); exit(0) }
let path = CommandLine.arguments[1]
let url = URL(fileURLWithPath: path)
guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
      let cgImage = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
    print("[]"); exit(0)
}

let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate

try? handler.perform([request])

var output: [[String: Any]] = []
for obs in (request.results ?? []) {
    guard let candidate = obs.topCandidates(1).first else { continue }
    let b = obs.boundingBox
    output.append([
        "t": candidate.string,
        "c": Double(candidate.confidence),
        "x": b.origin.x, "y": b.origin.y,
        "w": b.width, "h": b.height
    ])
}

let data = try! JSONSerialization.data(withJSONObject: output)
print(String(data: data, encoding: .utf8)!)
"#;

fn ocr_binary_path() -> PathBuf {
    let dir = std::env::temp_dir().join("cu-ocr-helper");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("ocr")
}

fn ensure_ocr_binary() -> Result<PathBuf, String> {
    let bin = ocr_binary_path();
    if bin.exists() {
        return Ok(bin);
    }

    let src = bin.with_extension("swift");
    std::fs::write(&src, SWIFT_OCR_SRC).map_err(|e| format!("failed to write OCR helper: {e}"))?;

    let output = Command::new("xcrun")
        .args([
            "swiftc",
            "-O",
            "-o", bin.to_str().unwrap(),
            src.to_str().unwrap(),
        ])
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("swiftc failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("swiftc compilation failed: {}", stderr.trim()));
    }

    Ok(bin)
}

pub fn recognize(pid: i32) -> OcrResult {
    let win = match screenshot::find_window(pid) {
        Some(w) => w,
        None => return err("no on-screen window found"),
    };

    let tmp_img = format!("/tmp/cu-ocr-{}.png", std::process::id());
    if let Err(e) = screenshot::capture_window(win.window_id, &tmp_img) {
        return err(&format!("capture failed: {e}"));
    }

    let bin = match ensure_ocr_binary() {
        Ok(b) => b,
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_img);
            return err(&e);
        }
    };

    let output = Command::new(&bin)
        .arg(&tmp_img)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output();

    let _ = std::fs::remove_file(&tmp_img);

    let output = match output {
        Ok(o) => o,
        Err(e) => return err(&format!("OCR helper failed: {e}")),
    };

    if !output.status.success() {
        return err(&format!(
            "OCR failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: Vec<serde_json::Value> = match serde_json::from_str(stdout.trim()) {
        Ok(v) => v,
        Err(e) => return err(&format!("parse error: {e}")),
    };

    let texts = parsed
        .iter()
        .map(|item| {
            let nx = item["x"].as_f64().unwrap_or(0.0);
            let ny = item["y"].as_f64().unwrap_or(0.0);
            let nw = item["w"].as_f64().unwrap_or(0.0);
            let nh = item["h"].as_f64().unwrap_or(0.0);

            OcrText {
                text: item["t"].as_str().unwrap_or("").to_string(),
                confidence: item["c"].as_f64().unwrap_or(0.0),
                x: (win.x + nx * win.width).round(),
                y: (win.y + (1.0 - ny - nh) * win.height).round(),
                width: (nw * win.width).round(),
                height: (nh * win.height).round(),
            }
        })
        .collect();

    OcrResult { ok: true, texts, error: None }
}

fn err(msg: &str) -> OcrResult {
    OcrResult { ok: false, texts: vec![], error: Some(msg.to_string()) }
}
