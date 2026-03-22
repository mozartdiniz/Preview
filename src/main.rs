mod image_ops;

use gtk4::cairo;
use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};
use image::DynamicImage;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::{Cell, RefCell};
use std::env;
use std::path::PathBuf;
use std::rc::Rc;

const APP_ID: &str = "com.example.Preview";

// ── State ─────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct State {
    // Zoom / display
    zoom: f64,
    fit_mode: bool,
    img_width: i32,
    img_height: i32,
    // Cairo surface used by the draw func (derived from `image`)
    surface: Option<cairo::ImageSurface>,
    // Editable pixel data (None for display-only formats like SVG)
    image: Option<DynamicImage>,
    // Original file path (for Save overwrite)
    file_path: Option<PathBuf>,
    // Crop rubber-band in widget (viewport) coordinates — fit-mode only
    in_crop: bool,
    drag_start: Option<(f64, f64)>,
    drag_end: Option<(f64, f64)>,
    // Text annotation tool
    annotations: Vec<image_ops::TextAnnotation>,
    text_tool_active: bool,
    text_font_desc: Option<gtk4::pango::FontDescription>,
    text_color: (f64, f64, f64, f64),
    text_rotation: f64,  // radians
    // Draft annotation being typed (image-space coords)
    draft_pos: Option<(f64, f64)>,    // top-left of text (updated each keystroke)
    draft_center: Option<(f64, f64)>, // fixed visual centre (set at click/re-edit)
    draft_text: String,
    // Selected annotation index (for drag-to-move)
    selected_ann: Option<usize>,
    move_origin: Option<(f64, f64)>,  // annotation's image coords at drag start
    // Rotation drag
    rotation_drag: bool,
    rotation_drag_anchor: (f64, f64),    // text centre in widget space
    rotation_drag_begin: (f64, f64),     // widget pos where drag started
    rotation_drag_initial_rotation: f64,
    // Copy/paste clipboard
    clipboard: Option<image_ops::TextAnnotation>,
}

impl State {
    fn new() -> Self {
        Self {
            zoom: 1.0,
            fit_mode: true,
            text_color: (1.0, 0.0, 0.0, 1.0),
            ..Default::default()
        }
    }
    fn has_image(&self) -> bool {
        self.img_width > 0
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    app.connect_activate(|app| build_ui(app, None));

    app.connect_open(|app, files, _hint| {
        let path = files.first().and_then(|f: &gio::File| f.path());
        build_ui(app, path.as_deref());
    });

    app.run_with_args(&env::args().collect::<Vec<_>>())
}

// ── UI ────────────────────────────────────────────────────────────────────────

fn build_ui(app: &adw::Application, initial_file: Option<&std::path::Path>) {
    let state = Rc::new(RefCell::new(State::new()));

    // ── Canvas (DrawingArea replaces Picture + Overlay) ───────────────────────
    //
    // set_content_width/height controls the natural size used by the ScrolledWindow
    // for scroll-bar decisions. In zoom mode we set it to img*zoom; in fit mode
    // we set it to 0 and let the widget expand to fill the viewport.
    let canvas = gtk4::DrawingArea::builder()
        .hexpand(true)
        .vexpand(true)
        .build();

    let scrolled = gtk4::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .build();
    let canvas_overlay = gtk4::Overlay::new();
    canvas_overlay.set_child(Some(&canvas));
    scrolled.set_child(Some(&canvas_overlay));

    // ── Status bar ────────────────────────────────────────────────────────────
    let status_label = gtk4::Label::builder()
        .label("Open an image to get started")
        .margin_start(12)
        .margin_top(5)
        .margin_bottom(5)
        .halign(gtk4::Align::Start)
        .hexpand(true)
        .ellipsize(gtk4::pango::EllipsizeMode::Middle)
        .build();

    let zoom_label = gtk4::Label::builder()
        .label("")
        .margin_end(12)
        .margin_top(5)
        .margin_bottom(5)
        .halign(gtk4::Align::End)
        .build();
    zoom_label.add_css_class("dim-label");

    let status_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    status_box.append(&status_label);
    status_box.append(&zoom_label);

    // ── Crop controls bar ─────────────────────────────────────────────────────
    let cancel_crop_btn = gtk4::Button::with_label("Cancel");
    let apply_crop_btn = gtk4::Button::builder()
        .label("Apply Crop")
        .sensitive(false)
        .build();
    apply_crop_btn.add_css_class("suggested-action");

    let crop_hint = gtk4::Label::builder()
        .label("Drag to select the crop area")
        .hexpand(true)
        .halign(gtk4::Align::Center)
        .build();
    crop_hint.add_css_class("dim-label");

    let crop_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    crop_bar.set_visible(false);
    crop_bar.set_margin_start(12);
    crop_bar.set_margin_end(12);
    crop_bar.set_margin_top(6);
    crop_bar.set_margin_bottom(6);
    crop_bar.append(&cancel_crop_btn);
    crop_bar.append(&crop_hint);
    crop_bar.append(&apply_crop_btn);

    // ── Inline text entry (overlaid directly on the canvas) ──────────────────
    // Opacity 0: completely invisible (including focus ring) but still
    // focusable — all visual feedback comes from the canvas preview.
    // can_target=false: pointer/drag events always fall through to the canvas
    // so rotation handles and other gestures keep working.
    let draft_entry = gtk4::Entry::builder()
        .placeholder_text("Type and press Enter…")
        .width_request(1)
        .height_request(1)
        .halign(gtk4::Align::Start)
        .valign(gtk4::Align::Start)
        .visible(false)
        .opacity(0.0)
        .can_target(false)
        .build();

    // ── Text annotation toolbar ───────────────────────────────────────────────
    let font_dialog = gtk4::FontDialog::new();
    let font_btn = gtk4::FontDialogButton::new(Some(font_dialog));
    font_btn.set_font_desc(&gtk4::pango::FontDescription::from_string("Sans 24"));

    let color_dialog = gtk4::ColorDialog::new();
    let color_btn = gtk4::ColorDialogButton::new(Some(color_dialog));
    color_btn.set_rgba(&gdk::RGBA::new(1.0, 0.0, 0.0, 1.0));

    let done_text_btn = gtk4::Button::with_label("Done");
    let text_hint = gtk4::Label::builder()
        .label("Click image to place text — drag to move")
        .hexpand(true)
        .halign(gtk4::Align::Center)
        .build();
    text_hint.add_css_class("dim-label");

    // Rotation spin button (degrees)
    let rotation_adj = gtk4::Adjustment::new(0.0, -180.0, 180.0, 1.0, 15.0, 0.0);
    let rotation_spin = gtk4::SpinButton::new(Some(&rotation_adj), 1.0, 0);
    rotation_spin.set_wrap(true);
    rotation_spin.set_tooltip_text(Some("Rotation (°)"));
    rotation_spin.set_width_chars(5);

    let rotation_label = gtk4::Label::new(Some("°"));

    let text_tool_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    text_tool_bar.set_visible(false);
    text_tool_bar.set_margin_start(12);
    text_tool_bar.set_margin_end(12);
    text_tool_bar.set_margin_top(6);
    text_tool_bar.set_margin_bottom(6);
    text_tool_bar.append(&done_text_btn);
    text_tool_bar.append(&text_hint);
    text_tool_bar.append(&font_btn);
    text_tool_bar.append(&color_btn);
    text_tool_bar.append(&rotation_spin);
    text_tool_bar.append(&rotation_label);

    // ── Header bar ────────────────────────────────────────────────────────────
    let open_btn = gtk4::Button::builder()
        .icon_name("document-open-symbolic")
        .tooltip_text("Open Image (Ctrl+O)")
        .build();

    // Zoom group
    let zoom_out_btn = gtk4::Button::builder()
        .icon_name("zoom-out-symbolic")
        .tooltip_text("Zoom Out  (–)")
        .sensitive(false)
        .build();
    let zoom_fit_btn = gtk4::Button::builder()
        .icon_name("zoom-fit-best-symbolic")
        .tooltip_text("Fit to Window  (3)")
        .sensitive(false)
        .build();
    let zoom_orig_btn = gtk4::Button::builder()
        .icon_name("zoom-original-symbolic")
        .tooltip_text("Actual Size  (1)")
        .sensitive(false)
        .build();
    let zoom_in_btn = gtk4::Button::builder()
        .icon_name("zoom-in-symbolic")
        .tooltip_text("Zoom In  (+)")
        .sensitive(false)
        .build();
    let zoom_group = linked_box(&[
        zoom_out_btn.upcast_ref(),
        zoom_fit_btn.upcast_ref(),
        zoom_orig_btn.upcast_ref(),
        zoom_in_btn.upcast_ref(),
    ]);

    // Edit group
    let resize_btn = gtk4::Button::builder()
        .label("Resize")
        .tooltip_text("Change dimensions")
        .sensitive(false)
        .build();
    let rotate_ccw_btn = gtk4::Button::builder()
        .icon_name("object-rotate-left-symbolic")
        .tooltip_text("Rotate 90° Left")
        .sensitive(false)
        .build();
    let rotate_cw_btn = gtk4::Button::builder()
        .icon_name("object-rotate-right-symbolic")
        .tooltip_text("Rotate 90° Right")
        .sensitive(false)
        .build();
    let flip_h_btn = gtk4::Button::builder()
        .icon_name("object-flip-horizontal-symbolic")
        .tooltip_text("Flip Horizontal")
        .sensitive(false)
        .build();
    let flip_v_btn = gtk4::Button::builder()
        .icon_name("object-flip-vertical-symbolic")
        .tooltip_text("Flip Vertical")
        .sensitive(false)
        .build();
    let crop_btn = gtk4::Button::builder()
        .icon_name("transform-crop-symbolic")
        .tooltip_text("Crop Image")
        .sensitive(false)
        .build();
    let text_btn = gtk4::Button::builder()
        .icon_name("insert-text-symbolic")
        .tooltip_text("Add Text Annotation")
        .sensitive(false)
        .build();
    let edit_group = linked_box(&[
        resize_btn.upcast_ref(),
        rotate_ccw_btn.upcast_ref(),
        rotate_cw_btn.upcast_ref(),
        flip_h_btn.upcast_ref(),
        flip_v_btn.upcast_ref(),
        crop_btn.upcast_ref(),
        text_btn.upcast_ref(),
    ]);

    // Save menu button
    let save_menu = {
        let m = gio::Menu::new();
        let file_sec = gio::Menu::new();
        file_sec.append(Some("Save"), Some("win.save"));
        file_sec.append(Some("Save As…"), Some("win.save-as"));
        m.append_section(None, &file_sec);
        let exp_sec = gio::Menu::new();
        exp_sec.append(Some("Export as PNG"), Some("win.export-png"));
        exp_sec.append(Some("Export as JPEG"), Some("win.export-jpeg"));
        exp_sec.append(Some("Export as WebP"), Some("win.export-webp"));
        m.append_section(Some("Export"), &exp_sec);
        m
    };
    let save_btn = gtk4::MenuButton::builder()
        .icon_name("document-save-symbolic")
        .tooltip_text("Save / Export")
        .menu_model(&save_menu)
        .sensitive(false)
        .build();

    let header = adw::HeaderBar::new();
    header.pack_start(&open_btn);
    header.pack_end(&save_btn);
    header.pack_end(&zoom_group);
    header.pack_end(&edit_group);

    // ── Layout ────────────────────────────────────────────────────────────────
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.append(&scrolled);
    content.append(&crop_bar);
    content.append(&text_tool_bar);
    content.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    content.append(&status_box);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&content));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Preview")
        .default_width(1024)
        .default_height(768)
        .content(&toolbar_view)
        .build();

    // Add inline text entry as overlay on the canvas
    canvas_overlay.add_overlay(&draft_entry);

    // Blinking cursor state — shared between the blink timer and the draw func
    let cursor_blink_on = Rc::new(Cell::new(false));
    // Cursor character position within the draft text (mirrors entry's cursor-position)
    let draft_cursor_pos: Rc<Cell<i32>> = Rc::new(Cell::new(0));

    // ── Canvas draw function ──────────────────────────────────────────────────
    //
    // Single draw func handles both the image and the crop overlay.
    // Image transform:
    //   fit mode  → scale to fill viewport, centered (letterbox)
    //   zoom mode → cr.scale(zoom); content_width/height drive scrollbars
    canvas.set_draw_func({
        let state = state.clone();
        let cursor_blink_on = cursor_blink_on.clone();
        let draft_cursor_pos = draft_cursor_pos.clone();
        move |_da, cr, width, height| {
            let s = state.borrow();
            let Some(ref surface) = s.surface else { return };

            let vw = width as f64;
            let vh = height as f64;
            let img_w = s.img_width as f64;
            let img_h = s.img_height as f64;

            let (ox, oy, scale) = if s.fit_mode {
                image_ops::fit_transform(s.img_width, s.img_height, vw, vh)
            } else {
                (0.0, 0.0, s.zoom)
            };

            // Draw image
            cr.save().unwrap();
            cr.translate(ox, oy);
            cr.scale(scale, scale);
            cr.set_source_surface(surface, 0.0, 0.0).unwrap();
            cr.source().set_filter(cairo::Filter::Bilinear);
            cr.paint().unwrap();
            // Draw committed annotations
            for (i, ann) in s.annotations.iter().enumerate() {
                image_ops::draw_text_annotation(cr, ann);
                // Selection outline when text tool is active
                if s.text_tool_active && s.selected_ann == Some(i) {
                    let layout = pangocairo::functions::create_layout(cr);
                    layout.set_font_description(Some(&ann.font_desc));
                    layout.set_text(&ann.text);
                    let (tw, th) = layout.pixel_size();
                    let half_w = tw as f64 / 2.0;
                    let half_h = th as f64 / 2.0;
                    let pad = 4.0 / scale;
                    let handle_r = 5.0 / scale;
                    cr.save().unwrap();
                    cr.translate(ann.x + half_w, ann.y + half_h);
                    cr.rotate(ann.rotation);
                    let bx = -half_w - pad;
                    let by = -half_h - pad;
                    let bw = tw as f64 + pad * 2.0;
                    let bh = th as f64 + pad * 2.0;
                    // Dashed bounding box
                    cr.set_source_rgba(0.2, 0.6, 1.0, 0.9);
                    cr.set_line_width(1.5 / scale);
                    cr.set_dash(&[5.0 / scale, 4.0 / scale], 0.0);
                    cr.rectangle(bx, by, bw, bh);
                    cr.stroke().unwrap();
                    cr.set_dash(&[], 0.0);
                    // Corner rotation handles
                    for (cx, cy) in [(bx, by), (bx + bw, by), (bx, by + bh), (bx + bw, by + bh)] {
                        cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                        cr.arc(cx, cy, handle_r, 0.0, std::f64::consts::TAU);
                        cr.fill().unwrap();
                        cr.set_source_rgba(0.2, 0.6, 1.0, 0.9);
                        cr.set_line_width(1.5 / scale);
                        cr.arc(cx, cy, handle_r, 0.0, std::f64::consts::TAU);
                        cr.stroke().unwrap();
                    }
                    cr.restore().unwrap();
                }
            }
            // Draw draft annotation (live preview while typing)
            if let Some((dx, dy)) = s.draft_pos {
                let font_desc = s.text_font_desc.clone()
                    .unwrap_or_else(|| gtk4::pango::FontDescription::from_string("Sans 24"));
                if !s.draft_text.is_empty() {
                    let preview = image_ops::TextAnnotation {
                        x: dx,
                        y: dy,
                        text: s.draft_text.clone(),
                        font_desc: font_desc.clone(),
                        color: s.text_color,
                        rotation: s.text_rotation,
                    };
                    image_ops::draw_text_annotation(cr, &preview);
                }
                // Blinking text cursor at the entry's current cursor position
                if cursor_blink_on.get() {
                    let layout = pangocairo::functions::create_layout(cr);
                    layout.set_font_description(Some(&font_desc));
                    layout.set_text(&s.draft_text);
                    // Convert character index → byte index for Pango
                    let char_pos = draft_cursor_pos.get() as usize;
                    let byte_idx = s.draft_text
                        .char_indices()
                        .nth(char_pos)
                        .map(|(i, _)| i)
                        .unwrap_or(s.draft_text.len()) as i32;
                    let rect = layout.index_to_pos(byte_idx);
                    let cursor_x = rect.x() as f64 / gtk4::pango::SCALE as f64;
                    let (tw, ph) = layout.pixel_size();
                    let half_w = tw as f64 / 2.0;
                    let half_h = ph as f64 / 2.0;
                    let (r, g, b, a) = s.text_color;
                    cr.set_source_rgba(r, g, b, a);
                    cr.set_line_width(2.0 / scale);
                    cr.save().unwrap();
                    // Rotate around text centre (dx/dy is top-left)
                    cr.translate(dx + half_w, dy + half_h);
                    cr.rotate(s.text_rotation);
                    cr.move_to(-half_w + cursor_x, -half_h);
                    cr.line_to(-half_w + cursor_x, half_h);
                    cr.stroke().unwrap();
                    cr.restore().unwrap();
                }
                // Placement dot (shows anchor point when text is empty)
                if s.draft_text.is_empty() {
                    let dot = 4.0 / scale;
                    cr.set_source_rgba(1.0, 0.9, 0.0, 0.9);
                    cr.arc(dx, dy, dot, 0.0, std::f64::consts::TAU);
                    cr.fill().unwrap();
                }
            }
            cr.restore().unwrap();

            // Crop overlay
            if s.in_crop {
                let rendered_w = img_w * scale;
                let rendered_h = img_h * scale;
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.5);

                if let (Some((ax, ay)), Some((bx, by))) =
                    (s.drag_start, s.drag_end)
                {
                    let sx = ax.min(bx);
                    let sy = ay.min(by);
                    let ex = ax.max(bx);
                    let ey = ay.max(by);

                    // 4 dark bands around the selection
                    cr.rectangle(ox, oy, rendered_w, sy - oy);
                    cr.fill().unwrap();
                    cr.rectangle(ox, ey, rendered_w, oy + rendered_h - ey);
                    cr.fill().unwrap();
                    cr.rectangle(ox, sy, sx - ox, ey - sy);
                    cr.fill().unwrap();
                    cr.rectangle(ex, sy, ox + rendered_w - ex, ey - sy);
                    cr.fill().unwrap();

                    // Border
                    cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
                    cr.set_line_width(1.5);
                    cr.rectangle(sx, sy, ex - sx, ey - sy);
                    cr.stroke().unwrap();

                    // Rule-of-thirds grid
                    cr.set_source_rgba(1.0, 1.0, 1.0, 0.35);
                    cr.set_line_width(0.5);
                    let sw = ex - sx;
                    let sh = ey - sy;
                    for i in 1..3 {
                        let f = i as f64 / 3.0;
                        cr.move_to(sx + sw * f, sy);
                        cr.line_to(sx + sw * f, ey);
                        cr.stroke().unwrap();
                        cr.move_to(sx, sy + sh * f);
                        cr.line_to(ex, sy + sh * f);
                        cr.stroke().unwrap();
                    }
                } else {
                    // No selection yet — overlay the entire image area
                    cr.rectangle(ox, oy, rendered_w, rendered_h);
                    cr.fill().unwrap();
                }
            }
        }
    });

    // ── Closures (defined in dependency order) ────────────────────────────────

    // 1. apply_zoom — updates canvas content size and queues a redraw
    let apply_zoom: Rc<dyn Fn()> = Rc::new({
        let canvas = canvas.clone();
        let zoom_label = zoom_label.clone();
        let zoom_out_btn = zoom_out_btn.clone();
        let zoom_in_btn = zoom_in_btn.clone();
        let state = state.clone();
        move || {
            let s = state.borrow();
            if !s.has_image() {
                return;
            }
            if s.fit_mode {
                // Let the canvas fill the viewport; the draw func computes scale
                canvas.set_hexpand(true);
                canvas.set_vexpand(true);
                canvas.set_halign(gtk4::Align::Fill);
                canvas.set_valign(gtk4::Align::Fill);
                canvas.set_content_width(0);
                canvas.set_content_height(0);
                zoom_label.set_text("Fit");
            } else {
                // Drive the scrolled window by setting the natural (content) size
                let cw = (s.img_width as f64 * s.zoom).round() as i32;
                let ch = (s.img_height as f64 * s.zoom).round() as i32;
                canvas.set_hexpand(false);
                canvas.set_vexpand(false);
                canvas.set_halign(gtk4::Align::Center);
                canvas.set_valign(gtk4::Align::Center);
                canvas.set_content_width(cw);
                canvas.set_content_height(ch);
                zoom_label.set_text(&format!("{:.0}%", s.zoom * 100.0));
            }
            zoom_out_btn.set_sensitive(true);
            zoom_in_btn.set_sensitive(true);
            canvas.queue_draw();
        }
    });

    // 2. update_image — store new DynamicImage, build cairo surface, refresh UI
    let update_image: Rc<dyn Fn(DynamicImage)> = Rc::new({
        let canvas = canvas.clone();
        let state = state.clone();
        let apply_zoom = apply_zoom.clone();
        let status_label = status_label.clone();
        let window = window.clone();
        let save_btn = save_btn.clone();
        let resize_btn = resize_btn.clone();
        let rotate_ccw_btn = rotate_ccw_btn.clone();
        let rotate_cw_btn = rotate_cw_btn.clone();
        let flip_h_btn = flip_h_btn.clone();
        let flip_v_btn = flip_v_btn.clone();
        let crop_btn = crop_btn.clone();
        let text_btn = text_btn.clone();
        let zoom_fit_btn = zoom_fit_btn.clone();
        let zoom_orig_btn = zoom_orig_btn.clone();
        move |img: DynamicImage| {
            let surface = image_ops::to_cairo_surface(&img);
            let (w, h) = (img.width() as i32, img.height() as i32);
            {
                let mut s = state.borrow_mut();
                s.img_width = w;
                s.img_height = h;
                s.surface = Some(surface);
                s.image = Some(img);
                s.annotations.clear();
            }

            // Enable buttons
            for btn in &[
                resize_btn.upcast_ref::<gtk4::Widget>(),
                rotate_ccw_btn.upcast_ref(),
                rotate_cw_btn.upcast_ref(),
                flip_h_btn.upcast_ref(),
                flip_v_btn.upcast_ref(),
                crop_btn.upcast_ref(),
                text_btn.upcast_ref(),
                zoom_fit_btn.upcast_ref(),
                zoom_orig_btn.upcast_ref(),
                save_btn.upcast_ref(),
            ] {
                btn.set_sensitive(true);
            }

            // Update status
            let title = window.title().unwrap_or_default();
            let name = title.trim_end_matches(" — Preview").to_string();
            status_label.set_markup(&format!(
                "<b>{}</b>  {}×{} px",
                glib::markup_escape_text(&name),
                w,
                h
            ));

            // Force a redraw; apply_zoom refreshes content size
            canvas.queue_draw();
            apply_zoom();
        }
    });

    // 3. load_image_file — try image crate, fall back to GDK (SVG, etc.)
    let load_image_file: Rc<dyn Fn(&std::path::Path)> = Rc::new({
        let canvas = canvas.clone();
        let window = window.clone();
        let state = state.clone();
        let status_label = status_label.clone();
        let apply_zoom = apply_zoom.clone();
        let update_image = update_image.clone();
        let zoom_fit_btn = zoom_fit_btn.clone();
        let zoom_orig_btn = zoom_orig_btn.clone();
        let save_btn = save_btn.clone();
        let resize_btn = resize_btn.clone();
        let rotate_ccw_btn = rotate_ccw_btn.clone();
        let rotate_cw_btn = rotate_cw_btn.clone();
        let flip_h_btn = flip_h_btn.clone();
        let flip_v_btn = flip_v_btn.clone();
        let crop_btn = crop_btn.clone();
        let text_btn = text_btn.clone();

        move |path: &std::path::Path| {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();

            match image::open(path) {
                Ok(img) => {
                    window.set_title(Some(&format!("{} — Preview", name)));
                    {
                        let mut s = state.borrow_mut();
                        s.zoom = 1.0;
                        s.fit_mode = true;
                        s.file_path = Some(path.to_path_buf());
                    }
                    update_image(img);
                }
                Err(_) => {
                    // Display-only fallback (SVG, etc.)
                    let file = gio::File::for_path(path);
                    match gdk::Texture::from_file(&file) {
                        Ok(texture) => {
                            let w = texture.width();
                            let h = texture.height();
                            // For GDK-only images, we build a cairo surface from pixels
                            // by downloading the texture, since we can't use image crate.
                            // For now we store no DynamicImage (no editing).
                            {
                                let mut s = state.borrow_mut();
                                s.img_width = w;
                                s.img_height = h;
                                s.image = None;
                                s.surface = gdk_texture_to_cairo(&texture);
                                s.zoom = 1.0;
                                s.fit_mode = true;
                                s.file_path = Some(path.to_path_buf());
                                s.annotations.clear();
                            }
                            window.set_title(Some(&format!("{} — Preview", name)));
                            status_label.set_markup(&format!(
                                "<b>{}</b>  {}×{} px  (display only)",
                                glib::markup_escape_text(&name),
                                w,
                                h
                            ));
                            for btn in &[
                                zoom_fit_btn.upcast_ref::<gtk4::Widget>(),
                                zoom_orig_btn.upcast_ref(),
                                save_btn.upcast_ref(),
                            ] {
                                btn.set_sensitive(true);
                            }
                            for btn in &[
                                resize_btn.upcast_ref::<gtk4::Widget>(),
                                rotate_ccw_btn.upcast_ref(),
                                rotate_cw_btn.upcast_ref(),
                                flip_h_btn.upcast_ref(),
                                flip_v_btn.upcast_ref(),
                                crop_btn.upcast_ref(),
                                text_btn.upcast_ref(),
                            ] {
                                btn.set_sensitive(false);
                            }
                            canvas.queue_draw();
                            apply_zoom();
                        }
                        Err(err) => {
                            let dialog = gtk4::AlertDialog::builder()
                                .message("Cannot Open Image")
                                .detail(&err.to_string())
                                .buttons(["OK"])
                                .build();
                            dialog.show(Some(&window));
                        }
                    }
                }
            }
        }
    });

    // 4. show_open_dialog
    let show_open_dialog: Rc<dyn Fn()> = Rc::new({
        let window = window.clone();
        let load_image_file = load_image_file.clone();
        move || {
            let filter = gtk4::FileFilter::new();
            filter.set_name(Some("Images"));
            for mime in &[
                "image/png", "image/jpeg", "image/gif", "image/bmp",
                "image/tiff", "image/webp", "image/svg+xml",
            ] {
                filter.add_mime_type(mime);
            }
            let filters = gio::ListStore::new::<gtk4::FileFilter>();
            filters.append(&filter);

            let dialog = gtk4::FileDialog::builder()
                .title("Open Image")
                .modal(true)
                .filters(&filters)
                .build();

            let window = window.clone();
            let load_image_file = load_image_file.clone();
            dialog.open(Some(&window), gio::Cancellable::NONE, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        load_image_file(&path);
                    }
                }
            });
        }
    });

    // 5. set_crop_mode
    let set_crop_mode: Rc<dyn Fn(bool)> = Rc::new({
        let state = state.clone();
        let canvas = canvas.clone();
        let crop_bar = crop_bar.clone();
        let edit_group = edit_group.clone();
        let apply_crop_btn = apply_crop_btn.clone();
        let apply_zoom = apply_zoom.clone();
        move |active: bool| {
            {
                let mut s = state.borrow_mut();
                s.in_crop = active;
                s.drag_start = None;
                s.drag_end = None;
                if active {
                    s.fit_mode = true;
                }
            }
            crop_bar.set_visible(active);
            edit_group.set_sensitive(!active);
            apply_crop_btn.set_sensitive(false);
            if active {
                apply_zoom();
            }
            canvas.queue_draw();
        }
    });

    // ── Blink timer helpers ───────────────────────────────────────────────────
    let blink_source: Rc<Cell<Option<glib::SourceId>>> = Rc::new(Cell::new(None));

    let start_blink: Rc<dyn Fn()> = Rc::new({
        let cursor_blink_on = cursor_blink_on.clone();
        let blink_source = blink_source.clone();
        let canvas = canvas.clone();
        move || {
            if let Some(id) = blink_source.take() { id.remove(); }
            cursor_blink_on.set(true);
            canvas.queue_draw();
            let id = glib::timeout_add_local(std::time::Duration::from_millis(530), {
                let cursor_blink_on = cursor_blink_on.clone();
                let canvas = canvas.clone();
                move || {
                    cursor_blink_on.set(!cursor_blink_on.get());
                    canvas.queue_draw();
                    glib::ControlFlow::Continue
                }
            });
            blink_source.set(Some(id));
        }
    });

    let stop_blink: Rc<dyn Fn()> = Rc::new({
        let cursor_blink_on = cursor_blink_on.clone();
        let blink_source = blink_source.clone();
        let canvas = canvas.clone();
        move || {
            if let Some(id) = blink_source.take() { id.remove(); }
            cursor_blink_on.set(false);
            canvas.queue_draw();
        }
    });

    // 6. commit_draft — push any in-progress annotation to the list
    let commit_draft: Rc<dyn Fn()> = Rc::new({
        let state = state.clone();
        let canvas = canvas.clone();
        let draft_entry = draft_entry.clone();
        let stop_blink = stop_blink.clone();
        move || {
            stop_blink();
            let mut s = state.borrow_mut();
            s.draft_center = None;
            if let Some((dx, dy)) = s.draft_pos.take() {
                let text = std::mem::take(&mut s.draft_text);
                if !text.is_empty() {
                    let font_desc = s
                        .text_font_desc
                        .clone()
                        .unwrap_or_else(|| gtk4::pango::FontDescription::from_string("Sans 24"));
                    let color = s.text_color;
                    let rotation = s.text_rotation;
                    s.annotations.push(image_ops::TextAnnotation {
                        x: dx,
                        y: dy,
                        text,
                        font_desc,
                        color,
                        rotation,
                    });
                }
            } else {
                s.draft_text.clear();
            }
            drop(s);
            draft_entry.set_text("");
            draft_entry.set_visible(false);
            canvas.queue_draw();
        }
    });

    // 7. set_text_mode
    let set_text_mode: Rc<dyn Fn(bool)> = Rc::new({
        let state = state.clone();
        let canvas = canvas.clone();
        let text_tool_bar = text_tool_bar.clone();
        let edit_group = edit_group.clone();
        let commit_draft = commit_draft.clone();
        move |active: bool| {
            if !active {
                commit_draft();
                state.borrow_mut().selected_ann = None;
            }
            state.borrow_mut().text_tool_active = active;
            text_tool_bar.set_visible(active);
            edit_group.set_sensitive(!active);
            canvas.queue_draw();
        }
    });

    // ── Text tool: click to place / select ───────────────────────────────────

    let text_click = gtk4::GestureClick::new();
    text_click.connect_released({
        let state = state.clone();
        let canvas = canvas.clone();
        let draft_entry = draft_entry.clone();
        let commit_draft = commit_draft.clone();
        let set_text_mode = set_text_mode.clone();
        let font_btn = font_btn.clone();
        let color_btn = color_btn.clone();
        let start_blink = start_blink.clone();
        let draft_cursor_pos = draft_cursor_pos.clone();
        let rotation_spin = rotation_spin.clone();
        move |g, n, x, y| {
            let (active, has_image, fit_mode, img_w, img_h) = {
                let s = state.borrow();
                (s.text_tool_active, s.image.is_some(), s.fit_mode, s.img_width, s.img_height)
            };
            if !has_image {
                g.set_state(gtk4::EventSequenceState::Denied);
                return;
            }
            // Any click on an annotation re-enters text mode (even after Done)
            if !active {
                let has_hit = {
                    let vw = canvas.width() as f64;
                    let vh = canvas.height() as f64;
                    let s = state.borrow();
                    let (ox, oy, scale) = if s.fit_mode {
                        image_ops::fit_transform(s.img_width, s.img_height, vw, vh)
                    } else {
                        (0.0, 0.0, s.zoom)
                    };
                    hit_test_annotation(&s.annotations, x, y, ox, oy, scale, &canvas.pango_context()).is_some()
                };
                if has_hit {
                    set_text_mode(true);
                } else {
                    g.set_state(gtk4::EventSequenceState::Denied);
                    return;
                }
            }
            let vw = canvas.width() as f64;
            let vh = canvas.height() as f64;
            let (ox, oy, scale) = if fit_mode {
                image_ops::fit_transform(img_w, img_h, vw, vh)
            } else {
                (0.0, 0.0, state.borrow().zoom)
            };
            // Hit-test existing annotations
            let hit = {
                let s = state.borrow();
                hit_test_annotation(&s.annotations, x, y, ox, oy, scale, &canvas.pango_context())
            };
            if let Some(idx) = hit {
                commit_draft();
                if n >= 2 {
                    // Double-click → unpack annotation into draft mode for editing
                    let (ann_x, ann_y, ann_text) = {
                        let s = state.borrow();
                        let ann = &s.annotations[idx];
                        (ann.x, ann.y, ann.text.clone())
                    };
                    // Compute the fixed visual centre of the existing annotation
                    let (ann_cx, ann_cy) = {
                        let pc = canvas.pango_context();
                        let layout = gtk4::pango::Layout::new(&pc);
                        let s = state.borrow();
                        let fd = s.annotations[idx].font_desc.clone();
                        drop(s);
                        layout.set_font_description(Some(&fd));
                        layout.set_text(&ann_text);
                        let (tw, th) = layout.pixel_size();
                        (ann_x + tw as f64 / scale / 2.0,
                         ann_y + th as f64 / scale / 2.0)
                    };
                    {
                        let mut s = state.borrow_mut();
                        s.annotations.remove(idx);
                        s.draft_pos = Some((ann_x, ann_y));
                        s.draft_center = Some((ann_cx, ann_cy));
                        s.draft_text = ann_text.clone();
                        s.selected_ann = None;
                    }
                    // Pre-fill entry; select all so typing immediately replaces
                    draft_entry.set_text(&ann_text);
                    let sx = (ox + ann_x * scale) as i32;
                    let sy = (oy + ann_y * scale) as i32;
                    let ent = draft_entry.clone();
                    let blink = start_blink.clone();
                    let cpos = draft_cursor_pos.clone();
                    glib::idle_add_local_once(move || {
                        ent.set_margin_start(sx);
                        ent.set_margin_top(sy);
                        ent.set_visible(true);
                        ent.grab_focus();
                        // GTK auto-selects all on focus — clear it by placing
                        // cursor at end with no selection.
                        let end = ent.text().chars().count() as i32;
                        ent.select_region(end, end);
                        cpos.set(end);
                        blink();
                    });
                } else {
                    // Single click → select for dragging, sync toolbar to annotation's style
                    let (ann_font, ann_color, ann_rotation) = {
                        let s = state.borrow();
                        let ann = &s.annotations[idx];
                        (ann.font_desc.clone(), ann.color, ann.rotation)
                    };
                    state.borrow_mut().selected_ann = Some(idx);
                    font_btn.set_font_desc(&ann_font);
                    color_btn.set_rgba(&gdk::RGBA::new(
                        ann_color.0 as f32,
                        ann_color.1 as f32,
                        ann_color.2 as f32,
                        ann_color.3 as f32,
                    ));
                    rotation_spin.set_value(ann_rotation.to_degrees());
                }
                canvas.queue_draw();
                return;
            }
            // No annotation hit → commit any draft and start a new one here
            commit_draft();
            let img_x = (x - ox) / scale;
            let img_y = (y - oy) / scale;
            {
                let mut s = state.borrow_mut();
                s.draft_pos = Some((img_x, img_y));
                s.draft_center = Some((img_x, img_y)); // fixed visual centre
                s.draft_text.clear();
                s.selected_ann = None;
                s.text_rotation = 0.0;
            }
            rotation_spin.set_value(0.0);
            draft_entry.set_text("");
            let ent = draft_entry.clone();
            let blink = start_blink.clone();
            let cpos = draft_cursor_pos.clone();
            glib::idle_add_local_once(move || {
                ent.set_margin_start(x as i32);
                ent.set_margin_top(y as i32);
                ent.set_visible(true);
                ent.grab_focus();
                cpos.set(ent.property::<i32>("cursor-position"));
                blink();
            });
            canvas.queue_draw();
        }
    });
    canvas_overlay.add_controller(text_click);

    // ── Text tool: drag to move a selected annotation ─────────────────────────

    let text_drag = gtk4::GestureDrag::new();
    text_drag.set_exclusive(true);
    text_drag.connect_drag_begin({
        let state = state.clone();
        let canvas = canvas.clone();
        move |g, x, y| {
            enum DragMode { Rotate, Move }
            let pango_ctx = canvas.pango_context();
            let (active, drag_mode, hit) = {
                let s = state.borrow();
                let vw = canvas.width() as f64;
                let vh = canvas.height() as f64;
                let (ox, oy, scale) = if s.fit_mode {
                    image_ops::fit_transform(s.img_width, s.img_height, vw, vh)
                } else {
                    (0.0, 0.0, s.zoom)
                };
                // Priority: rotation corner > body move
                let (mode, hit) = if let Some(idx) = s.selected_ann {
                    let ann = &s.annotations[idx];
                    if hit_test_rotation_handle(ann, x, y, ox, oy, scale, &pango_ctx) {
                        // Compute text-centre anchor in widget space
                        let layout = gtk4::pango::Layout::new(&pango_ctx);
                        layout.set_font_description(Some(&ann.font_desc));
                        layout.set_text(&ann.text);
                        let (tw, th) = layout.pixel_size();
                        let anchor_wx = ox + ann.x * scale + tw as f64 / 2.0;
                        let anchor_wy = oy + ann.y * scale + th as f64 / 2.0;
                        (DragMode::Rotate, Some((idx, ann.x, ann.y, anchor_wx, anchor_wy, ann.rotation)))
                    } else {
                        let h = hit_test_annotation(&s.annotations, x, y, ox, oy, scale, &pango_ctx)
                            .map(|i| (i, s.annotations[i].x, s.annotations[i].y, 0.0, 0.0, 0.0));
                        (DragMode::Move, h)
                    }
                } else {
                    let h = hit_test_annotation(&s.annotations, x, y, ox, oy, scale, &pango_ctx)
                        .map(|i| (i, s.annotations[i].x, s.annotations[i].y, 0.0, 0.0, 0.0));
                    (DragMode::Move, h)
                };
                (s.text_tool_active, mode, hit)
            };
            if !active {
                g.set_state(gtk4::EventSequenceState::Denied);
                return;
            }
            if let Some((idx, orig_x, orig_y, anchor_wx, anchor_wy, init_rot)) = hit {
                let mut s = state.borrow_mut();
                s.selected_ann = Some(idx);
                match drag_mode {
                    DragMode::Rotate => {
                        s.rotation_drag = true;
                        s.rotation_drag_anchor = (anchor_wx, anchor_wy);
                        s.rotation_drag_begin = (x, y);
                        s.rotation_drag_initial_rotation = init_rot;
                    }
                    DragMode::Move => {
                        s.move_origin = Some((orig_x, orig_y));
                    }
                }
                drop(s);
                canvas.queue_draw();
            } else {
                g.set_state(gtk4::EventSequenceState::Denied);
            }
        }
    });
    text_drag.connect_drag_update({
        let state = state.clone();
        let canvas = canvas.clone();
        let rotation_spin = rotation_spin.clone();
        move |_g, dx, dy| {
            let s = state.borrow();
            if !s.text_tool_active { return; }
            let (_, _, scale) = if s.fit_mode {
                image_ops::fit_transform(s.img_width, s.img_height,
                    canvas.width() as f64, canvas.height() as f64)
            } else { (0.0, 0.0, s.zoom) };
            if s.rotation_drag {
                if let Some(idx) = s.selected_ann {
                    let (ax, ay) = s.rotation_drag_anchor;
                    let (bx, by) = s.rotation_drag_begin;
                    let init_rot = s.rotation_drag_initial_rotation;
                    // Angles from anchor to begin and to current position
                    let angle_begin = (by - ay).atan2(bx - ax);
                    let cur_x = bx + dx;
                    let cur_y = by + dy;
                    let angle_cur = (cur_y - ay).atan2(cur_x - ax);
                    let new_rot = init_rot + (angle_cur - angle_begin);
                    drop(s);
                    let mut s = state.borrow_mut();
                    s.annotations[idx].rotation = new_rot;
                    s.text_rotation = new_rot;
                    drop(s);
                    rotation_spin.set_value(new_rot.to_degrees());
                    canvas.queue_draw();
                }
            } else if let (Some(idx), Some((ox, oy))) = (s.selected_ann, s.move_origin) {
                let nx = ox + dx / scale;
                let ny = oy + dy / scale;
                drop(s);
                let mut s = state.borrow_mut();
                s.annotations[idx].x = nx;
                s.annotations[idx].y = ny;
                drop(s);
                canvas.queue_draw();
            }
        }
    });
    text_drag.connect_drag_end({
        let state = state.clone();
        move |_, _, _| {
            let mut s = state.borrow_mut();
            s.move_origin = None;
            s.rotation_drag = false;
        }
    });
    canvas_overlay.add_controller(text_drag);

    // ── Crop gesture ──────────────────────────────────────────────────────────

    let drag = gtk4::GestureDrag::new();
    drag.set_exclusive(true);

    drag.connect_drag_begin({
        let state = state.clone();
        let canvas = canvas.clone();
        let apply_crop_btn = apply_crop_btn.clone();
        move |g, x, y| {
            if !state.borrow().in_crop {
                g.set_state(gtk4::EventSequenceState::Denied);
                return;
            }
            let mut s = state.borrow_mut();
            // Clamp to image bounds
            let vw = canvas.width() as f64;
            let vh = canvas.height() as f64;
            let (ox, oy, scale) =
                image_ops::fit_transform(s.img_width, s.img_height, vw, vh);
            let cx = x.clamp(ox, ox + s.img_width as f64 * scale);
            let cy = y.clamp(oy, oy + s.img_height as f64 * scale);
            s.drag_start = Some((cx, cy));
            s.drag_end = Some((cx, cy));
            apply_crop_btn.set_sensitive(false);
            drop(s);
            canvas.queue_draw();
        }
    });

    drag.connect_drag_update({
        let state = state.clone();
        let canvas = canvas.clone();
        move |g, dx, dy| {
            if !state.borrow().in_crop {
                return;
            }
            let Some((sx, sy)) = g.start_point() else { return };
            let mut s = state.borrow_mut();
            let vw = canvas.width() as f64;
            let vh = canvas.height() as f64;
            let (ox, oy, scale) =
                image_ops::fit_transform(s.img_width, s.img_height, vw, vh);
            let ex = (sx + dx).clamp(ox, ox + s.img_width as f64 * scale);
            let ey = (sy + dy).clamp(oy, oy + s.img_height as f64 * scale);
            s.drag_end = Some((ex, ey));
            drop(s);
            canvas.queue_draw();
        }
    });

    drag.connect_drag_end({
        let state = state.clone();
        let canvas = canvas.clone();
        let apply_crop_btn = apply_crop_btn.clone();
        move |g, dx, dy| {
            if !state.borrow().in_crop {
                return;
            }
            let Some((sx, sy)) = g.start_point() else { return };
            let mut s = state.borrow_mut();
            let vw = canvas.width() as f64;
            let vh = canvas.height() as f64;
            let (ox, oy, scale) =
                image_ops::fit_transform(s.img_width, s.img_height, vw, vh);
            let ex = (sx + dx).clamp(ox, ox + s.img_width as f64 * scale);
            let ey = (sy + dy).clamp(oy, oy + s.img_height as f64 * scale);
            s.drag_end = Some((ex, ey));
            let valid = s
                .drag_start
                .zip(s.drag_end)
                .map(|((ax, ay), (bx, by))| {
                    (ax - bx).abs() > 4.0 && (ay - by).abs() > 4.0
                })
                .unwrap_or(false);
            drop(s);
            apply_crop_btn.set_sensitive(valid);
            canvas.queue_draw();
        }
    });

    canvas.add_controller(drag);

    // ── Button callbacks ──────────────────────────────────────────────────────

    open_btn.connect_clicked({
        let show_open_dialog = show_open_dialog.clone();
        move |_| show_open_dialog()
    });

    zoom_in_btn.connect_clicked({
        let state = state.clone();
        let apply_zoom = apply_zoom.clone();
        move |_| {
            let mut s = state.borrow_mut();
            if s.fit_mode { s.fit_mode = false; }
            s.zoom = (s.zoom * 1.25).min(32.0);
            drop(s);
            apply_zoom();
        }
    });
    zoom_out_btn.connect_clicked({
        let state = state.clone();
        let apply_zoom = apply_zoom.clone();
        move |_| {
            let mut s = state.borrow_mut();
            if s.fit_mode { s.fit_mode = false; }
            s.zoom = (s.zoom / 1.25).max(0.05);
            drop(s);
            apply_zoom();
        }
    });
    zoom_fit_btn.connect_clicked({
        let state = state.clone();
        let apply_zoom = apply_zoom.clone();
        move |_| {
            state.borrow_mut().fit_mode = true;
            apply_zoom();
        }
    });
    zoom_orig_btn.connect_clicked({
        let state = state.clone();
        let apply_zoom = apply_zoom.clone();
        move |_| {
            let mut s = state.borrow_mut();
            s.zoom = 1.0;
            s.fit_mode = false;
            drop(s);
            apply_zoom();
        }
    });

    resize_btn.connect_clicked({
        let state = state.clone();
        let window = window.clone();
        let update_image = update_image.clone();
        move |_| {
            let (w, h) = {
                let s = state.borrow();
                (s.img_width as u32, s.img_height as u32)
            };
            let state = state.clone();
            let update_image = update_image.clone();
            show_resize_dialog(&window, w, h, move |nw, nh| {
                let img = state.borrow().image.as_ref().map(|i| i.clone());
                if let Some(img) = img {
                    update_image(image_ops::resize(&img, nw, nh));
                }
            });
        }
    });

    rotate_cw_btn.connect_clicked({
        let state = state.clone();
        let update_image = update_image.clone();
        move |_| {
            let img = state.borrow().image.as_ref().map(|i| i.clone());
            if let Some(img) = img { update_image(image_ops::rotate_cw(&img)); }
        }
    });
    rotate_ccw_btn.connect_clicked({
        let state = state.clone();
        let update_image = update_image.clone();
        move |_| {
            let img = state.borrow().image.as_ref().map(|i| i.clone());
            if let Some(img) = img { update_image(image_ops::rotate_ccw(&img)); }
        }
    });
    flip_h_btn.connect_clicked({
        let state = state.clone();
        let update_image = update_image.clone();
        move |_| {
            let img = state.borrow().image.as_ref().map(|i| i.clone());
            if let Some(img) = img { update_image(image_ops::flip_h(&img)); }
        }
    });
    flip_v_btn.connect_clicked({
        let state = state.clone();
        let update_image = update_image.clone();
        move |_| {
            let img = state.borrow().image.as_ref().map(|i| i.clone());
            if let Some(img) = img { update_image(image_ops::flip_v(&img)); }
        }
    });

    crop_btn.connect_clicked({
        let set_crop_mode = set_crop_mode.clone();
        move |_| set_crop_mode(true)
    });
    cancel_crop_btn.connect_clicked({
        let set_crop_mode = set_crop_mode.clone();
        move |_| set_crop_mode(false)
    });

    text_btn.connect_clicked({
        let set_text_mode = set_text_mode.clone();
        move |_| set_text_mode(true)
    });
    done_text_btn.connect_clicked({
        let set_text_mode = set_text_mode.clone();
        move |_| set_text_mode(false)
    });

    draft_entry.connect_changed({
        let state = state.clone();
        let canvas = canvas.clone();
        move |entry| {
            let text = entry.text().to_string();
            let (font_desc, scale) = {
                let s = state.borrow_mut();
                let fd = s.text_font_desc.clone()
                    .unwrap_or_else(|| gtk4::pango::FontDescription::from_string("Sans 24"));
                let sc = if s.fit_mode {
                    let (_, _, sc) = image_ops::fit_transform(
                        s.img_width, s.img_height,
                        canvas.width() as f64, canvas.height() as f64,
                    );
                    sc
                } else {
                    s.zoom
                };
                drop(s);
                (fd, sc)
            };
            state.borrow_mut().draft_text = text.clone();
            // Measure current text; update draft_pos so the visual centre stays fixed
            let pc = canvas.pango_context();
            let layout = gtk4::pango::Layout::new(&pc);
            layout.set_font_description(Some(&font_desc));
            layout.set_text(&text);
            let (pw, ph) = layout.pixel_size();
            if !text.is_empty() {
                entry.set_width_request((pw as f64 * scale).ceil() as i32 + 4);
                entry.set_height_request((ph as f64 * scale).ceil() as i32 + 4);
            }
            // Keep the visual centre pinned as text grows/shrinks
            let draft_center = state.borrow().draft_center;
            if let Some((cx, cy)) = draft_center {
                let half_w = pw as f64 / scale / 2.0;
                let half_h = ph as f64 / scale / 2.0;
                state.borrow_mut().draft_pos = Some((cx - half_w, cy - half_h));
            }
            canvas.queue_draw();
        }
    });
    draft_entry.connect_activate({
        let commit_draft = commit_draft.clone();
        move |_| commit_draft()
    });
    // Track cursor position so the canvas cursor follows arrow-key movement
    draft_entry.connect_notify_local(Some("cursor-position"), {
        let draft_cursor_pos = draft_cursor_pos.clone();
        let start_blink = start_blink.clone();
        move |entry, _| {
            draft_cursor_pos.set(entry.property::<i32>("cursor-position"));
            start_blink(); // restart blink so cursor is always visible after moving
        }
    });
    // Escape cancels the draft without committing
    let esc_ctrl = gtk4::EventControllerKey::new();
    esc_ctrl.connect_key_pressed({
        let state = state.clone();
        let canvas = canvas.clone();
        let draft_entry = draft_entry.clone();
        let stop_blink = stop_blink.clone();
        move |_, keyval, _, _| {
            if keyval == gdk::Key::Escape {
                stop_blink();
                let mut s = state.borrow_mut();
                s.draft_pos = None;
                s.draft_center = None;
                s.draft_text.clear();
                drop(s);
                draft_entry.set_text("");
                draft_entry.set_visible(false);
                canvas.queue_draw();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        }
    });
    draft_entry.add_controller(esc_ctrl);

    // Delete / Ctrl+C / Ctrl+V for selected annotations (when not in draft mode)
    let ann_key_ctrl = gtk4::EventControllerKey::new();
    ann_key_ctrl.connect_key_pressed({
        let state = state.clone();
        let canvas = canvas.clone();
        let draft_entry = draft_entry.clone();
        move |_, keyval, _, modifiers| {
            // Only act when the text tool is active and we're not mid-edit
            let s = state.borrow();
            if !s.text_tool_active || gtk4::prelude::WidgetExt::is_visible(&draft_entry) {
                return glib::Propagation::Proceed;
            }
            let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
            let selected = s.selected_ann;
            drop(s);

            match (keyval, ctrl) {
                // Delete selected annotation
                (gdk::Key::Delete, false) | (gdk::Key::KP_Delete, false) => {
                    let mut s = state.borrow_mut();
                    if let Some(idx) = selected {
                        s.annotations.remove(idx);
                        s.selected_ann = None;
                    }
                    drop(s);
                    canvas.queue_draw();
                    glib::Propagation::Stop
                }
                // Ctrl+C — copy selected annotation
                (gdk::Key::c, true) | (gdk::Key::C, true) => {
                    let mut s = state.borrow_mut();
                    if let Some(idx) = selected {
                        s.clipboard = s.annotations.get(idx).cloned();
                    }
                    glib::Propagation::Stop
                }
                // Ctrl+V — paste with +10x +10y offset (accumulates per paste)
                (gdk::Key::v, true) | (gdk::Key::V, true) => {
                    let mut s = state.borrow_mut();
                    if let Some(ref mut cb) = s.clipboard {
                        cb.x += 10.0;
                        cb.y += 10.0;
                        let ann = cb.clone();
                        s.annotations.push(ann);
                        s.selected_ann = Some(s.annotations.len() - 1);
                        drop(s);
                        canvas.queue_draw();
                    }
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        }
    });
    window.add_controller(ann_key_ctrl);

    font_btn.connect_font_desc_notify({
        let state = state.clone();
        let canvas = canvas.clone();
        move |btn| {
            let mut s = state.borrow_mut();
            s.text_font_desc = btn.font_desc();
            if let Some(idx) = s.selected_ann {
                if let Some(ann) = s.annotations.get_mut(idx) {
                    if let Some(fd) = btn.font_desc() {
                        ann.font_desc = fd;
                    }
                }
                drop(s);
                canvas.queue_draw();
            }
        }
    });
    color_btn.connect_rgba_notify({
        let state = state.clone();
        let canvas = canvas.clone();
        move |btn| {
            let c = btn.rgba();
            let color = (c.red() as f64, c.green() as f64, c.blue() as f64, c.alpha() as f64);
            let mut s = state.borrow_mut();
            s.text_color = color;
            if let Some(idx) = s.selected_ann {
                if let Some(ann) = s.annotations.get_mut(idx) {
                    ann.color = color;
                }
                drop(s);
                canvas.queue_draw();
            }
        }
    });

    rotation_spin.connect_value_changed({
        let state = state.clone();
        let canvas = canvas.clone();
        move |spin| {
            let rad = spin.value().to_radians();
            let mut s = state.borrow_mut();
            s.text_rotation = rad;
            if let Some(idx) = s.selected_ann {
                if let Some(ann) = s.annotations.get_mut(idx) {
                    ann.rotation = rad;
                }
            }
            drop(s);
            canvas.queue_draw();
        }
    });

    apply_crop_btn.connect_clicked({
        let state = state.clone();
        let canvas = canvas.clone();
        let update_image = update_image.clone();
        let set_crop_mode = set_crop_mode.clone();
        move |_| {
            let (crop_rect, img) = {
                let s = state.borrow();
                let vw = canvas.width() as f64;
                let vh = canvas.height() as f64;
                let (ox, oy, scale) =
                    image_ops::fit_transform(s.img_width, s.img_height, vw, vh);
                let rect = s.drag_start.zip(s.drag_end).map(|((ax, ay), (bx, by))| {
                    let (x1, y1) = image_ops::widget_to_img(
                        ax.min(bx), ay.min(by),
                        s.img_width, s.img_height, ox, oy, scale,
                    );
                    let (x2, y2) = image_ops::widget_to_img(
                        ax.max(bx), ay.max(by),
                        s.img_width, s.img_height, ox, oy, scale,
                    );
                    (x1, y1, x2.saturating_sub(x1), y2.saturating_sub(y1))
                });
                (rect, s.image.as_ref().map(|i| i.clone()))
            };
            if let (Some((x, y, w, h)), Some(img)) = (crop_rect, img) {
                if let Some(cropped) = image_ops::crop(&img, x, y, w, h) {
                    update_image(cropped);
                }
            }
            set_crop_mode(false);
        }
    });

    // ── Scroll-wheel zoom (Ctrl + scroll) ─────────────────────────────────────
    let scroll_ctrl = gtk4::EventControllerScroll::new(
        gtk4::EventControllerScrollFlags::VERTICAL
            | gtk4::EventControllerScrollFlags::DISCRETE,
    );
    scroll_ctrl.connect_scroll({
        let state = state.clone();
        let apply_zoom = apply_zoom.clone();
        move |ctrl, _dx, dy| {
            if !ctrl
                .current_event_state()
                .contains(gdk::ModifierType::CONTROL_MASK)
            {
                return glib::Propagation::Proceed;
            }
            let mut s = state.borrow_mut();
            if !s.has_image() { return glib::Propagation::Proceed; }
            if s.fit_mode { s.fit_mode = false; }
            if dy < 0.0 { s.zoom = (s.zoom * 1.15).min(32.0); }
            else         { s.zoom = (s.zoom / 1.15).max(0.05); }
            drop(s);
            apply_zoom();
            glib::Propagation::Stop
        }
    });
    scrolled.add_controller(scroll_ctrl);

    // ── Drag-and-drop ─────────────────────────────────────────────────────────
    let drop_target = gtk4::DropTarget::new(gio::File::static_type(), gdk::DragAction::COPY);
    drop_target.connect_drop({
        let load_image_file = load_image_file.clone();
        move |_, value, _, _| {
            if let Ok(file) = value.get::<gio::File>() {
                if let Some(path) = file.path() {
                    load_image_file(&path);
                    return true;
                }
            }
            false
        }
    });
    window.add_controller(drop_target);

    // ── Window actions ────────────────────────────────────────────────────────

    let act_open = gio::SimpleAction::new("open", None);
    act_open.connect_activate({
        let show_open_dialog = show_open_dialog.clone();
        move |_, _| show_open_dialog()
    });

    let act_zoom_in = gio::SimpleAction::new("zoom-in", None);
    act_zoom_in.connect_activate({
        let state = state.clone();
        let apply_zoom = apply_zoom.clone();
        move |_, _| {
            let mut s = state.borrow_mut();
            if s.fit_mode { s.fit_mode = false; }
            s.zoom = (s.zoom * 1.25).min(32.0);
            drop(s);
            apply_zoom();
        }
    });

    let act_zoom_out = gio::SimpleAction::new("zoom-out", None);
    act_zoom_out.connect_activate({
        let state = state.clone();
        let apply_zoom = apply_zoom.clone();
        move |_, _| {
            let mut s = state.borrow_mut();
            if s.fit_mode { s.fit_mode = false; }
            s.zoom = (s.zoom / 1.25).max(0.05);
            drop(s);
            apply_zoom();
        }
    });

    let act_zoom_fit = gio::SimpleAction::new("zoom-fit", None);
    act_zoom_fit.connect_activate({
        let state = state.clone();
        let apply_zoom = apply_zoom.clone();
        move |_, _| {
            state.borrow_mut().fit_mode = true;
            apply_zoom();
        }
    });

    let act_zoom_orig = gio::SimpleAction::new("zoom-orig", None);
    act_zoom_orig.connect_activate({
        let state = state.clone();
        let apply_zoom = apply_zoom.clone();
        move |_, _| {
            let mut s = state.borrow_mut();
            s.zoom = 1.0;
            s.fit_mode = false;
            drop(s);
            apply_zoom();
        }
    });

    let act_fullscreen =
        gio::SimpleAction::new_stateful("fullscreen", None, &false.to_variant());
    act_fullscreen.connect_activate({
        let window = window.clone();
        move |action, _| {
            let is_fs = action.state().and_then(|v| v.get::<bool>()).unwrap_or(false);
            if is_fs { window.unfullscreen(); action.set_state(&false.to_variant()); }
            else      { window.fullscreen();   action.set_state(&true.to_variant()); }
        }
    });

    // ── Save actions ──────────────────────────────────────────────────────────

    // Save (overwrite original file)
    let act_save = gio::SimpleAction::new("save", None);
    act_save.connect_activate({
        let state = state.clone();
        let window = window.clone();
        move |_, _| {
            let (img, annotations, path) = {
                let s = state.borrow();
                (
                    s.image.as_ref().map(|i| i.clone()),
                    s.annotations.clone(),
                    s.file_path.clone(),
                )
            };
            if let (Some(img), Some(path)) = (img, path) {
                let flat = image_ops::flatten_annotations(&img, &annotations);
                if let Err(e) = image_ops::save_image(&flat, &path) {
                    show_error(&window, "Save failed", &e.to_string());
                }
            }
        }
    });

    // Save As (pick new path, update file_path, keep format from extension)
    let act_save_as = gio::SimpleAction::new("save-as", None);
    act_save_as.connect_activate({
        let state = state.clone();
        let window = window.clone();
        move |_, _| {
            let (img, annotations, current_path) = {
                let s = state.borrow();
                (
                    s.image.as_ref().map(|i| i.clone()),
                    s.annotations.clone(),
                    s.file_path.clone(),
                )
            };
            if img.is_none() { return; }
            let img = img.unwrap();
            let state = state.clone();
            let window = window.clone();
            let window2 = window.clone();
            show_save_dialog(&window, current_path.as_deref(), None, move |path| {
                let flat = image_ops::flatten_annotations(&img, &annotations);
                if let Err(e) = image_ops::save_image(&flat, &path) {
                    show_error(&window2, "Save failed", &e.to_string());
                    return;
                }
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                window2.set_title(Some(&format!("{} — Preview", name)));
                state.borrow_mut().file_path = Some(path);
            });
        }
    });

    // Export as PNG / JPEG / WebP — file dialog, do NOT update file_path
    let make_export_action = |ext: &'static str| {
        let act = gio::SimpleAction::new(&format!("export-{}", ext), None);
        let state = state.clone();
        let window = window.clone();
        act.connect_activate(move |_, _| {
            let (img, annotations, current_path) = {
                let s = state.borrow();
                (
                    s.image.as_ref().map(|i| i.clone()),
                    s.annotations.clone(),
                    s.file_path.clone(),
                )
            };
            if img.is_none() { return; }
            let img = img.unwrap();
            // Suggest filename with new extension
            let suggested = current_path.as_deref().and_then(|p| p.file_stem())
                .map(|s| format!("{}.{}", s.to_string_lossy(), ext));
            let window = window.clone();
            let window2 = window.clone();
            show_save_dialog(&window, None, suggested.as_deref(), move |path| {
                let flat = image_ops::flatten_annotations(&img, &annotations);
                if let Err(e) = image_ops::save_image(&flat, &path) {
                    show_error(&window2, "Export failed", &e.to_string());
                }
            });
        });
        act
    };
    let act_export_png  = make_export_action("png");
    let act_export_jpeg = make_export_action("jpeg");
    let act_export_webp = make_export_action("webp");

    for a in &[
        act_open.upcast_ref::<gio::Action>(),
        act_zoom_in.upcast_ref(),
        act_zoom_out.upcast_ref(),
        act_zoom_fit.upcast_ref(),
        act_zoom_orig.upcast_ref(),
        act_fullscreen.upcast_ref(),
        act_save.upcast_ref(),
        act_save_as.upcast_ref(),
        act_export_png.upcast_ref(),
        act_export_jpeg.upcast_ref(),
        act_export_webp.upcast_ref(),
    ] {
        window.add_action(*a);
    }

    app.set_accels_for_action("win.open",      &["<Ctrl>o"]);
    app.set_accels_for_action("win.zoom-in",   &["plus", "equal", "<Ctrl>equal"]);
    app.set_accels_for_action("win.zoom-out",  &["minus", "<Ctrl>minus"]);
    app.set_accels_for_action("win.zoom-fit",  &["3", "<Ctrl>0"]);
    app.set_accels_for_action("win.zoom-orig", &["1"]);
    app.set_accels_for_action("win.fullscreen",&["F11"]);
    app.set_accels_for_action("win.save",      &["<Ctrl>s"]);
    app.set_accels_for_action("win.save-as",   &["<Ctrl><Shift>s"]);
    app.set_accels_for_action("app.quit",      &["<Ctrl>q"]);

    let act_quit = gio::SimpleAction::new("quit", None);
    act_quit.connect_activate({
        let app = app.clone();
        move |_, _| app.quit()
    });
    app.add_action(&act_quit);

    // ── Initial file ──────────────────────────────────────────────────────────
    if let Some(path) = initial_file {
        load_image_file(path);
    }

    window.present();
}

// ── Resize dialog ─────────────────────────────────────────────────────────────

fn show_resize_dialog(
    parent: &adw::ApplicationWindow,
    current_w: u32,
    current_h: u32,
    on_apply: impl Fn(u32, u32) + 'static,
) {
    let dialog = adw::Window::builder()
        .title("Resize Image")
        .transient_for(parent)
        .modal(true)
        .default_width(340)
        .resizable(false)
        .build();

    let width_row = adw::SpinRow::with_range(1.0, 32767.0, 1.0);
    width_row.set_title("Width");
    width_row.set_digits(0);
    width_row.set_value(current_w as f64);

    let height_row = adw::SpinRow::with_range(1.0, 32767.0, 1.0);
    height_row.set_title("Height");
    height_row.set_digits(0);
    height_row.set_value(current_h as f64);

    let lock_row = adw::SwitchRow::new();
    lock_row.set_title("Lock aspect ratio");
    lock_row.set_active(true);

    let dim_group = adw::PreferencesGroup::new();
    dim_group.add(&width_row);
    dim_group.add(&height_row);
    dim_group.add(&lock_row);
    dim_group.set_margin_top(12);
    dim_group.set_margin_bottom(8);
    dim_group.set_margin_start(12);
    dim_group.set_margin_end(12);

    let ratio = current_w as f64 / current_h as f64;
    let updating = Rc::new(Cell::new(false));

    width_row.connect_value_notify({
        let height_row = height_row.clone();
        let lock_row = lock_row.clone();
        let updating = updating.clone();
        move |row| {
            if updating.get() { return; }
            if lock_row.is_active() {
                updating.set(true);
                height_row.set_value((row.value() / ratio).round().max(1.0));
                updating.set(false);
            }
        }
    });

    height_row.connect_value_notify({
        let width_row = width_row.clone();
        let lock_row = lock_row.clone();
        let updating = updating.clone();
        move |row| {
            if updating.get() { return; }
            if lock_row.is_active() {
                updating.set(true);
                width_row.set_value((row.value() * ratio).round().max(1.0));
                updating.set(false);
            }
        }
    });

    let cancel_btn = gtk4::Button::with_label("Cancel");
    let resize_btn = gtk4::Button::with_label("Resize");
    resize_btn.add_css_class("suggested-action");

    let btn_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    btn_box.set_halign(gtk4::Align::End);
    btn_box.set_margin_start(12);
    btn_box.set_margin_end(12);
    btn_box.set_margin_top(4);
    btn_box.set_margin_bottom(12);
    btn_box.append(&cancel_btn);
    btn_box.append(&resize_btn);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.append(&dim_group);
    content.append(&btn_box);

    let hb = adw::HeaderBar::new();
    let tv = adw::ToolbarView::new();
    tv.add_top_bar(&hb);
    tv.set_content(Some(&content));
    dialog.set_content(Some(&tv));

    cancel_btn.connect_clicked({
        let dialog = dialog.clone();
        move |_| dialog.close()
    });
    resize_btn.connect_clicked({
        let dialog = dialog.clone();
        let width_row = width_row.clone();
        let height_row = height_row.clone();
        move |_| {
            on_apply(width_row.value() as u32, height_row.value() as u32);
            dialog.close();
        }
    });

    dialog.present();
}

// ── Save / Export dialog ──────────────────────────────────────────────────────

fn show_save_dialog(
    parent: &adw::ApplicationWindow,
    current_path: Option<&std::path::Path>,
    suggested_name: Option<&str>,
    on_save: impl Fn(PathBuf) + 'static,
) {
    let filters = gio::ListStore::new::<gtk4::FileFilter>();
    for (label, mime, ext) in &[
        ("PNG",  "image/png",  "png"),
        ("JPEG", "image/jpeg", "jpg"),
        ("WebP", "image/webp", "webp"),
        ("TIFF", "image/tiff", "tiff"),
        ("BMP",  "image/bmp",  "bmp"),
    ] {
        let f = gtk4::FileFilter::new();
        f.set_name(Some(label));
        f.add_mime_type(mime);
        f.add_suffix(ext);
        filters.append(&f);
    }

    let dialog = gtk4::FileDialog::builder()
        .title("Save Image")
        .modal(true)
        .filters(&filters)
        .build();

    // Set initial folder from current path
    if let Some(p) = current_path {
        if let Some(parent_dir) = p.parent() {
            dialog.set_initial_folder(Some(&gio::File::for_path(parent_dir)));
        }
    }

    // Set initial filename
    let initial = suggested_name
        .map(|s| s.to_string())
        .or_else(|| {
            current_path
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
        });
    if let Some(name) = initial {
        dialog.set_initial_name(Some(&name));
    }

    let parent = parent.clone();
    dialog.save(Some(&parent), gio::Cancellable::NONE, move |result| {
        if let Ok(file) = result {
            if let Some(path) = file.path() {
                on_save(path);
            }
        }
    });
}

// ── Error dialog ──────────────────────────────────────────────────────────────

fn show_error(parent: &adw::ApplicationWindow, title: &str, detail: &str) {
    let dialog = gtk4::AlertDialog::builder()
        .message(title)
        .detail(detail)
        .buttons(["OK"])
        .build();
    dialog.show(Some(parent));
}

// ── GDK texture → Cairo surface (for display-only formats like SVG) ───────────

fn gdk_texture_to_cairo(texture: &gdk::Texture) -> Option<cairo::ImageSurface> {
    let w = texture.width();
    let h = texture.height();
    let stride = (w * 4) as usize;
    // download() gives RGBA straight-alpha
    let mut rgba = vec![0u8; stride * h as usize];
    texture.download(&mut rgba, stride);
    // Convert to premultiplied BGRA (cairo ARgb32 on little-endian)
    let mut bgra: Vec<u8> = Vec::with_capacity(rgba.len());
    for chunk in rgba.chunks_exact(4) {
        let r = chunk[0] as u32;
        let g = chunk[1] as u32;
        let b = chunk[2] as u32;
        let a = chunk[3] as u32;
        bgra.push(((b * a + 127) / 255) as u8);
        bgra.push(((g * a + 127) / 255) as u8);
        bgra.push(((r * a + 127) / 255) as u8);
        bgra.push(a as u8);
    }
    cairo::ImageSurface::create_for_data(
        bgra,
        cairo::Format::ARgb32,
        w,
        h,
        stride as i32,
    )
    .ok()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns the index of the topmost annotation whose text bounding box contains (wx, wy)
/// in widget (viewport) coordinates.
fn hit_test_annotation(
    annotations: &[image_ops::TextAnnotation],
    wx: f64,
    wy: f64,
    ox: f64,
    oy: f64,
    scale: f64,
    pango_ctx: &gtk4::pango::Context,
) -> Option<usize> {
    for (i, ann) in annotations.iter().enumerate().rev() {
        let layout = gtk4::pango::Layout::new(pango_ctx);
        layout.set_font_description(Some(&ann.font_desc));
        layout.set_text(&ann.text);
        let (tw, th) = layout.pixel_size();
        let cx = ox + ann.x * scale + tw as f64 / 2.0;
        let cy = oy + ann.y * scale + th as f64 / 2.0;
        let dx = (wx - cx) / scale;
        let dy = (wy - cy) / scale;
        let cos_r = ann.rotation.cos();
        let sin_r = ann.rotation.sin();
        let lx = dx * cos_r + dy * sin_r;
        let ly = -dx * sin_r + dy * cos_r;
        let pad = 4.0 / scale;
        let half_w = tw as f64 / scale / 2.0 + pad;
        let half_h = th as f64 / scale / 2.0 + pad;
        if lx >= -half_w && lx <= half_w && ly >= -half_h && ly <= half_h {
            return Some(i);
        }
    }
    None
}


/// Returns true if (wx, wy) is on a corner rotation handle of `ann`.
fn hit_test_rotation_handle(
    ann: &image_ops::TextAnnotation,
    wx: f64,
    wy: f64,
    ox: f64,
    oy: f64,
    scale: f64,
    pango_ctx: &gtk4::pango::Context,
) -> bool {
    let layout = gtk4::pango::Layout::new(pango_ctx);
    layout.set_font_description(Some(&ann.font_desc));
    layout.set_text(&ann.text);
    let (tw, th) = layout.pixel_size();
    let cx = ox + ann.x * scale + tw as f64 / 2.0;
    let cy = oy + ann.y * scale + th as f64 / 2.0;
    let dx = (wx - cx) / scale;
    let dy = (wy - cy) / scale;
    let cos_r = ann.rotation.cos();
    let sin_r = ann.rotation.sin();
    let lx = dx * cos_r + dy * sin_r;
    let ly = -dx * sin_r + dy * cos_r;
    let pad = 4.0 / scale;
    let hit_r = 10.0 / scale;
    let half_w = tw as f64 / scale / 2.0 + pad;
    let half_h = th as f64 / scale / 2.0 + pad;
    for (hx, hy) in [(-half_w, -half_h), (half_w, -half_h), (-half_w, half_h), (half_w, half_h)] {
        if ((lx - hx).powi(2) + (ly - hy).powi(2)).sqrt() <= hit_r {
            return true;
        }
    }
    false
}

fn linked_box(buttons: &[&gtk4::Widget]) -> gtk4::Box {
    let b = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    b.add_css_class("linked");
    for btn in buttons {
        b.append(*btn);
    }
    b
}
