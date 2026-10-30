# Virtual cursor research and implementation boundary

The Codex Computer Use experience separates two things that are easy to conflate:

1. **Input delivery** sends an action to a target process or window without
   moving the user's hardware pointer or changing the user's foreground app.
2. **Visual feedback** draws a software cursor in a separate, non-interactive
   overlay so the user can see where the agent is acting.

The third-party reference implementation is
[`iFurySt/open-codex-computer-use`](https://github.com/iFurySt/open-codex-computer-use).
Its [`InputSimulation.swift`](https://github.com/iFurySt/open-codex-computer-use/blob/c6ffbce2fc49b2a6a42b7c7acac31014efc6b1e2/packages/OpenComputerUseKit/Sources/OpenComputerUseKit/InputSimulation.swift)
uses `CGEvent.postToPid` with a combined-session event source for targeted click,
keyboard, scroll, and drag paths. Its global pointer path is explicitly gated
because it can move the real pointer and change foreground focus. For difficult
Chromium surfaces it also has an explicit SkyLight path, rather than silently
falling back to global HID input.

Its [`SoftwareCursorOverlay.swift`](https://github.com/iFurySt/open-codex-computer-use/blob/c6ffbce2fc49b2a6a42b7c7acac31014efc6b1e2/packages/OpenComputerUseKit/Sources/OpenComputerUseKit/SoftwareCursorOverlay.swift)
implements the visible part as an `NSPanel`:

- borderless, transparent, non-activating, and `ignoresMouseEvents = true`;
- available across Spaces and full-screen windows;
- ordered relative to the target `CGWindowID`, with a liveness check before
  reordering;
- anchored to the target window's local point so a moving window does not leave
  the visual cursor behind;
- animated on the main AppKit run loop, then hidden after a short idle period;
- reset at the end of a turn, while the last position is preserved between
  nearby actions.

The cursor is therefore a presentation layer. It is not the system cursor and it
does not make a global mouse move. The reference service also does not show the
cursor for every tool: keyboard typing and key presses use direct targeted input;
the visible cursor is mainly tied to pointer-like actions and value operations.
The open implementation's service code calls `moveVisualCursor`, `settleVisualCursor`,
and `pulseVisualCursor` around the action, while the actual input path remains
independent.

The reference also explicitly reconstructs the original Codex cursor's appearance
and motion; it is not an official OpenAI UI specification.
[`SoftwareCursorGlyphRenderer.swift`](https://github.com/iFurySt/open-codex-computer-use/blob/c6ffbce2fc49b2a6a42b7c7acac31014efc6b1e2/packages/OpenComputerUseKit/Sources/OpenComputerUseKit/SoftwareCursorGlyphRenderer.swift)
draws the pointer, while
[`CursorMotionModel.swift`](https://github.com/iFurySt/open-codex-computer-use/blob/c6ffbce2fc49b2a6a42b7c7acac31014efc6b1e2/packages/OpenComputerUseKit/Sources/OpenComputerUseKit/CursorMotionModel.swift)
and `animateMove` in the overlay advance it through intermediate positions using
curved paths and spring progress. These paths describe movement, not visible
trail lines. Its
[motion alignment notes](https://github.com/iFurySt/open-codex-computer-use/blob/c6ffbce2fc49b2a6a42b7c7acac31014efc6b1e2/docs/exec-plans/completed/20260419-overlay-official-cursor-motion-alignment.md)
document the reconstruction and its uncertainties.

## Why `cu` felt disruptive

Before this change, omitting `--app` selected the global HID path for coordinate
clicks, OCR clicks, scroll, hover, drag, typing, and key presses. That path is
inherently coupled to the user's desktop: it can warp the hardware pointer,
deliver to whichever window is currently frontmost, and trigger focus changes.
The existing PID implementation only helped when the agent remembered to pass
`--app`, so a normal model omission reintroduced the worst behavior.

`cu` now resolves an omitted selector to the exact current frontmost PID and uses
PID-targeted delivery. `--allow-global` is the explicit opt-in for desktop-wide
HID input. The terminal/IDE guard remains for selector-less input because those
applications turn stray text and shortcuts into commands or destructive edits.

## Reference revision and adopted behavior

Source reading and parity checks use commit
[`c6ffbce2fc49b2a6a42b7c7acac31014efc6b1e2`](https://github.com/iFurySt/open-codex-computer-use/tree/c6ffbce2fc49b2a6a42b7c7acac31014efc6b1e2).
The implementation follows the reconstruction requested for this cursor:

| Area | Upstream source and evidence | `cu` implementation |
| --- | --- | --- |
| Appearance | `SoftwareCursorGlyphRenderer.draw` first loads `official-software-cursor-window-252.png`; its procedural contour is only a fallback and produces a visibly different shape | Embed that same 252 px reference asset, draw at 126 pt with anchor `(60.35, 70.3)`. Warm gray arrow, light outline, local fog, no trail. No runtime asset lookup. |
| Geometry | `OfficialCursorMotionModel`: two base candidates plus `3 × 3 × 2` two-segment cubic candidates | Port all 20 candidates, ray clipping, 24 samples per segment, five measurement terms, in-bounds preference and score. |
| Timing | `CursorMotionProgressAnimator`: Velocity Verlet, response `1.4`, damping `0.9`, timestep `1/240`; stiffness `min((2π/response)², 28800)`, drag `2*damping*sqrt(stiffness)` | Preserve step ordering and close-enough gate: progress >= 1 and distance <= 0.01. Reaches this gate at about 1.429167 s; clamp path sampling to its endpoint. |
| Pose | `CursorVisualDynamicsConfiguration.officialInspired` | Port tip spring `(0.18, 0.76)` and angle spring `(0.24, 0.82)`, velocity-derived heading and bounded body lag. These are upstream tuning, not binary-confirmed original parameters. |
| Click | `animateClickPulse` and `drawReferenceImage` | 160 ms sine pulse, 3% horizontal compression and 1.2% vertical stretch; motion compression capped at 1.8%. |

Important distinctions found by reading the actual source:

- Current upstream `SoftwareCursorOverlay.bestMotionCandidate` uses the newer
  **HeadingDrivenCursorMotionModel**, not `OfficialCursorMotionModel`. This port
  deliberately uses the latter's binary-backed reconstruction; it does not claim
  to copy every behavior of the latest upstream runtime.
- The recovered guide coefficients are confirmed by upstream's binary analysis,
  but their projection into a path-local coordinate basis is explicitly an
  inference. The exact original animation scheduling/finished predicate is not
  fully recovered. The 1.429167 s runtime duration follows the current Swift
  implementation, not a claim of independently verified official wall-clock timing.
- The default image renderer already contains the fog. It does not draw the
  separate procedural fog layers, even though its pose model computes fog fields.
  `cu` likewise omits those unused layers and parameters.
- Core Animation draws the embedded image, with the same centered affine
  transform as the reference's `drawReferenceImage`. The production layer is
  rasterized in memory in a test; its output was visually compared with the source
  image during implementation.

## Integration boundary

The cursor remains optional (`COMPUTER_PILOT_VISUAL_CURSOR=1`), click-through,
capture-protected and owned by the private Broker. AppKit's event loop renders
intermediate frames independently of input delivery. It never moves the real
pointer or activates an app.

Successive events in the same window start from the currently displayed pose;
retargeting preserves pose velocity. A first event or a different target window
establishes a new position directly, rather than inventing a route from the
user's pointer. Screen bounds are taken from the destination display. Window
ordering is reasserted during visible ticks so user activation does not hide the
panel. The existing 12-second idle hide remains. The AppKit event pump continues while
hidden so deferred window operations can finish. Fixed view geometry is
initialized once; each frame updates only panel position and glyph transform.

This is presentation feedback after dispatch, not upstream's synchronous
move-before-click choreography. A click pulse follows visual travel, while
actual PID/AX input does not wait for animation. The target's window-local point
is resolved when an event arrives; continuous window-follow, window-hit scoring,
idle wobble and cross-display path bounds union are not part of this port.

## Verification and provenance

`tests/fixtures/visual-cursor-reference.json` was generated by compiling and
executing the unmodified upstream `CursorMotionModel.swift` at the pinned commit.
Rust tests compare selected candidates, scores, sampled points for seven routes
(including reversed, diagonal, edge, short and zero-length moves), and spring
progress samples. `tests/fixtures/generate-cursor-reference.swift` regenerates the
fixture when compiled alongside that source as `main.swift`.

The live command test samples the real overlay window: intermediate positions,
nonlinear path, motion still underway after 0.5 s, constant panel dimensions and
arrival at the requested coordinate, idle hide and reappearance on the next event.
It also checks screenshot exclusion and
hardware-pointer isolation. The reference image and ported code are attributed
in `THIRD_PARTY_NOTICES.md`, shipped in the binary archive and beside the raw
binary release asset.
