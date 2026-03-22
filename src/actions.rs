use gtk4::prelude::*;
use gtk4::gio;
use std::cell::RefCell;
use std::rc::Rc;

use crate::annotation;
use crate::dialogs;
use crate::export;
use crate::state::State;
use crate::widgets::Widgets;
use crate::Closures;

pub fn setup(w: &Widgets, state: Rc<RefCell<State>>, c: &Closures) {
    let app = w.app.clone();
    let window = w.window.clone();

    let act_open = gio::SimpleAction::new("open", None);
    act_open.connect_activate({ let d = c.show_open_dialog.clone(); move |_, _| d() });

    let act_zoom_in = gio::SimpleAction::new("zoom-in", None);
    act_zoom_in.connect_activate({
        let state = state.clone(); let apply_zoom = c.apply_zoom.clone();
        move |_, _| {
            let mut s = state.borrow_mut();
            if s.fit_mode { s.fit_mode = false; }
            s.zoom = (s.zoom * 1.25).min(32.0); drop(s); apply_zoom();
        }
    });

    let act_zoom_out = gio::SimpleAction::new("zoom-out", None);
    act_zoom_out.connect_activate({
        let state = state.clone(); let apply_zoom = c.apply_zoom.clone();
        move |_, _| {
            let mut s = state.borrow_mut();
            if s.fit_mode { s.fit_mode = false; }
            s.zoom = (s.zoom / 1.25).max(0.05); drop(s); apply_zoom();
        }
    });

    let act_zoom_fit = gio::SimpleAction::new("zoom-fit", None);
    act_zoom_fit.connect_activate({
        let state = state.clone(); let apply_zoom = c.apply_zoom.clone();
        move |_, _| { state.borrow_mut().fit_mode = true; apply_zoom(); }
    });

    let act_zoom_orig = gio::SimpleAction::new("zoom-orig", None);
    act_zoom_orig.connect_activate({
        let state = state.clone(); let apply_zoom = c.apply_zoom.clone();
        move |_, _| {
            let mut s = state.borrow_mut();
            s.zoom = 1.0; s.fit_mode = false; drop(s); apply_zoom();
        }
    });

    let act_fullscreen = gio::SimpleAction::new_stateful("fullscreen", None, &false.to_variant());
    act_fullscreen.connect_activate({
        let window = window.clone();
        move |action, _| {
            let is_fs = action.state().and_then(|v| v.get::<bool>()).unwrap_or(false);
            if is_fs { window.unfullscreen(); action.set_state(&false.to_variant()); }
            else      { window.fullscreen();   action.set_state(&true.to_variant()); }
        }
    });

    let act_save = gio::SimpleAction::new("save", None);
    act_save.connect_activate({
        let state = state.clone(); let window = window.clone();
        move |_, _| {
            let (img, annotations, path) = {
                let s = state.borrow();
                (s.image.as_ref().map(|i| i.clone()), s.annotations.clone(), s.file_path.clone())
            };
            if let (Some(img), Some(path)) = (img, path) {
                let flat = annotation::flatten_annotations(&img, &annotations);
                if let Err(e) = export::save_image(&flat, &path) {
                    dialogs::show_error(&window, "Save failed", &e.to_string());
                }
            }
        }
    });

    let act_save_as = gio::SimpleAction::new("save-as", None);
    act_save_as.connect_activate({
        let state = state.clone(); let window = window.clone();
        move |_, _| {
            let (img, annotations, current_path) = {
                let s = state.borrow();
                (s.image.as_ref().map(|i| i.clone()), s.annotations.clone(), s.file_path.clone())
            };
            let Some(img) = img else { return };
            let state = state.clone(); let window = window.clone(); let window2 = window.clone();
            dialogs::show_save_dialog(&window, current_path.as_deref(), None, move |path| {
                let flat = annotation::flatten_annotations(&img, &annotations);
                if let Err(e) = export::save_image(&flat, &path) {
                    dialogs::show_error(&window2, "Save failed", &e.to_string()); return;
                }
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                window2.set_title(Some(&format!("{} — Preview", name)));
                state.borrow_mut().file_path = Some(path);
            });
        }
    });

    let make_export_action = |ext: &'static str| {
        let act = gio::SimpleAction::new(&format!("export-{}", ext), None);
        let state = state.clone(); let window = window.clone();
        act.connect_activate(move |_, _| {
            let (img, annotations, current_path) = {
                let s = state.borrow();
                (s.image.as_ref().map(|i| i.clone()), s.annotations.clone(), s.file_path.clone())
            };
            let Some(img) = img else { return };
            let suggested = current_path.as_deref().and_then(|p| p.file_stem())
                .map(|s| format!("{}.{}", s.to_string_lossy(), ext));
            let window = window.clone(); let window2 = window.clone();
            dialogs::show_save_dialog(&window, None, suggested.as_deref(), move |path| {
                let flat = annotation::flatten_annotations(&img, &annotations);
                if let Err(e) = export::save_image(&flat, &path) {
                    dialogs::show_error(&window2, "Export failed", &e.to_string());
                }
            });
        });
        act
    };
    let act_export_png  = make_export_action("png");
    let act_export_jpeg = make_export_action("jpeg");
    let act_export_webp = make_export_action("webp");

    for a in &[
        act_open.upcast_ref::<gio::Action>(), act_zoom_in.upcast_ref(), act_zoom_out.upcast_ref(),
        act_zoom_fit.upcast_ref(), act_zoom_orig.upcast_ref(), act_fullscreen.upcast_ref(),
        act_save.upcast_ref(), act_save_as.upcast_ref(),
        act_export_png.upcast_ref(), act_export_jpeg.upcast_ref(), act_export_webp.upcast_ref(),
    ] { window.add_action(*a); }

    app.set_accels_for_action("win.open",      &["<Ctrl>o"]);
    app.set_accels_for_action("win.zoom-in",   &["plus", "equal", "<Ctrl>equal"]);
    app.set_accels_for_action("win.zoom-out",  &["minus", "<Ctrl>minus"]);
    app.set_accels_for_action("win.zoom-fit",  &["3", "<Ctrl>0"]);
    app.set_accels_for_action("win.zoom-orig", &["1"]);
    app.set_accels_for_action("win.fullscreen",&["F11"]);
    app.set_accels_for_action("win.save",      &["<Ctrl>s"]);
    app.set_accels_for_action("win.save-as",   &["<Ctrl><Shift>s"]);
    app.set_accels_for_action("app.quit",      &["<Ctrl>q"]);

    let act_undo = gio::SimpleAction::new("undo", None);
    act_undo.connect_activate({ let undo = c.undo.clone(); move |_, _| undo() });

    let act_redo = gio::SimpleAction::new("redo", None);
    act_redo.connect_activate({ let redo = c.redo.clone(); move |_, _| redo() });

    for a in &[act_undo.upcast_ref::<gio::Action>(), act_redo.upcast_ref()] {
        window.add_action(*a);
    }

    app.set_accels_for_action("win.undo", &["<Ctrl>z"]);
    app.set_accels_for_action("win.redo", &["<Ctrl>y", "<Ctrl><Shift>z"]);

    let act_quit = gio::SimpleAction::new("quit", None);
    act_quit.connect_activate({ let app = app.clone(); move |_, _| app.quit() });
    app.add_action(&act_quit);
}
