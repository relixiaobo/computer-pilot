//! Default reference-image rendering from SoftwareCursorGlyphRenderer.swift,
//! c6ffbce2. The image already includes the local fog; no trail is drawn.
use super::motion::Point;
pub(super) const SIZE: f64 = 126.0;
pub(super) const ANCHOR: Point = Point { x: 60.35, y: 70.3 };
pub(super) const IMAGE: &[u8] = include_bytes!("cursor.png");

#[repr(C)]
pub(super) struct Transform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    tx: f64,
    ty: f64,
}
pub(super) fn transform(rotation: f64, body: Point, pulse: f64) -> Transform {
    let compression = (body.length() * 0.008).min(0.018);
    let sx = 1.0 - compression - pulse * 0.03;
    let sy = 1.0 + pulse * 0.03 * 0.4;
    Transform {
        a: rotation.cos() * sx,
        b: rotation.sin() * sx,
        c: -rotation.sin() * sy,
        d: rotation.cos() * sy,
        tx: body.x,
        ty: body.y,
    }
}
