# Visual And OCR Workflows

Use this reference when AX is sparse, exact visual state matters, or a
vision-capable Agent must inspect the real UI.

## Choose A Visual Surface

- Use `cu state <app>` when window metadata is needed along with tree/image.
- Use `cu snapshot <app> --with-screenshot` for a fresh tree/image pair.
- Use `cu snapshot <app> --annotated` to draw each current ref on the image.
- Use `cu screenshot --region` for a small verification image.
- Use `cu ocr <app>` when visible text is missing from AX.

Window capture works in the background. `screenshot --region` and `--full`
capture the desktop surface and are not constrained to `--app`; other windows
can cover the intended content. Use window capture for an obscured target.

Choose the observation that answers the missing question; do not run all of
these in sequence. A visible icon needs an image, not OCR. A known text label
can use `click --text` directly when its region/uniqueness is established;
that command performs OCR itself. Use a current visual target when AX is absent
or demonstrably unusable, not merely because a previous ref became stale.

Set the task output directory first:

```bash
export COMPUTER_PILOT_OUTPUT_DIR="/absolute/task-output"
cu snapshot Mail --limit 80 --annotated
```

The response includes an absolute legacy path and structured file metadata:

```json
{
  "annotated_screenshot_file": {
    "path": "/absolute/task-output/cu-annotated-....png",
    "mime": "image/png",
    "bytes": 201337,
    "width": 1440,
    "height": 900,
    "scale": 2.0
  }
}
```

Read that path with the host Agent's normal image tool.

## Coordinate Mapping

AX and region arguments use screen points. PNG dimensions use pixels. Convert
an image pixel back to screen coordinates with:

```text
screen_x = pixel_x / scale + offset_x
screen_y = pixel_y / scale + offset_y
```

Reuse a matching ref already returned with the image. Use `cu nearest` only
when an AX candidate still needs to be located; inspect `inside`, `distance`,
and identity. It adds no value when AX cannot expose the target. For an observed
visual-only target, use `cu click <screen_x> <screen_y> --app <app>` directly.
For tree/image output use full JSON when geometry is needed. Window image
offsets come from `window_frame.x/y`; standalone screenshot output supplies
`offset_x/offset_y`. Captures and AX reads are sequential, not atomic: observe
again if the layout changes before acting.

Use `cu observe-region` when a dialog, panel, or pane defines the search area:

```bash
cu observe-region 480 200 400 300 --app Mail --mode center
```

## OCR

```bash
cu ocr "System Settings"
cu click --text "Privacy & Security" --app "System Settings"
```

Read `confidence`, aggregate confidence fields, and `confidence_hint`. Restrict
duplicate text with `--index` or `--region`. Verify OCR-driven actions visually
or through a fresh AX observation.

Targeted `type` still requires a focused AX textfield/textarea/combobox. Seeing
a caret in pixels cannot satisfy that precondition for a fully AX-less input.
Use a permitted app-native operation if available, or report that limitation;
do not switch to global typing to bypass it.

## Capture Protection

When `kCGWindowSharingState=0`, Computer Pilot returns `capture_protected` or a
`screenshot_error` string instead of a blank image. The OS restriction cannot
be bypassed. Continue with `snapshot`, `find`, `set-value`, `perform`, and
targeted actions; request manual visual confirmation when visual proof is
required.

## File Safety

For human observation, `COMPUTER_PILOT_VISUAL_CURSOR=1` before the first command
shows an optional click-through software cursor. It does not move the real
pointer and is excluded from agent screenshots.

Paths must be absolute and must not traverse symlinks. Existing files are not
overwritten. Files are atomically published with mode `0600`. Generate a new
path or omit `--output` when a previous artifact already exists.
