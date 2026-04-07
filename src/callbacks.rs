use gtk4::prelude::*;
use gtk4::{gdk, gio};
use std::cell::RefCell;
use std::rc::Rc;

use crate::annotation::ShapeKind;
use crate::dialogs;
use crate::state::State;
use crate::transforms;
use crate::widgets::Widgets;
use crate::Closures;

pub fn connect(w: &Widgets, state: Rc<RefCell<State>>, c: &Closures) {
    w.open_btn.connect_clicked({
        let show_open_dialog = c.show_open_dialog.clone();
        move |_| show_open_dialog()
    });

    w.zoom_in_btn.connect_clicked({
        let state = state.clone(); let apply_zoom = c.apply_zoom.clone();
        move |_| {
            let mut s = state.borrow_mut();
            if s.fit_mode { s.fit_mode = false; }
            s.zoom = (s.zoom * 1.25).min(32.0); drop(s); apply_zoom();
        }
    });
    w.zoom_out_btn.connect_clicked({
        let state = state.clone(); let apply_zoom = c.apply_zoom.clone();
        move |_| {
            let mut s = state.borrow_mut();
            if s.fit_mode { s.fit_mode = false; }
            s.zoom = (s.zoom / 1.25).max(0.05); drop(s); apply_zoom();
        }
    });
    w.zoom_fit_btn.connect_clicked({
        let state = state.clone(); let apply_zoom = c.apply_zoom.clone();
        move |_| { state.borrow_mut().fit_mode = true; apply_zoom(); }
    });
    w.zoom_orig_btn.connect_clicked({
        let state = state.clone(); let apply_zoom = c.apply_zoom.clone();
        move |_| {
            let mut s = state.borrow_mut();
            s.zoom = 1.0; s.fit_mode = false; drop(s); apply_zoom();
        }
    });

    w.resize_btn.connect_clicked({
        let state = state.clone();
        let window = w.window.clone();
        let update_image = c.update_image.clone();
        let set_text_mode = c.set_text_mode.clone();
        move |_| {
            set_text_mode(false);
            let (iw, ih) = { let s = state.borrow(); (s.img_width as u32, s.img_height as u32) };
            let state = state.clone(); let update_image = update_image.clone();
            dialogs::show_resize_dialog(&window, iw, ih, move |nw, nh| {
                let img = state.borrow().image.as_ref().map(|i| i.clone());
                if let Some(img) = img { update_image(transforms::resize(&img, nw, nh)); }
            });
        }
    });

    w.rotate_cw_btn.connect_clicked({
        let state = state.clone(); let update_image = c.update_image.clone();
        let set_text_mode = c.set_text_mode.clone();
        move |_| {
            set_text_mode(false);
            let img = state.borrow().image.as_ref().map(|i| i.clone());
            if let Some(img) = img { update_image(transforms::rotate_cw(&img)); }
        }
    });
    w.rotate_ccw_btn.connect_clicked({
        let state = state.clone(); let update_image = c.update_image.clone();
        let set_text_mode = c.set_text_mode.clone();
        move |_| {
            set_text_mode(false);
            let img = state.borrow().image.as_ref().map(|i| i.clone());
            if let Some(img) = img { update_image(transforms::rotate_ccw(&img)); }
        }
    });
    w.flip_h_btn.connect_clicked({
        let state = state.clone(); let update_image = c.update_image.clone();
        let set_text_mode = c.set_text_mode.clone();
        move |_| {
            set_text_mode(false);
            let img = state.borrow().image.as_ref().map(|i| i.clone());
            if let Some(img) = img { update_image(transforms::flip_h(&img)); }
        }
    });
    w.flip_v_btn.connect_clicked({
        let state = state.clone(); let update_image = c.update_image.clone();
        let set_text_mode = c.set_text_mode.clone();
        move |_| {
            set_text_mode(false);
            let img = state.borrow().image.as_ref().map(|i| i.clone());
            if let Some(img) = img { update_image(transforms::flip_v(&img)); }
        }
    });

    w.undo_btn.connect_clicked({
        let undo = c.undo.clone();
        move |_| undo()
    });

    w.crop_btn.connect_clicked({
        let set_crop_mode = c.set_crop_mode.clone();
        move |_| set_crop_mode(true)
    });
    w.cancel_crop_btn.connect_clicked({
        let set_crop_mode = c.set_crop_mode.clone();
        move |_| set_crop_mode(false)
    });
    w.text_btn.connect_clicked({
        let set_text_mode = c.set_text_mode.clone();
        move |_| set_text_mode(true)
    });
    w.done_text_btn.connect_clicked({
        let set_text_mode = c.set_text_mode.clone();
        move |_| set_text_mode(false)
    });

    w.rect_btn.connect_clicked({
        let set_shape_tool = c.set_shape_tool.clone();
        move |_| set_shape_tool(Some(ShapeKind::Rect))
    });
    w.line_btn.connect_clicked({
        let set_shape_tool = c.set_shape_tool.clone();
        move |_| set_shape_tool(Some(ShapeKind::Line))
    });
    w.arrow_btn.connect_clicked({
        let set_shape_tool = c.set_shape_tool.clone();
        move |_| set_shape_tool(Some(ShapeKind::Arrow))
    });
    w.shape_done_btn.connect_clicked({
        let set_shape_tool = c.set_shape_tool.clone();
        move |_| set_shape_tool(None)
    });

    // Scroll-wheel zoom (Ctrl + scroll) — on canvas_overlay so the ScrolledWindow
    // handles all native trackpad/horizontal events without interference.
    let scroll_ctrl = gtk4::EventControllerScroll::new(
        gtk4::EventControllerScrollFlags::VERTICAL | gtk4::EventControllerScrollFlags::HORIZONTAL,
    );
    scroll_ctrl.connect_scroll({
        let state = state.clone(); let apply_zoom = c.apply_zoom.clone();
        move |ctrl, _dx, dy| {
            if !ctrl.current_event_state().contains(gdk::ModifierType::CONTROL_MASK) {
                return glib::Propagation::Proceed;
            }
            let mut s = state.borrow_mut();
            if !s.has_image() { return glib::Propagation::Proceed; }
            if s.fit_mode { s.fit_mode = false; }
            if dy < 0.0 { s.zoom = (s.zoom * 1.15).min(32.0); }
            else         { s.zoom = (s.zoom / 1.15).max(0.05); }
            drop(s); apply_zoom();
            glib::Propagation::Stop
        }
    });
    w.canvas_overlay.add_controller(scroll_ctrl);

    // Pinch-to-zoom
    let pinch = gtk4::GestureZoom::new();
    let pinch_start_zoom: Rc<RefCell<f64>> = Rc::new(RefCell::new(1.0));
    pinch.connect_begin({
        let state = state.clone();
        let pinch_start_zoom = pinch_start_zoom.clone();
        move |_, _| { *pinch_start_zoom.borrow_mut() = state.borrow().zoom; }
    });
    pinch.connect_scale_changed({
        let state = state.clone(); let apply_zoom = c.apply_zoom.clone();
        let pinch_start_zoom = pinch_start_zoom.clone();
        move |_, scale| {
            let mut s = state.borrow_mut();
            if !s.has_image() { return; }
            if s.fit_mode { s.fit_mode = false; }
            s.zoom = (*pinch_start_zoom.borrow() * scale).clamp(0.05, 32.0);
            drop(s); apply_zoom();
        }
    });
    w.canvas_overlay.add_controller(pinch);

    // Drag-and-drop
    let drop_target = gtk4::DropTarget::new(gio::File::static_type(), gdk::DragAction::COPY);
    drop_target.connect_drop({
        let load_image_file = c.load_image_file.clone();
        move |_, value, _, _| {
            if let Ok(file) = value.get::<gio::File>() {
                if let Some(path) = file.path() { load_image_file(&path); return true; }
            }
            false
        }
    });
    w.window.add_controller(drop_target);
}

use gtk4::glib;
