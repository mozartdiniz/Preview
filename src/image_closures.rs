use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};
use image::DynamicImage;
use std::cell::RefCell;
use std::rc::Rc;

use crate::annotation;
use crate::state::State;
use crate::widgets::Widgets;

pub fn make_apply_zoom(w: &Widgets, state: Rc<RefCell<State>>) -> Rc<dyn Fn()> {
    let canvas = w.canvas.clone();
    let zoom_label = w.zoom_label.clone();
    let zoom_out_btn = w.zoom_out_btn.clone();
    let zoom_in_btn = w.zoom_in_btn.clone();
    Rc::new(move || {
        let s = state.borrow();
        if !s.has_image() { return; }
        if s.fit_mode {
            canvas.set_hexpand(true); canvas.set_vexpand(true);
            canvas.set_halign(gtk4::Align::Fill); canvas.set_valign(gtk4::Align::Fill);
            canvas.set_content_width(0); canvas.set_content_height(0);
            zoom_label.set_text("Fit");
        } else {
            let cw = (s.img_width as f64 * s.zoom).round() as i32;
            let ch = (s.img_height as f64 * s.zoom).round() as i32;
            canvas.set_hexpand(false); canvas.set_vexpand(false);
            canvas.set_halign(gtk4::Align::Center); canvas.set_valign(gtk4::Align::Center);
            canvas.set_content_width(cw); canvas.set_content_height(ch);
            zoom_label.set_text(&format!("{:.0}%", s.zoom * 100.0));
        }
        zoom_out_btn.set_sensitive(true);
        zoom_in_btn.set_sensitive(true);
        canvas.queue_draw();
    })
}

pub fn make_update_image(
    w: &Widgets,
    state: Rc<RefCell<State>>,
    apply_zoom: Rc<dyn Fn()>,
) -> Rc<dyn Fn(DynamicImage)> {
    let canvas = w.canvas.clone();
    let status_label = w.status_label.clone();
    let window = w.window.clone();
    let save_btn = w.save_btn.clone();
    let resize_btn = w.resize_btn.clone();
    let rotate_ccw_btn = w.rotate_ccw_btn.clone();
    let rotate_cw_btn = w.rotate_cw_btn.clone();
    let flip_h_btn = w.flip_h_btn.clone();
    let flip_v_btn = w.flip_v_btn.clone();
    let crop_btn = w.crop_btn.clone();
    let text_btn = w.text_btn.clone();
    let rect_btn = w.rect_btn.clone();
    let line_btn = w.line_btn.clone();
    let arrow_btn = w.arrow_btn.clone();
    let zoom_fit_btn = w.zoom_fit_btn.clone();
    let zoom_orig_btn = w.zoom_orig_btn.clone();
    let undo_btn = w.undo_btn.clone();
    Rc::new(move |img: DynamicImage| {
        let surface = annotation::to_cairo_surface(&img);
        let (iw, ih) = (img.width() as i32, img.height() as i32);
        {
            state.borrow_mut().push_undo();
            undo_btn.set_sensitive(true);
            let mut s = state.borrow_mut();
            s.img_width = iw; s.img_height = ih;
            s.surface = Some(surface); s.image = Some(img); s.annotations.clear();
            s.shape_annotations.clear();
            s.draft_pos = None; s.draft_center = None; s.draft_text.clear(); s.selected_ann = None;
        }
        for btn in &[
            resize_btn.upcast_ref::<gtk4::Widget>(), rotate_ccw_btn.upcast_ref(),
            rotate_cw_btn.upcast_ref(), flip_h_btn.upcast_ref(), flip_v_btn.upcast_ref(),
            crop_btn.upcast_ref(), text_btn.upcast_ref(),
            rect_btn.upcast_ref(), line_btn.upcast_ref(), arrow_btn.upcast_ref(),
            zoom_fit_btn.upcast_ref(), zoom_orig_btn.upcast_ref(), save_btn.upcast_ref(),
        ] { btn.set_sensitive(true); }
        let title = window.title().unwrap_or_default();
        let name = title.trim_end_matches(" — Preview").to_string();
        status_label.set_markup(&format!(
            "<b>{}</b>  {}×{} px",
            glib::markup_escape_text(&name), iw, ih
        ));
        canvas.queue_draw();
        apply_zoom();
    })
}

pub fn make_load_image_file(
    w: &Widgets,
    state: Rc<RefCell<State>>,
    apply_zoom: Rc<dyn Fn()>,
    update_image: Rc<dyn Fn(DynamicImage)>,
) -> Rc<dyn Fn(&std::path::Path)> {
    let canvas = w.canvas.clone();
    let window = w.window.clone();
    let status_label = w.status_label.clone();
    let zoom_fit_btn = w.zoom_fit_btn.clone();
    let zoom_orig_btn = w.zoom_orig_btn.clone();
    let save_btn = w.save_btn.clone();
    let resize_btn = w.resize_btn.clone();
    let rotate_ccw_btn = w.rotate_ccw_btn.clone();
    let rotate_cw_btn = w.rotate_cw_btn.clone();
    let flip_h_btn = w.flip_h_btn.clone();
    let flip_v_btn = w.flip_v_btn.clone();
    let crop_btn = w.crop_btn.clone();
    let text_btn = w.text_btn.clone();
    let rect_btn2 = w.rect_btn.clone();
    let line_btn2 = w.line_btn.clone();
    let arrow_btn2 = w.arrow_btn.clone();
    Rc::new(move |path: &std::path::Path| {
        let name = path.file_name()
            .map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match image::open(path) {
            Ok(img) => {
                window.set_title(Some(&format!("{} — Preview", name)));
                { let mut s = state.borrow_mut(); s.zoom = 1.0; s.fit_mode = true; s.file_path = Some(path.to_path_buf()); }
                update_image(img);
                { let mut s = state.borrow_mut(); s.undo_stack.clear(); s.redo_stack.clear(); }
            }
            Err(_) => {
                let file = gio::File::for_path(path);
                match gdk::Texture::from_file(&file) {
                    Ok(texture) => {
                        let tw = texture.width(); let th = texture.height();
                        {
                            let mut s = state.borrow_mut();
                            s.img_width = tw; s.img_height = th; s.image = None;
                            s.surface = annotation::gdk_texture_to_cairo(&texture);
                            s.zoom = 1.0; s.fit_mode = true;
                            s.file_path = Some(path.to_path_buf()); s.annotations.clear();
                            s.shape_annotations.clear();
                        }
                        window.set_title(Some(&format!("{} — Preview", name)));
                        { let mut s = state.borrow_mut(); s.undo_stack.clear(); s.redo_stack.clear(); }
                        status_label.set_markup(&format!(
                            "<b>{}</b>  {}×{} px  (display only)",
                            glib::markup_escape_text(&name), tw, th
                        ));
                        for btn in &[zoom_fit_btn.upcast_ref::<gtk4::Widget>(), zoom_orig_btn.upcast_ref(), save_btn.upcast_ref()] {
                            btn.set_sensitive(true);
                        }
                        for btn in &[
                            resize_btn.upcast_ref::<gtk4::Widget>(), rotate_ccw_btn.upcast_ref(),
                            rotate_cw_btn.upcast_ref(), flip_h_btn.upcast_ref(), flip_v_btn.upcast_ref(),
                            crop_btn.upcast_ref(), text_btn.upcast_ref(),
                            rect_btn2.upcast_ref(), line_btn2.upcast_ref(), arrow_btn2.upcast_ref(),
                        ] { btn.set_sensitive(false); }
                        canvas.queue_draw();
                        apply_zoom();
                    }
                    Err(err) => {
                        let dialog = gtk4::AlertDialog::builder()
                            .message("Cannot Open Image").detail(&err.to_string()).buttons(["OK"]).build();
                        dialog.show(Some(&window));
                    }
                }
            }
        }
    })
}

pub fn make_show_open_dialog(
    window: libadwaita::ApplicationWindow,
    load_image_file: Rc<dyn Fn(&std::path::Path)>,
) -> Rc<dyn Fn()> {
    Rc::new(move || {
        let filter = gtk4::FileFilter::new();
        filter.set_name(Some("Images"));
        for mime in &["image/png","image/jpeg","image/gif","image/bmp","image/tiff","image/webp","image/svg+xml"] {
            filter.add_mime_type(mime);
        }
        let filters = gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(&filter);
        let dialog = gtk4::FileDialog::builder()
            .title("Open Image").modal(true).filters(&filters).build();
        let window = window.clone();
        let load_image_file = load_image_file.clone();
        dialog.open(Some(&window), gio::Cancellable::NONE, move |result| {
            if let Ok(file) = result {
                if let Some(path) = file.path() { load_image_file(&path); }
            }
        });
    })
}

pub fn make_undo(
    w: &Widgets,
    state: Rc<RefCell<State>>,
    apply_zoom: Rc<dyn Fn()>,
) -> Rc<dyn Fn()> {
    let status_label = w.status_label.clone();
    let window = w.window.clone();
    let undo_btn = w.undo_btn.clone();
    Rc::new(move || {
        let mut s = state.borrow_mut();
        let Some(entry) = s.undo_stack.pop() else { return };
        let stack_empty = s.undo_stack.is_empty();
        let current = crate::state::HistoryEntry {
            image: s.image.clone(),
            img_width: s.img_width,
            img_height: s.img_height,
            annotations: s.annotations.clone(),
            shape_annotations: s.shape_annotations.clone(),
        };
        s.redo_stack.push(current);
        if let Some(img) = &entry.image {
            s.surface = Some(annotation::to_cairo_surface(img));
        }
        s.image = entry.image;
        s.img_width = entry.img_width;
        s.img_height = entry.img_height;
        s.annotations = entry.annotations;
        s.shape_annotations = entry.shape_annotations;
        s.draft_pos = None; s.draft_center = None; s.draft_text.clear();
        s.selected_ann = None; s.property_undo_pushed = false;
        s.selected_shape = None; s.shape_property_undo_pushed = false;
        let (iw, ih) = (s.img_width, s.img_height);
        drop(s);
        undo_btn.set_sensitive(!stack_empty);
        let title = window.title().unwrap_or_default();
        let name = title.trim_end_matches(" — Preview").to_string();
        status_label.set_markup(&format!("<b>{}</b>  {}×{} px", glib::markup_escape_text(&name), iw, ih));
        apply_zoom();
    })
}

pub fn make_redo(
    w: &Widgets,
    state: Rc<RefCell<State>>,
    apply_zoom: Rc<dyn Fn()>,
) -> Rc<dyn Fn()> {
    let status_label = w.status_label.clone();
    let window = w.window.clone();
    let undo_btn = w.undo_btn.clone();
    Rc::new(move || {
        let mut s = state.borrow_mut();
        let Some(entry) = s.redo_stack.pop() else { return };
        let current = crate::state::HistoryEntry {
            image: s.image.clone(),
            img_width: s.img_width,
            img_height: s.img_height,
            annotations: s.annotations.clone(),
            shape_annotations: s.shape_annotations.clone(),
        };
        s.undo_stack.push(current);
        if let Some(img) = &entry.image {
            s.surface = Some(annotation::to_cairo_surface(img));
        }
        s.image = entry.image;
        s.img_width = entry.img_width;
        s.img_height = entry.img_height;
        s.annotations = entry.annotations;
        s.shape_annotations = entry.shape_annotations;
        s.draft_pos = None; s.draft_center = None; s.draft_text.clear();
        s.selected_ann = None; s.property_undo_pushed = false;
        s.selected_shape = None; s.shape_property_undo_pushed = false;
        let (iw, ih) = (s.img_width, s.img_height);
        drop(s);
        undo_btn.set_sensitive(true);
        let title = window.title().unwrap_or_default();
        let name = title.trim_end_matches(" — Preview").to_string();
        status_label.set_markup(&format!("<b>{}</b>  {}×{} px", glib::markup_escape_text(&name), iw, ih));
        apply_zoom();
    })
}
