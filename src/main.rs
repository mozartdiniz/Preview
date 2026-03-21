use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};
use libadwaita as adw;
use std::cell::RefCell;
use std::env;
use std::rc::Rc;

const APP_ID: &str = "com.example.Preview";

// ── App state ────────────────────────────────────────────────────────────────

#[derive(Default)]
struct State {
    zoom: f64,
    img_width: i32,
    img_height: i32,
    fit_mode: bool,
}

impl State {
    fn new() -> Self {
        Self {
            zoom: 1.0,
            fit_mode: true,
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

// ── UI construction ───────────────────────────────────────────────────────────

fn build_ui(app: &adw::Application, initial_file: Option<&std::path::Path>) {
    let state = Rc::new(RefCell::new(State::new()));

    // ── Image display ────────────────────────────────────────────────────────
    let picture = gtk4::Picture::new();
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk4::ContentFit::Contain);
    picture.set_hexpand(true);
    picture.set_vexpand(true);

    // Checkerboard-ish background so transparency is visible
    picture.add_css_class("image-view");

    let scrolled = gtk4::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .build();
    scrolled.set_child(Some(&picture));

    // ── Status bar ───────────────────────────────────────────────────────────
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

    // ── Header bar buttons ───────────────────────────────────────────────────
    let open_btn = gtk4::Button::builder()
        .icon_name("document-open-symbolic")
        .tooltip_text("Open Image (Ctrl+O)")
        .build();

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

    let zoom_group = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    zoom_group.add_css_class("linked");
    zoom_group.append(&zoom_out_btn);
    zoom_group.append(&zoom_fit_btn);
    zoom_group.append(&zoom_orig_btn);
    zoom_group.append(&zoom_in_btn);

    let header = adw::HeaderBar::new();
    header.pack_start(&open_btn);
    header.pack_end(&zoom_group);

    // ── Layout ───────────────────────────────────────────────────────────────
    let status_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    status_box.append(&status_label);
    status_box.append(&zoom_label);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.append(&scrolled);
    content.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    content.append(&status_box);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&content));

    // ── Window ───────────────────────────────────────────────────────────────
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Preview")
        .default_width(1024)
        .default_height(768)
        .content(&toolbar_view)
        .build();

    // ── Shared closures ──────────────────────────────────────────────────────

    // apply_zoom: re-renders the picture at the current zoom level
    let apply_zoom: Rc<dyn Fn()> = Rc::new({
        let picture = picture.clone();
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
                picture.set_can_shrink(true);
                picture.set_content_fit(gtk4::ContentFit::Contain);
                picture.set_size_request(-1, -1);
                zoom_label.set_text("Fit");
            } else {
                let w = (s.img_width as f64 * s.zoom).round() as i32;
                let h = (s.img_height as f64 * s.zoom).round() as i32;
                picture.set_can_shrink(false);
                picture.set_content_fit(gtk4::ContentFit::Contain);
                picture.set_size_request(w, h);
                zoom_label.set_text(&format!("{:.0}%", s.zoom * 100.0));
            }
            zoom_out_btn.set_sensitive(true);
            zoom_in_btn.set_sensitive(true);
        }
    });

    // ── Open file logic ──────────────────────────────────────────────────────

    let load_image = {
        let window = window.clone();
        let picture = picture.clone();
        let status_label = status_label.clone();
        let state = state.clone();
        let apply_zoom = apply_zoom.clone();
        let zoom_fit_btn = zoom_fit_btn.clone();
        let zoom_orig_btn = zoom_orig_btn.clone();

        Rc::new(move |file: gio::File| {
            let Some(path) = file.path() else { return };

            // Use GdkTexture to load image and retrieve dimensions
            match gdk::Texture::from_filename(&path) {
                Ok(texture) => {
                    let w = texture.width();
                    let h = texture.height();

                    {
                        let mut s = state.borrow_mut();
                        s.img_width = w;
                        s.img_height = h;
                        s.zoom = 1.0;
                        s.fit_mode = true;
                    }

                    picture.set_paintable(Some(&texture));
                    zoom_fit_btn.set_sensitive(true);
                    zoom_orig_btn.set_sensitive(true);

                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();

                    window.set_title(Some(&format!("{} — Preview", name)));
                    status_label.set_markup(&format!(
                        "<b>{}</b>  {}×{} px",
                        glib::markup_escape_text(&name),
                        w,
                        h
                    ));
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
        })
    };

    // Show file picker
    let show_open_dialog = {
        let window = window.clone();
        let load_image = load_image.clone();

        Rc::new(move || {
            let image_filter = gtk4::FileFilter::new();
            image_filter.set_name(Some("Images"));
            for mime in &[
                "image/png",
                "image/jpeg",
                "image/gif",
                "image/bmp",
                "image/tiff",
                "image/webp",
                "image/svg+xml",
            ] {
                image_filter.add_mime_type(mime);
            }

            let filters = gio::ListStore::new::<gtk4::FileFilter>();
            filters.append(&image_filter);

            let dialog = gtk4::FileDialog::builder()
                .title("Open Image")
                .modal(true)
                .filters(&filters)
                .build();

            let window = window.clone();
            let load_image = load_image.clone();

            dialog.open(
                Some(&window),
                gio::Cancellable::NONE,
                move |result| {
                    if let Ok(file) = result {
                        load_image(file);
                    }
                },
            );
        })
    };

    // ── Button callbacks ─────────────────────────────────────────────────────

    open_btn.connect_clicked({
        let show_open_dialog = show_open_dialog.clone();
        move |_| show_open_dialog()
    });

    zoom_in_btn.connect_clicked({
        let state = state.clone();
        let apply_zoom = apply_zoom.clone();
        move |_| {
            let mut s = state.borrow_mut();
            if s.fit_mode {
                s.fit_mode = false;
            }
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
            if s.fit_mode {
                s.fit_mode = false;
            }
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

    // ── Mouse wheel zoom (Ctrl + scroll) ─────────────────────────────────────

    let scroll_ctrl = gtk4::EventControllerScroll::new(
        gtk4::EventControllerScrollFlags::VERTICAL
            | gtk4::EventControllerScrollFlags::DISCRETE,
    );
    scroll_ctrl.connect_scroll({
        let state = state.clone();
        let apply_zoom = apply_zoom.clone();
        move |ctrl, _dx, dy| {
            let modifiers = ctrl.current_event_state();
            if !modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
                return glib::Propagation::Proceed;
            }
            let mut s = state.borrow_mut();
            if !s.has_image() {
                return glib::Propagation::Proceed;
            }
            if s.fit_mode {
                s.fit_mode = false;
            }
            if dy < 0.0 {
                s.zoom = (s.zoom * 1.15).min(32.0);
            } else {
                s.zoom = (s.zoom / 1.15).max(0.05);
            }
            drop(s);
            apply_zoom();
            glib::Propagation::Stop
        }
    });
    scrolled.add_controller(scroll_ctrl);

    // ── Drag and drop ────────────────────────────────────────────────────────

    let drop_target = gtk4::DropTarget::new(gio::File::static_type(), gdk::DragAction::COPY);
    drop_target.connect_drop({
        let load_image = load_image.clone();
        move |_, value, _, _| {
            if let Ok(file) = value.get::<gio::File>() {
                load_image(file);
                return true;
            }
            false
        }
    });
    window.add_controller(drop_target);

    // ── Window-level actions (keyboard shortcuts) ─────────────────────────────

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
            if s.fit_mode {
                s.fit_mode = false;
            }
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
            if s.fit_mode {
                s.fit_mode = false;
            }
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

    let act_fullscreen = gio::SimpleAction::new_stateful(
        "fullscreen",
        None,
        &false.to_variant(),
    );
    act_fullscreen.connect_activate({
        let window = window.clone();
        move |action, _| {
            let is_fs = action
                .state()
                .and_then(|v| v.get::<bool>())
                .unwrap_or(false);
            if is_fs {
                window.unfullscreen();
                action.set_state(&false.to_variant());
            } else {
                window.fullscreen();
                action.set_state(&true.to_variant());
            }
        }
    });

    window.add_action(&act_open);
    window.add_action(&act_zoom_in);
    window.add_action(&act_zoom_out);
    window.add_action(&act_zoom_fit);
    window.add_action(&act_zoom_orig);
    window.add_action(&act_fullscreen);

    app.set_accels_for_action("win.open", &["<Ctrl>o"]);
    app.set_accels_for_action("win.zoom-in", &["plus", "equal", "<Ctrl>equal"]);
    app.set_accels_for_action("win.zoom-out", &["minus", "<Ctrl>minus"]);
    app.set_accels_for_action("win.zoom-fit", &["3", "<Ctrl>0"]);
    app.set_accels_for_action("win.zoom-orig", &["1"]);
    app.set_accels_for_action("win.fullscreen", &["F11"]);
    app.set_accels_for_action("app.quit", &["<Ctrl>q"]);

    let act_quit = gio::SimpleAction::new("quit", None);
    act_quit.connect_activate({
        let app = app.clone();
        move |_, _| app.quit()
    });
    app.add_action(&act_quit);

    // ── Open initial file if given on command line ────────────────────────────

    if let Some(path) = initial_file {
        let file = gio::File::for_path(path);
        load_image(file);
    }

    window.present();
}
