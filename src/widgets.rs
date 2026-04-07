use gtk4::prelude::*;
use gtk4::{gdk, gio};
use libadwaita as adw;

pub struct Widgets {
    pub canvas: gtk4::DrawingArea,
    pub canvas_overlay: gtk4::Overlay,
    pub scrolled: gtk4::ScrolledWindow,
    pub window: adw::ApplicationWindow,
    pub app: adw::Application,
    pub status_label: gtk4::Label,
    pub zoom_label: gtk4::Label,
    pub cancel_crop_btn: gtk4::Button,
    pub apply_crop_btn: gtk4::Button,
    pub crop_bar: gtk4::Box,
    pub draft_entry: gtk4::Entry,
    pub font_btn: gtk4::FontDialogButton,
    pub color_btn: gtk4::ColorDialogButton,
    pub rotation_spin: gtk4::SpinButton,
    pub text_tool_bar: gtk4::Box,
    pub done_text_btn: gtk4::Button,
    pub edit_group: gtk4::Box,
    pub open_btn: gtk4::Button,
    pub undo_btn: gtk4::Button,
    pub zoom_out_btn: gtk4::Button,
    pub zoom_fit_btn: gtk4::Button,
    pub zoom_orig_btn: gtk4::Button,
    pub zoom_in_btn: gtk4::Button,
    pub resize_btn: gtk4::Button,
    pub rotate_ccw_btn: gtk4::Button,
    pub rotate_cw_btn: gtk4::Button,
    pub flip_h_btn: gtk4::Button,
    pub flip_v_btn: gtk4::Button,
    pub crop_btn: gtk4::Button,
    pub text_btn: gtk4::Button,
    pub save_btn: gtk4::MenuButton,
    // Shape tool
    pub rect_btn: gtk4::Button,
    pub line_btn: gtk4::Button,
    pub arrow_btn: gtk4::Button,
    pub shape_tool_bar: gtk4::Box,
    pub shape_done_btn: gtk4::Button,
    pub shape_color_btn: gtk4::ColorDialogButton,
    pub shape_stroke_spin: gtk4::SpinButton,
}

fn linked_box(buttons: &[&gtk4::Widget]) -> gtk4::Box {
    let b = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    b.add_css_class("linked");
    for btn in buttons { b.append(*btn); }
    b
}

pub fn build(app: &adw::Application) -> Widgets {
    let canvas = gtk4::DrawingArea::builder().hexpand(true).vexpand(true).build();
    let canvas_overlay = gtk4::Overlay::new();
    canvas_overlay.set_child(Some(&canvas));
    let scrolled = gtk4::ScrolledWindow::builder().hexpand(true).vexpand(true).build();
    scrolled.set_child(Some(&canvas_overlay));

    let status_label = gtk4::Label::builder()
        .label("Open an image to get started")
        .margin_start(12).margin_top(5).margin_bottom(5)
        .halign(gtk4::Align::Start).hexpand(true)
        .ellipsize(gtk4::pango::EllipsizeMode::Middle)
        .build();
    let zoom_label = gtk4::Label::builder()
        .label("").margin_end(12).margin_top(5).margin_bottom(5)
        .halign(gtk4::Align::End).build();
    zoom_label.add_css_class("dim-label");
    let status_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    status_box.append(&status_label);
    status_box.append(&zoom_label);

    let cancel_crop_btn = gtk4::Button::with_label("Cancel");
    let apply_crop_btn = gtk4::Button::builder().label("Apply Crop").sensitive(false).build();
    apply_crop_btn.add_css_class("suggested-action");
    let crop_hint = gtk4::Label::builder()
        .label("Drag to select the crop area").hexpand(true).halign(gtk4::Align::Center).build();
    crop_hint.add_css_class("dim-label");
    let crop_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    crop_bar.set_visible(false);
    crop_bar.set_margin_start(12); crop_bar.set_margin_end(12);
    crop_bar.set_margin_top(6); crop_bar.set_margin_bottom(6);
    crop_bar.append(&cancel_crop_btn);
    crop_bar.append(&crop_hint);
    crop_bar.append(&apply_crop_btn);

    let draft_entry = gtk4::Entry::builder()
        .placeholder_text("Type and press Enter…")
        .width_request(1).height_request(1)
        .halign(gtk4::Align::Start).valign(gtk4::Align::Start)
        .visible(false).opacity(0.0).can_target(false)
        .build();

    let font_btn = gtk4::FontDialogButton::new(Some(gtk4::FontDialog::new()));
    font_btn.set_font_desc(&gtk4::pango::FontDescription::from_string("Sans 24"));
    let color_btn = gtk4::ColorDialogButton::new(Some(gtk4::ColorDialog::new()));
    color_btn.set_rgba(&gdk::RGBA::new(1.0, 0.0, 0.0, 1.0));
    let done_text_btn = gtk4::Button::with_label("Done");
    let text_hint = gtk4::Label::builder()
        .label("Click image to place text — drag to move")
        .hexpand(true).halign(gtk4::Align::Center).build();
    text_hint.add_css_class("dim-label");
    let rotation_adj = gtk4::Adjustment::new(0.0, -180.0, 180.0, 1.0, 15.0, 0.0);
    let rotation_spin = gtk4::SpinButton::new(Some(&rotation_adj), 1.0, 0);
    rotation_spin.set_wrap(true);
    rotation_spin.set_tooltip_text(Some("Rotation (°)"));
    rotation_spin.set_width_chars(5);
    let rotation_label = gtk4::Label::new(Some("°"));
    let text_tool_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    text_tool_bar.set_visible(false);
    text_tool_bar.set_margin_start(12); text_tool_bar.set_margin_end(12);
    text_tool_bar.set_margin_top(6); text_tool_bar.set_margin_bottom(6);
    text_tool_bar.append(&done_text_btn);
    text_tool_bar.append(&text_hint);
    text_tool_bar.append(&font_btn);
    text_tool_bar.append(&color_btn);
    text_tool_bar.append(&rotation_spin);
    text_tool_bar.append(&rotation_label);

    let open_btn = gtk4::Button::builder()
        .icon_name("document-open-symbolic").tooltip_text("Open Image (Ctrl+O)").build();
    let undo_btn = gtk4::Button::builder()
        .icon_name("edit-undo-symbolic").tooltip_text("Undo (Ctrl+Z)").sensitive(false).build();
    let zoom_out_btn = gtk4::Button::builder()
        .icon_name("zoom-out-symbolic").tooltip_text("Zoom Out  (–)").sensitive(false).build();
    let zoom_fit_btn = gtk4::Button::builder()
        .icon_name("zoom-fit-best-symbolic").tooltip_text("Fit to Window  (3)").sensitive(false).build();
    let zoom_orig_btn = gtk4::Button::builder()
        .icon_name("zoom-original-symbolic").tooltip_text("Actual Size  (1)").sensitive(false).build();
    let zoom_in_btn = gtk4::Button::builder()
        .icon_name("zoom-in-symbolic").tooltip_text("Zoom In  (+)").sensitive(false).build();
    let zoom_group = linked_box(&[
        zoom_out_btn.upcast_ref(), zoom_fit_btn.upcast_ref(),
        zoom_orig_btn.upcast_ref(), zoom_in_btn.upcast_ref(),
    ]);
    let resize_btn = gtk4::Button::builder()
        .label("Resize").tooltip_text("Change dimensions").sensitive(false).build();
    let rotate_ccw_btn = gtk4::Button::builder()
        .icon_name("object-rotate-left-symbolic").tooltip_text("Rotate 90° Left").sensitive(false).build();
    let rotate_cw_btn = gtk4::Button::builder()
        .icon_name("object-rotate-right-symbolic").tooltip_text("Rotate 90° Right").sensitive(false).build();
    let flip_h_btn = gtk4::Button::builder()
        .icon_name("object-flip-horizontal-symbolic").tooltip_text("Flip Horizontal").sensitive(false).build();
    let flip_v_btn = gtk4::Button::builder()
        .icon_name("object-flip-vertical-symbolic").tooltip_text("Flip Vertical").sensitive(false).build();
    let crop_btn = gtk4::Button::builder()
        .icon_name("edit-cut-symbolic").tooltip_text("Crop Image").sensitive(false).build();
    let text_btn = gtk4::Button::builder()
        .icon_name("insert-text-symbolic").tooltip_text("Add Text Annotation").sensitive(false).build();
    let edit_group = linked_box(&[
        resize_btn.upcast_ref(), rotate_ccw_btn.upcast_ref(), rotate_cw_btn.upcast_ref(),
        flip_h_btn.upcast_ref(), flip_v_btn.upcast_ref(), crop_btn.upcast_ref(), text_btn.upcast_ref(),
    ]);

    let rect_btn = gtk4::Button::builder()
        .label("Rect").tooltip_text("Draw Rectangle").sensitive(false).build();
    let line_btn = gtk4::Button::builder()
        .label("Line").tooltip_text("Draw Line").sensitive(false).build();
    let arrow_btn = gtk4::Button::builder()
        .label("Arrow").tooltip_text("Draw Arrow").sensitive(false).build();
    let shapes_group = linked_box(&[
        rect_btn.upcast_ref(), line_btn.upcast_ref(), arrow_btn.upcast_ref(),
    ]);

    let shape_done_btn = gtk4::Button::with_label("Done");
    let shape_hint = gtk4::Label::builder()
        .label("Drag on the image to draw — Ctrl+Z to undo")
        .hexpand(true).halign(gtk4::Align::Center).build();
    shape_hint.add_css_class("dim-label");
    let shape_color_btn = gtk4::ColorDialogButton::new(Some(gtk4::ColorDialog::new()));
    shape_color_btn.set_rgba(&gdk::RGBA::new(1.0, 0.0, 0.0, 1.0));
    let stroke_adj = gtk4::Adjustment::new(2.0, 1.0, 100.0, 1.0, 5.0, 0.0);
    let shape_stroke_spin = gtk4::SpinButton::new(Some(&stroke_adj), 1.0, 0);
    shape_stroke_spin.set_tooltip_text(Some("Stroke width (px)"));
    shape_stroke_spin.set_width_chars(4);
    let stroke_label = gtk4::Label::new(Some("px"));
    let shape_tool_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    shape_tool_bar.set_visible(false);
    shape_tool_bar.set_margin_start(12); shape_tool_bar.set_margin_end(12);
    shape_tool_bar.set_margin_top(6); shape_tool_bar.set_margin_bottom(6);
    shape_tool_bar.append(&shape_done_btn);
    shape_tool_bar.append(&shape_hint);
    shape_tool_bar.append(&shape_color_btn);
    shape_tool_bar.append(&shape_stroke_spin);
    shape_tool_bar.append(&stroke_label);

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
        .icon_name("document-save-symbolic").tooltip_text("Save / Export")
        .menu_model(&save_menu).sensitive(false).build();

    let header = adw::HeaderBar::new();
    header.pack_start(&open_btn);
    header.pack_start(&undo_btn);
    header.pack_end(&save_btn);
    header.pack_end(&zoom_group);
    header.pack_end(&edit_group);
    header.pack_end(&shapes_group);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.append(&scrolled);
    content.append(&crop_bar);
    content.append(&text_tool_bar);
    content.append(&shape_tool_bar);
    content.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    content.append(&status_box);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&content));

    let window = adw::ApplicationWindow::builder()
        .application(app).title("Preview")
        .default_width(1024).default_height(768)
        .content(&toolbar_view)
        .build();

    canvas_overlay.add_overlay(&draft_entry);

    Widgets {
        canvas, canvas_overlay, scrolled, window, app: app.clone(),
        status_label, zoom_label,
        cancel_crop_btn, apply_crop_btn, crop_bar,
        draft_entry, font_btn, color_btn, rotation_spin, text_tool_bar, done_text_btn,
        edit_group, open_btn, undo_btn, zoom_out_btn, zoom_fit_btn, zoom_orig_btn, zoom_in_btn,
        resize_btn, rotate_ccw_btn, rotate_cw_btn, flip_h_btn, flip_v_btn,
        crop_btn, text_btn, save_btn,
        rect_btn, line_btn, arrow_btn,
        shape_tool_bar, shape_done_btn, shape_color_btn, shape_stroke_spin,
    }
}
