# Visual And OCR Workflows

Use this reference when AX is sparse, exact visual state matters, or a
vision-capable Agent must inspect the real UI.

## Choose A Visual Surface

- Use `cu state <app>` for tree plus a plain window image.
- Use `cu snapshot <app> --with-screenshot` for a fresh tree/image pair.
- Use `cu snapshot <app> --annotated` to draw each current ref on the image.
- Use `cu screenshot --region` for a small verification image.
- Use `cu ocr <app>` when visible text is missing from AX.

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

Prefer `cu nearest <screen_x> <screen_y> --app <app>` to obtain an AX candidate
before clicking. Inspect `inside` and `distance`.

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

## Capture Protection

When `kCGWindowSharingState=0`, Computer Pilot returns `capture_protected` or a
`screenshot_error` string instead of a blank image. The OS restriction cannot
be bypassed. Continue with `snapshot`, `find`, `set-value`, `perform`, and
targeted actions; request manual visual confirmation when visual proof is
required.

## File Safety

Paths must be absolute and must not traverse symlinks. Existing files are not
overwritten. Files are atomically published with mode `0600`. Generate a new
path or omit `--output` when a previous artifact already exists.
