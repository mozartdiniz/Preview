# Preview

A fast, lightweight image viewer for Linux inspired by Apple Preview.
Built with **Rust + GTK4 + libadwaita** for a native GNOME experience.

---

## Feature Checklist

### Viewing
- [x] Open common image formats: PNG, JPEG, GIF, BMP, TIFF, WebP, SVG
- [ ] Open HEIC / AVIF images
- [ ] Multi-page document support (TIFF stacks)
- [x] Fit to window
- [x] Actual size (100%)
- [x] Zoom in / Zoom out
- [ ] Zoom to selection
- [x] Scroll when zoomed in
- [x] Mouse wheel zoom (Ctrl + scroll)
- [ ] Pinch-to-zoom (touchpad gesture)
- [ ] Smooth zoom animations
- [ ] Rotate view without modifying file

### Navigation
- [x] Open file via dialog
- [x] Open file from command line argument
- [x] Drag and drop to open image
- [ ] Previous / Next image in same directory
- [ ] Thumbnail strip / sidebar
- [ ] Slideshow mode
- [x] Fullscreen mode (F11)
- [ ] Open recent files

### File Management
- [ ] Save (overwrite original)
- [ ] Save As (with format conversion: PNG, JPEG, WebP, TIFF, BMP)
- [ ] Export to different format
- [ ] Print
- [ ] Copy image to clipboard
- [ ] Paste image from clipboard

### Image Adjustments
- [ ] Crop (interactive drag selection)
- [ ] Resize (width × height with aspect ratio lock)
- [ ] Rotate 90° clockwise
- [ ] Rotate 90° counter-clockwise
- [ ] Rotate arbitrary angle
- [ ] Flip horizontal
- [ ] Flip vertical
- [ ] Adjust brightness / exposure
- [ ] Adjust contrast
- [ ] Adjust saturation / vibrance
- [ ] Sharpen
- [ ] Gaussian blur
- [ ] Auto-enhance (auto levels)
- [ ] Revert to original (undo all edits)
- [ ] Non-destructive edit history

### Annotations & Markup
- [ ] Rectangle selection tool
- [ ] Freehand draw (pencil)
- [ ] Arrow tool
- [ ] Line tool
- [ ] Rectangle shape
- [ ] Circle / ellipse shape
- [ ] Polygon shape
- [ ] Text label (font, size, style)
- [ ] Highlight / marker tool
- [ ] Loupe / magnify tool
- [ ] Change annotation stroke color (color picker)
- [ ] Change annotation fill color
- [ ] Change line thickness
- [ ] Change opacity
- [ ] Undo / Redo annotations (Ctrl+Z / Ctrl+Shift+Z)
- [ ] Save annotations embedded into file
- [ ] Clear all annotations

### Metadata & Info
- [ ] View EXIF / IPTC / XMP metadata panel
- [ ] View ICC color profile info
- [ ] Image info bar: file size, dimensions, bit depth, color mode
- [ ] GPS location from EXIF on a map

### UI / UX
- [x] Native GNOME design with libadwaita
- [x] Status bar with image dimensions and filename
- [x] Zoom level indicator
- [x] Keyboard shortcuts (Ctrl+O, +, -, 1, 3, F11)
- [ ] Dark / light theme toggle
- [ ] Remember window size and position across sessions
- [ ] Preferences dialog (default zoom, background color)
- [ ] Adaptive layout (works on small screens / mobile)

---

## Keyboard Shortcuts

| Action           | Shortcut               |
|-----------------|------------------------|
| Open file        | `Ctrl+O`               |
| Zoom in          | `+` / `=`              |
| Zoom out         | `-`                    |
| Actual size      | `1`                    |
| Zoom to fit      | `3`                    |
| Fullscreen       | `F11`                  |
| Quit             | `Ctrl+Q`               |

---

## Building

```bash
cargo build --release
./target/release/Preview [image_file]
```

### Dependencies (system)

```bash
# Fedora / RHEL
sudo dnf install gtk4-devel libadwaita-devel

# Ubuntu / Debian
sudo apt install libgtk-4-dev libadwaita-1-dev
```
