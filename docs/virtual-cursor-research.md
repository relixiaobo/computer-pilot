# Virtual cursor research and implementation boundary

The Codex Computer Use experience separates two things that are easy to conflate:

1. **Input delivery** sends an action to a target process or window without
   moving the user's hardware pointer or changing the user's foreground app.
2. **Visual feedback** draws a software cursor in a separate, non-interactive
   overlay so the user can see where the agent is acting.

The public reference implementation is
[`iFurySt/open-codex-computer-use`](https://github.com/iFurySt/open-codex-computer-use).
Its [`InputSimulation.swift`](https://github.com/iFurySt/open-codex-computer-use/blob/main/packages/OpenComputerUseKit/Sources/OpenComputerUseKit/InputSimulation.swift)
uses `CGEvent.postToPid` with a combined-session event source for targeted click,
keyboard, scroll, and drag paths. Its global pointer path is explicitly gated
because it can move the real pointer and change foreground focus. For difficult
Chromium surfaces it also has an explicit SkyLight path, rather than silently
falling back to global HID input.

Its [`SoftwareCursorOverlay.swift`](https://github.com/iFurySt/open-codex-computer-use/blob/main/packages/OpenComputerUseKit/Sources/OpenComputerUseKit/SoftwareCursorOverlay.swift)
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

## Implemented smallest path to a matching experience

The implementation is staged so the visual layer does not become a second
input system:

1. Keep the current targeted input and use one optional visual-cursor owner in
   the long-lived private Broker. A short-lived CLI process cannot reliably
   host an AppKit animation loop.
2. Send only a private cursor event to that owner: target PID, target window
   ID, screen point, window-local point, and pulse state. Do not send raw mouse
   events through the overlay.
3. Keep the overlay opt-in with `COMPUTER_PILOT_VISUAL_CURSOR=1` and fail-open:
   if the panel cannot be shown, input still runs.
4. Use a simple bounded cursor with a short click pulse and idle hide. Spring
   motion, Fog mode, synthetic focus, and SkyLight remain outside this change.
5. Do not bake the cursor into screenshots returned to the agent. It is for the
   human observer; the agent continues receiving the real target state.

The macOS UI owner lives inside the private Broker. The event is private and
presentation-only; it is not another public command family or action fallback.
