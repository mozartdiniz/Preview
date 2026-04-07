# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build              # debug build
cargo build --release    # release build
cargo run                # run (no args = empty window)
cargo run -- path/to/img # open image on launch
cargo clippy             # lint
```

No test suite exists yet.

## Architecture

GTK4/libadwaita image viewer and editor written in Rust. Single binary, single window, no async runtime.

### Startup flow (`main.rs`)

`build_ui` creates three things and wires them together:

1. **`State`** — `Rc<RefCell<State>>` shared across all closures. Single source of truth for zoom, image data, annotations, crop rubber-band, text draft, undo/redo stacks, and various drag/mode flags.

2. **`Widgets`** — plain struct of GTK widget handles built in `widgets::build`. No logic, just references.

3. **`Closures`** — `Rc<dyn Fn(...)>` wrappers built in `image_closures` and `text_closures`. They capture `state` + `widgets` and expose named operations (`apply_zoom`, `update_image`, `load_image_file`, `set_text_mode`, `commit_draft`, `undo`, `redo`, …). Passing `Closures` avoids re-cloning state into every callback directly.

Then four wiring calls:
- `text_gesture::setup_text_click/drag` — GestureClick/GestureDrag on canvas for text placement, move, rotation
- `crop_gesture::setup` — GestureDrag on canvas for crop rubber-band
- `callbacks::connect` — button signals (toolbar buttons → closures/transforms)
- `text_callbacks::connect` — text-tool-bar widget signals (font, color, rotation, draft entry)
- `actions::setup` — `gio::SimpleAction`s + keyboard accelerators

### State and coordinate spaces

Two coordinate spaces:
- **Widget space** — pixel position within the `DrawingArea` widget
- **Image space** — pixel position within the `DynamicImage`

`coords::fit_transform` returns `(offset_x, offset_y, scale)` for contain-fit mode. `coords::widget_to_img` converts widget → image coords. The draw function applies the same transform via `cr.translate(ox,oy); cr.scale(scale,scale)`.

Fit mode (`state.fit_mode = true`) centers and scales to fill the viewport. Zoom mode uses `state.zoom` as the scale with no centering offset.

### Rendering pipeline (`draw.rs`, `annotation.rs`)

The `DrawingArea` draw func (set in `draw::setup`) reads from `state.surface` — a `cairo::ImageSurface` in premultiplied BGRA (Cairo ARgb32 format). When the image changes, `update_image` calls `annotation::to_cairo_surface` to rebuild this surface.

Annotations are `TextAnnotation` structs (image-space coords, pango `FontDescription`, RGBA color, rotation in radians). They are drawn live over the surface each frame via `annotation::draw_text_annotation`. On save/export, `annotation::flatten_annotations` renders them into a new `DynamicImage` using Cairo, then `export::save_image` writes the result.

The draft annotation (currently being typed) is stored separately in `state.draft_text` / `state.draft_pos` and rendered inline with a blinking cursor.

### Undo/redo

`state.push_undo()` snapshots `(image, img_width, img_height, annotations)` onto `undo_stack` (max 20). `redo_stack` is cleared on every push. Undo/redo closures in `image_closures` swap between stacks and call `update_image`.

`state.syncing_ui` suppresses undo pushes during programmatic UI updates (e.g. loading a new file, rotation drag). `state.property_undo_pushed` prevents multiple undo entries for continuous property edits (font, color, rotation) on the same annotation.

### Text tool flow

1. Click canvas → `text_gesture::setup_text_click` detects hit (`hit_test::find_annotation`) — selects existing annotation or places new draft at clicked image coords.
2. Keystrokes go to the hidden `draft_entry` widget; `text_callbacks` mirrors changes into `state.draft_text` and queues a canvas redraw.
3. `commit_draft` moves `draft_text` into `state.annotations`, clears draft state, pushes undo.
4. Drag on selected annotation → move (updates `ann.x`/`ann.y`). Drag on corner handles → rotation (updates `ann.rotation`).

### Module summary

| Module | Responsibility |
|---|---|
| `state` | `State` struct + `push_undo` |
| `widgets` | build all GTK widgets, return `Widgets` struct |
| `draw` | `DrawingArea` draw function |
| `annotation` | `TextAnnotation`, draw/flatten, Cairo↔image conversion |
| `coords` | fit transform, widget↔image coordinate conversion |
| `transforms` | image ops: resize, rotate, flip, crop (wraps `image` crate) |
| `export` | write `DynamicImage` to file (JPEG quality 92, others via `image::save`) |
| `image_closures` | closures: zoom, load, update, undo, redo |
| `text_closures` | closures: text mode, crop mode, cursor blink, commit draft |
| `callbacks` | connect toolbar button signals |
| `text_callbacks` | connect text-tool-bar widget signals |
| `text_gesture` | GestureClick + GestureDrag for text placement/move/rotation |
| `crop_gesture` | GestureDrag for crop rubber-band |
| `hit_test` | find annotation at widget coordinates |
| `actions` | `gio::SimpleAction`s and keyboard accelerators |
| `dialogs` | file open/save dialogs, error dialog |
