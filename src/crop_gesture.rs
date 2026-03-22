use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::coords;
use crate::state::State;

pub fn setup(
    canvas: &gtk4::DrawingArea,
    state: Rc<RefCell<State>>,
    apply_crop_btn: gtk4::Button,
) {
    let drag = gtk4::GestureDrag::new();
    drag.set_exclusive(true);

    drag.connect_drag_begin({
        let state = state.clone();
        let canvas = canvas.clone();
        let apply_crop_btn = apply_crop_btn.clone();
        move |g, x, y| {
            if !state.borrow().in_crop { g.set_state(gtk4::EventSequenceState::Denied); return; }
            let mut s = state.borrow_mut();
            let vw = canvas.width() as f64; let vh = canvas.height() as f64;
            let (ox, oy, scale) = coords::fit_transform(s.img_width, s.img_height, vw, vh);
            let cx = x.clamp(ox, ox + s.img_width as f64 * scale);
            let cy = y.clamp(oy, oy + s.img_height as f64 * scale);
            s.drag_start = Some((cx, cy)); s.drag_end = Some((cx, cy));
            apply_crop_btn.set_sensitive(false);
            drop(s); canvas.queue_draw();
        }
    });

    drag.connect_drag_update({
        let state = state.clone();
        let canvas = canvas.clone();
        move |g, dx, dy| {
            if !state.borrow().in_crop { return; }
            let Some((sx, sy)) = g.start_point() else { return };
            let mut s = state.borrow_mut();
            let vw = canvas.width() as f64; let vh = canvas.height() as f64;
            let (ox, oy, scale) = coords::fit_transform(s.img_width, s.img_height, vw, vh);
            let ex = (sx + dx).clamp(ox, ox + s.img_width as f64 * scale);
            let ey = (sy + dy).clamp(oy, oy + s.img_height as f64 * scale);
            s.drag_end = Some((ex, ey));
            drop(s); canvas.queue_draw();
        }
    });

    drag.connect_drag_end({
        let state = state.clone();
        let canvas = canvas.clone();
        let apply_crop_btn = apply_crop_btn.clone();
        move |g, dx, dy| {
            if !state.borrow().in_crop { return; }
            let Some((sx, sy)) = g.start_point() else { return };
            let mut s = state.borrow_mut();
            let vw = canvas.width() as f64; let vh = canvas.height() as f64;
            let (ox, oy, scale) = coords::fit_transform(s.img_width, s.img_height, vw, vh);
            let ex = (sx + dx).clamp(ox, ox + s.img_width as f64 * scale);
            let ey = (sy + dy).clamp(oy, oy + s.img_height as f64 * scale);
            s.drag_end = Some((ex, ey));
            let valid = s.drag_start.zip(s.drag_end)
                .map(|((ax, ay), (bx, by))| (ax - bx).abs() > 4.0 && (ay - by).abs() > 4.0)
                .unwrap_or(false);
            drop(s);
            apply_crop_btn.set_sensitive(valid);
            canvas.queue_draw();
        }
    });

    canvas.add_controller(drag);
}
