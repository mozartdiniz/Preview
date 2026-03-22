mod annotation;
mod coords;
mod dialogs;
mod export;
mod hit_test;
mod state;
mod transforms;
mod widgets;
mod draw;
mod image_closures;
mod text_closures;
mod text_gesture;
mod crop_gesture;
mod callbacks;
mod text_callbacks;
mod actions;

use gtk4::prelude::*;
use gtk4::{gio, glib};
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::env;
use std::rc::Rc;

use crate::state::State;

const APP_ID: &str = "com.example.Preview";

pub struct Closures {
    pub apply_zoom: Rc<dyn Fn()>,
    pub update_image: Rc<dyn Fn(image::DynamicImage)>,
    pub load_image_file: Rc<dyn Fn(&std::path::Path)>,
    pub show_open_dialog: Rc<dyn Fn()>,
    pub set_crop_mode: Rc<dyn Fn(bool)>,
    pub start_blink: Rc<dyn Fn()>,
    pub stop_blink: Rc<dyn Fn()>,
    pub commit_draft: Rc<dyn Fn()>,
    pub set_text_mode: Rc<dyn Fn(bool)>,
    pub undo: Rc<dyn Fn()>,
    pub redo: Rc<dyn Fn()>,
}

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

fn build_ui(app: &adw::Application, initial_file: Option<&std::path::Path>) {
    let state = Rc::new(RefCell::new(State::new()));
    let w = widgets::build(app);

    let cursor_blink_on = Rc::new(Cell::new(false));
    let draft_cursor_pos: Rc<Cell<i32>> = Rc::new(Cell::new(0));
    let blink_source: Rc<Cell<Option<glib::SourceId>>> = Rc::new(Cell::new(None));

    draw::setup(&w.canvas, state.clone(), cursor_blink_on.clone(), draft_cursor_pos.clone());

    let apply_zoom = image_closures::make_apply_zoom(&w, state.clone());
    let update_image = image_closures::make_update_image(&w, state.clone(), apply_zoom.clone());
    let load_image_file = image_closures::make_load_image_file(&w, state.clone(), apply_zoom.clone(), update_image.clone());
    let show_open_dialog = image_closures::make_show_open_dialog(w.window.clone(), load_image_file.clone());
    let set_crop_mode = text_closures::make_set_crop_mode(&w, state.clone(), apply_zoom.clone());
    let (start_blink, stop_blink) = text_closures::make_blink_handlers(w.canvas.clone(), cursor_blink_on.clone(), blink_source);
    let commit_draft = text_closures::make_commit_draft(&w, state.clone(), stop_blink.clone());
    let set_text_mode = text_closures::make_set_text_mode(&w, state.clone(), commit_draft.clone());
    let undo = image_closures::make_undo(&w, state.clone(), apply_zoom.clone());
    let redo = image_closures::make_redo(&w, state.clone(), apply_zoom.clone());

    let c = Closures {
        apply_zoom, update_image, load_image_file, show_open_dialog,
        set_crop_mode, start_blink, stop_blink, commit_draft, set_text_mode,
        undo, redo,
    };

    text_gesture::setup_text_click(&w, state.clone(), c.commit_draft.clone(), c.set_text_mode.clone(), c.start_blink.clone(), draft_cursor_pos.clone());
    text_gesture::setup_text_drag(&w, state.clone());
    crop_gesture::setup(&w.canvas, state.clone(), w.apply_crop_btn.clone());
    callbacks::connect(&w, state.clone(), &c);
    text_callbacks::connect(&w, state.clone(), &c, draft_cursor_pos);
    actions::setup(&w, state.clone(), &c);

    if let Some(path) = initial_file {
        (c.load_image_file)(path);
    }
    w.window.present();
}
