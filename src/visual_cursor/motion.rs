//! Port of the binary-backed reconstruction in open-codex-computer-use
//! c6ffbce2, CursorMotionModel.swift (OfficialCursorMotionModel, not its later
//! HeadingDriven model). See THIRD_PARTY_NOTICES.md and the research note.
use std::f64::consts::{PI, TAU};
use std::ops::{Add, Mul, Sub};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Point {
    pub x: f64,
    pub y: f64,
}
impl Add for Point {
    type Output = Self;
    fn add(self, p: Self) -> Self {
        Self {
            x: self.x + p.x,
            y: self.y + p.y,
        }
    }
}
impl Sub for Point {
    type Output = Self;
    fn sub(self, p: Self) -> Self {
        Self {
            x: self.x - p.x,
            y: self.y - p.y,
        }
    }
}
impl Mul<f64> for Point {
    type Output = Self;
    fn mul(self, n: f64) -> Self {
        Self {
            x: self.x * n,
            y: self.y * n,
        }
    }
}
impl Point {
    pub fn length(self) -> f64 {
        self.x.hypot(self.y)
    }
    fn normalized(self) -> Self {
        self * (1.0 / self.length().max(0.001))
    }
    fn normal(self) -> Self {
        Self {
            x: -self.y,
            y: self.x,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Bounds {
    pub min: Point,
    pub max: Point,
}
impl Bounds {
    fn contains(self, p: Point) -> bool {
        p.x >= self.min.x - 20.0
            && p.x < self.max.x + 20.0
            && p.y >= self.min.y - 20.0
            && p.y < self.max.y + 20.0
    }
    fn ray(self, p: Point, d: Point) -> f64 {
        let mut limit = f64::INFINITY;
        for (origin, direction, low, high) in [
            (p.x, d.x, self.min.x, self.max.x),
            (p.y, d.y, self.min.y, self.max.y),
        ] {
            if direction > 0.0 {
                limit = limit.min((high - origin) / direction);
            } else if direction < 0.0 {
                limit = limit.min((low - origin) / direction);
            }
        }
        limit.max(0.0)
    }
}
#[derive(Clone, Debug)]
struct Segment {
    end: Point,
    c1: Point,
    c2: Point,
}
#[derive(Clone, Debug)]
pub(super) struct Path {
    start: Point,
    segments: Vec<Segment>,
}
impl Path {
    pub fn sample(&self, progress: f64) -> Point {
        let scaled = progress.clamp(0.0, 1.0) * self.segments.len() as f64;
        let i = (scaled as usize).min(self.segments.len() - 1);
        let t = scaled - i as f64;
        let s = &self.segments[i];
        let start = if i == 0 {
            self.start
        } else {
            self.segments[i - 1].end
        };
        let u = 1.0 - t;
        start * (u * u * u)
            + s.c1 * (3.0 * u * u * t)
            + s.c2 * (3.0 * u * t * t)
            + s.end * (t * t * t)
    }
    fn measure(&self, bounds: Bounds) -> (f64, bool) {
        let mut length = 0.0;
        let mut energy = 0.0;
        let mut max_angle: f64 = 0.0;
        let mut total_turn = 0.0;
        let mut inside = bounds.contains(self.start);
        let mut previous = self.start;
        let mut angle: Option<f64> = None;
        let steps = self.segments.len() * 24;
        for i in 1..=steps {
            let p = self.sample(i as f64 / steps as f64);
            inside &= bounds.contains(p);
            let delta = p - previous;
            let distance = delta.length();
            if distance > 0.01 {
                length += distance;
                let next_angle = delta.y.atan2(delta.x);
                if let Some(old) = angle {
                    let change = normalize_angle(next_angle - old);
                    energy += change * change;
                    max_angle = max_angle.max(change.abs());
                    total_turn += change.abs();
                }
                angle = Some(next_angle);
                previous = p;
            }
        }
        let direct = (self.sample(1.0) - self.start).length().max(1.0);
        let score = ((length / direct) - 1.0).max(0.0) * 320.0
            + energy * 140.0
            + max_angle * 180.0
            + total_turn * 18.0
            + if inside { 0.0 } else { 45.0 };
        (score, inside)
    }
}
struct Candidate {
    id: String,
    path: Path,
    score: f64,
    inside: bool,
}
fn candidates(start: Point, end: Point, bounds: Bounds) -> Vec<Candidate> {
    let distance = (end - start).length().max(0.001);
    let direction = (end - start).normalized();
    let normal = direction.normal();
    // The coefficients are binary-backed; projecting them into this local basis
    // is explicitly identified as reconstruction/inference by upstream.
    let guide = direction * -0.6946583704589973 + normal * 0.7193398003386512;
    let primary = distance * 0.41960295031576633;
    let (start_extent, end_extent) = if primary < 48.0 {
        (48.0, 48.0)
    } else if primary < 640.0 {
        (primary, distance * 0.9)
    } else if distance * 0.15 < 640.0 {
        (640.0, 48.0)
    } else {
        (640.0, 640.0)
    };
    let start_extent = start_extent.min(bounds.ray(start, guide));
    let end_extent = end_extent.min(bounds.ray(end, guide * -1.0));
    let full_start = start + guide * start_extent;
    let full_end = end - guide * end_extent;
    let raw_handle = distance * 0.2765523188064277;
    let handle = if raw_handle < 50.0 {
        50.0
    } else if raw_handle < 640.0 {
        raw_handle
    } else {
        520.0
    };
    let arc = (distance * 0.5783555327868779).clamp(38.0, 440.0);
    let signed_normal = normal
        * if guide.y * direction.x - guide.x * direction.y < 0.0 {
            -1.0
        } else {
            1.0
        };
    let forward = direction * distance + signed_normal * arc;
    let forward = if forward.length() < handle {
        Point { x: 1.0, y: 0.0 }
    } else {
        forward.normalized()
    };
    let mut result = Vec::with_capacity(20);
    let mut add = |id: String, segments: Vec<Segment>| {
        let path = Path { start, segments };
        let (score, inside) = path.measure(bounds);
        result.push(Candidate {
            id,
            path,
            score,
            inside,
        });
    };
    add(
        "base-full-guide".into(),
        vec![Segment {
            end,
            c1: full_start,
            c2: full_end,
        }],
    );
    add(
        "base-scaled-guide".into(),
        vec![Segment {
            end,
            c1: start + guide * (start_extent * 0.65),
            c2: end - guide * (end_extent * 0.65),
        }],
    );
    for a in [0.55, 0.8, 1.05] {
        for b in [0.65, 1.0, 1.35] {
            for side in [1.0, -1.0] {
                let anchor = (start + end) * 0.5
                    + guide * (start_extent * 0.65)
                    + signed_normal * (handle * a * side);
                let span = forward * (arc * b);
                add(
                    format!(
                        "a{a:.2}-b{b:.2}-{}",
                        if side > 0.0 { "positive" } else { "negative" }
                    ),
                    vec![
                        Segment {
                            end: anchor,
                            c1: full_start,
                            c2: anchor - span,
                        },
                        Segment {
                            end,
                            c1: anchor + span,
                            c2: full_end,
                        },
                    ],
                );
            }
        }
    }
    result
}
fn best(start: Point, end: Point, bounds: Bounds) -> Candidate {
    candidates(start, end, bounds)
        .into_iter()
        .min_by(|a, b| {
            b.inside
                .cmp(&a.inside)
                .then_with(|| a.score.total_cmp(&b.score))
                .then_with(|| a.id.cmp(&b.id))
        })
        .expect("twenty candidates")
}
pub(super) fn select_path(start: Point, end: Point, bounds: Bounds) -> Path {
    best(start, end, bounds).path
}

pub(super) const DT: f64 = 1.0 / 240.0;
#[derive(Default)]
pub(super) struct Spring {
    pub time: f64,
    pub value: f64,
    velocity: f64,
    force: f64,
}
impl Spring {
    fn step(&mut self, target: f64, response: f64, damping: f64) {
        let stiffness = (TAU / response).powi(2).min(28_800.0);
        let drag = 2.0 * damping * stiffness.sqrt();
        let half_velocity = self.velocity + self.force * DT * 0.5;
        self.value += half_velocity * DT;
        self.force = stiffness * (target - self.value) - drag * half_velocity;
        self.velocity = half_velocity + self.force * DT * 0.5;
        self.time += DT;
    }
    pub fn advance(&mut self, time: f64, target: f64, response: f64, damping: f64) {
        if time - self.time > 1.0 {
            self.time = time - 1.0 / 60.0;
        }
        while self.time < time {
            self.step(target, response, damping);
        }
    }
    pub fn close_enough(&self) -> bool {
        self.value >= 1.0 && (1.0 - self.value).abs() <= 0.01
    }
}
// Fixed recovered parameters reach closeEnough after 343 integration steps.
// The regression test independently advances the spring to verify this duration.
pub(super) const TRAVEL_TIME: f64 = 343.0 * DT;
pub(super) fn normalize_angle(angle: f64) -> f64 {
    (angle + PI).rem_euclid(TAU) - PI
}

// These pose dynamics are upstream's `officialInspired` tuning, NOT recovered
// binary constants. Runtime coordinates are AppKit y-up; drawing rotation is
// converted from the upstream y-down screen state.
pub(super) struct Pose {
    x: Spring,
    y: Spring,
    angle: Spring,
}
pub(super) struct RenderPose {
    pub tip: Point,
    pub rotation: f64,
    pub body: Point,
}
impl Pose {
    pub fn new(tip: Point, time: f64) -> Self {
        Self {
            x: Spring {
                value: tip.x,
                time,
                ..Default::default()
            },
            y: Spring {
                value: tip.y,
                time,
                ..Default::default()
            },
            angle: Spring {
                time,
                ..Default::default()
            },
        }
    }
    pub fn tip(&self) -> Point {
        Point {
            x: self.x.value,
            y: self.y.value,
        }
    }
    pub fn advance(&mut self, target: Point, time: f64) -> RenderPose {
        if time - self.x.time > 1.0 {
            self.x.time = time - 1.0 / 60.0;
            self.y.time = self.x.time;
            self.angle.time = self.x.time;
        }
        while self.x.time < time {
            self.x.step(target.x, 0.18, 0.76);
            self.y.step(target.y, 0.18, 0.76);
            let velocity = Point {
                x: self.x.velocity,
                y: -self.y.velocity,
            };
            let target_angle = if velocity.length() > 14.0 {
                normalize_angle(velocity.y.atan2(velocity.x) + 3.0 * PI / 4.0)
            } else {
                0.0
            };
            let unwrapped = self.angle.value + normalize_angle(target_angle - self.angle.value);
            self.angle.step(unwrapped, 0.24, 0.82);
            self.angle.value = normalize_angle(self.angle.value);
        }
        let velocity = Point {
            x: self.x.velocity,
            y: -self.y.velocity,
        };
        let speed = velocity.length();
        let direction = if speed > 0.001 {
            velocity.normalized()
        } else {
            Point {
                x: (-3.0 * PI / 4.0).cos(),
                y: (-3.0 * PI / 4.0).sin(),
            }
        };
        let lateral = direction.normal() * (self.angle.velocity * 0.06).clamp(-1.4, 1.4);
        let body = direction * -(speed * 0.0012).min(2.4) + lateral;
        RenderPose {
            tip: self.tip(),
            rotation: -self.angle.value,
            body: Point {
                x: body.x,
                y: -body.y,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_reference_selects_same_curve_and_endpoint() {
        let start = Point { x: 100.0, y: 120.0 };
        let end = Point { x: 720.0, y: 380.0 };
        let bounds = Bounds {
            min: Point::default(),
            max: Point {
                x: 1280.0,
                y: 800.0,
            },
        };
        assert_eq!(candidates(start, end, bounds).len(), 20);
        let choice = best(start, end, bounds);
        assert_eq!(choice.id, "a1.05-b1.00-positive");
        assert_eq!(choice.path.sample(0.0), start);
        assert_eq!(choice.path.sample(1.1), end);
        assert!(choice.inside);
        assert!((choice.path.sample(0.5) - (start + end) * 0.5).length() > 20.0);
    }
    #[test]
    fn recovered_spring_reaches_endpoint_at_upstream_time() {
        assert!((TRAVEL_TIME - 1.429166666666663).abs() < 1e-10);
        let mut s = Spring::default();
        for step in 1..=343 {
            s.step(1.0, 1.4, 0.9);
            assert_eq!(s.close_enough(), step == 343);
        }
        assert!(s.close_enough());
        assert!(s.velocity > 0.0); // Geometry locks before the spring is at rest.
    }
    #[test]
    fn matches_executed_upstream_swift_fixtures() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/visual-cursor-reference.json"
        ))
        .unwrap();
        let point = |v: &serde_json::Value| Point {
            x: v[0].as_f64().unwrap(),
            y: v[1].as_f64().unwrap(),
        };
        let bounds = Bounds {
            min: Point::default(),
            max: Point {
                x: 1280.0,
                y: 800.0,
            },
        };
        for case in fixture["paths"].as_array().unwrap() {
            let chosen = best(point(&case["start"]), point(&case["end"]), bounds);
            assert_eq!(chosen.id, case["id"].as_str().unwrap());
            assert!((chosen.score - case["score"].as_f64().unwrap()).abs() < 1e-8);
            for (i, t) in [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0]
                .into_iter()
                .enumerate()
            {
                assert!((chosen.path.sample(t) - point(&case["points"][i])).length() < 1e-8);
            }
        }
        let mut spring = Spring::default();
        for sample in fixture["spring"].as_array().unwrap() {
            spring.advance(sample[0].as_f64().unwrap(), 1.0, 1.4, 0.9);
            assert!((spring.value - sample[1].as_f64().unwrap()).abs() < 1e-12);
        }
    }

    #[test]
    fn curve_stays_finite_in_all_directions_and_at_edges() {
        let bounds = Bounds {
            min: Point::default(),
            max: Point {
                x: 1280.0,
                y: 800.0,
            },
        };
        for start in [
            Point { x: 0.0, y: 0.0 },
            Point {
                x: 1279.0,
                y: 799.0,
            },
            Point { x: 640.0, y: 400.0 },
        ] {
            for end in [
                Point { x: 1.0, y: 799.0 },
                Point { x: 1279.0, y: 1.0 },
                start,
            ] {
                let path = select_path(start, end, bounds);
                for i in 0..=100 {
                    let p = path.sample(i as f64 / 100.0);
                    assert!(p.x.is_finite() && p.y.is_finite());
                }
                assert!((path.sample(1.0) - end).length() < 1e-8);
            }
        }
    }
}
