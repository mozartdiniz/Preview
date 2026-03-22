use gtk4::prelude::*;
use gtk4::gio;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

// ── Resize dialog ─────────────────────────────────────────────────────────────

pub fn show_resize_dialog(
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

pub fn show_save_dialog(
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

pub fn show_error(parent: &adw::ApplicationWindow, title: &str, detail: &str) {
    let dialog = gtk4::AlertDialog::builder()
        .message(title)
        .detail(detail)
        .buttons(["OK"])
        .build();
    dialog.show(Some(parent));
}
