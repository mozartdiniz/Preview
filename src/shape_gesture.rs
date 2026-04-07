use gtk4::prelude::*;
use gtk4::gdk;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::annotation::{ShapeAnnotation, ShapeKind};
use crate::coords;
use crate::hit_test::{self, ShapeHandle};
use crate::state::State;

// ── Drag mode ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum DragMode { Draw, Move, ResizeP1, ResizeP2 }

// ── Click — selection ─────────────────────────────────────────────────────────

pub fn setup_shape_click(
    canvas_overlay: &gtk4::Overlay,
    canvas: &gtk4::DrawingArea,
    state: Rc<RefCell<State>>,
    set_shape_tool: Rc<dyn Fn(Option<ShapeKind>)>,
    shape_color_btn: gtk4::ColorDialogButton,
    shape_stroke_spin: gtk4::SpinButton,
) {
    let click = gtk4::GestureClick::new();

    click.connect_released({
        let state = state.clone();
        let canvas = canvas.clone();
        move |g, _n, x, y| {
            // Don't handle if no image
            if !state.borrow().has_image() {
                g.set_state(gtk4::EventSequenceState::Denied);
                return;
            }
            // Suppress if drag already handled this press
            if state.borrow().shape_drag_was_active {
                state.borrow_mut().shape_drag_was_active = false;
                return;
            }

            let (ox, oy, scale) = {
                let s = state.borrow();
                let vw = canvas.width() as f64; let vh = canvas.height() as f64;
                if s.fit_mode { coords::fit_transform(s.img_width, s.img_height, vw, vh) }
                else { (0.0, 0.0, s.zoom) }
            };
            let ix = (x - ox) / scale;
            let iy = (y - oy) / scale;

            let (shape_active, hit) = {
                let s = state.borrow();
                let h = hit_test::hit_test_shape(&s.shape_annotations, ix, iy, scale);
                (s.shape_tool_active, h)
            };

            if let Some(idx) = hit {
                // Activate shape mode if needed, using the hit shape's kind
                if !shape_active {
                    let kind = state.borrow().shape_annotations[idx].kind;
                    set_shape_tool(Some(kind));
                }
                // Select the shape, sync toolbar
                {
                    let mut s = state.borrow_mut();
                    if s.selected_shape != Some(idx) {
                        s.shape_property_undo_pushed = false;
                    }
                    s.selected_shape = Some(idx);
                }
                let (color, stroke) = {
                    let s = state.borrow();
                    let sh = &s.shape_annotations[idx];
                    (sh.color, sh.stroke_width)
                };
                shape_color_btn.set_rgba(&gdk::RGBA::new(
                    color.0 as f32, color.1 as f32, color.2 as f32, color.3 as f32,
                ));
                shape_stroke_spin.set_value(stroke);
                // Update state.shape_color / stroke_width to match selection
                {
                    let mut s = state.borrow_mut();
                    s.shape_color = color;
                    s.shape_stroke_width = stroke;
                }
                canvas.queue_draw();
            } else if shape_active {
                // Click on empty area → deselect
                let mut s = state.borrow_mut();
                s.selected_shape = None;
                s.shape_property_undo_pushed = false;
                drop(s);
                canvas.queue_draw();
            } else {
                g.set_state(gtk4::EventSequenceState::Denied);
            }
        }
    });

    canvas_overlay.add_controller(click);
}

// ── Drag — draw / move / resize ───────────────────────────────────────────────

pub fn setup_shape_drag(
    canvas_overlay: &gtk4::Overlay,
    canvas: &gtk4::DrawingArea,
    state: Rc<RefCell<State>>,
    undo_btn: gtk4::Button,
) {
    let drag = gtk4::GestureDrag::new();
    let drag_mode: Rc<Cell<DragMode>> = Rc::new(Cell::new(DragMode::Draw));
    // Original coords stored for move/resize
    let origin: Rc<Cell<(f64, f64, f64, f64)>> = Rc::new(Cell::new((0.0, 0.0, 0.0, 0.0)));

    drag.connect_drag_begin({
        let state = state.clone();
        let canvas = canvas.clone();
        let undo_btn = undo_btn.clone();
        let drag_mode = drag_mode.clone();
        let origin = origin.clone();
        move |g, x, y| {
            let (active, has_image) = {
                let s = state.borrow();
                (s.shape_tool_active, s.has_image())
            };
            if !active || !has_image {
                g.set_state(gtk4::EventSequenceState::Denied);
                return;
            }

            let (ox, oy, scale) = {
                let s = state.borrow();
                let vw = canvas.width() as f64; let vh = canvas.height() as f64;
                if s.fit_mode { coords::fit_transform(s.img_width, s.img_height, vw, vh) }
                else { (0.0, 0.0, s.zoom) }
            };
            let ix = (x - ox) / scale;
            let iy = (y - oy) / scale;

            // Check handles first (if something is selected)
            let selected = state.borrow().selected_shape;
            if let Some(idx) = selected {
                let shape = state.borrow().shape_annotations.get(idx).cloned();
                if let Some(ref shape) = shape {
                    if let Some(handle) = hit_test::hit_test_shape_handle(shape, ix, iy, scale) {
                        let (x1, y1, x2, y2) = (shape.x1, shape.y1, shape.x2, shape.y2);
                        origin.set((x1, y1, x2, y2));
                        drag_mode.set(match handle {
                            ShapeHandle::P1 => DragMode::ResizeP1,
                            ShapeHandle::P2 => DragMode::ResizeP2,
                        });
                        state.borrow_mut().push_undo();
                        undo_btn.set_sensitive(true);
                        state.borrow_mut().shape_drag_was_active = true;
                        g.set_state(gtk4::EventSequenceState::Claimed);
                        return;
                    }
                    // Check body hit → Move
                    if hit_test::hit_test_shape(
                        &state.borrow().shape_annotations, ix, iy, scale
                    ) == Some(idx) {
                        let (x1, y1, x2, y2) = (shape.x1, shape.y1, shape.x2, shape.y2);
                        origin.set((x1, y1, x2, y2));
                        drag_mode.set(DragMode::Move);
                        state.borrow_mut().push_undo();
                        undo_btn.set_sensitive(true);
                        state.borrow_mut().shape_drag_was_active = true;
                        g.set_state(gtk4::EventSequenceState::Claimed);
                        return;
                    }
                }
            }

            // No hit on selected shape — draw a new one
            drag_mode.set(DragMode::Draw);
            {
                let mut s = state.borrow_mut();
                s.push_undo();
                s.shape_draft = Some((ix, iy, ix, iy));
                s.selected_shape = None;
            }
            undo_btn.set_sensitive(true);
            g.set_state(gtk4::EventSequenceState::Claimed);
            canvas.queue_draw();
        }
    });

    drag.connect_drag_update({
        let state = state.clone();
        let canvas = canvas.clone();
        let drag_mode = drag_mode.clone();
        let origin = origin.clone();
        move |g, dx, dy| {
            if !state.borrow().shape_tool_active { return; }

            let (ox, oy, scale) = {
                let s = state.borrow();
                let vw = canvas.width() as f64; let vh = canvas.height() as f64;
                if s.fit_mode { coords::fit_transform(s.img_width, s.img_height, vw, vh) }
                else { (0.0, 0.0, s.zoom) }
            };
            let (img_w, img_h) = {
                let s = state.borrow();
                (s.img_width as f64, s.img_height as f64)
            };

            match drag_mode.get() {
                DragMode::Draw => {
                    let Some((sx, sy)) = g.start_point() else { return };
                    let x2 = ((sx + dx - ox) / scale).clamp(0.0, img_w);
                    let y2 = ((sy + dy - oy) / scale).clamp(0.0, img_h);
                    let mut s = state.borrow_mut();
                    if let Some(ref mut draft) = s.shape_draft {
                        draft.2 = x2; draft.3 = y2;
                    }
                }
                DragMode::Move => {
                    let (ox1, oy1, ox2, oy2) = origin.get();
                    let delta_x = dx / scale; let delta_y = dy / scale;
                    let mut s = state.borrow_mut();
                    if let Some(idx) = s.selected_shape {
                        if let Some(shape) = s.shape_annotations.get_mut(idx) {
                            shape.x1 = ox1 + delta_x; shape.y1 = oy1 + delta_y;
                            shape.x2 = ox2 + delta_x; shape.y2 = oy2 + delta_y;
                        }
                    }
                }
                DragMode::ResizeP1 => {
                    let (ox1, oy1, _, _) = origin.get();
                    let delta_x = dx / scale; let delta_y = dy / scale;
                    let mut s = state.borrow_mut();
                    if let Some(idx) = s.selected_shape {
                        if let Some(shape) = s.shape_annotations.get_mut(idx) {
                            shape.x1 = (ox1 + delta_x).clamp(0.0, img_w);
                            shape.y1 = (oy1 + delta_y).clamp(0.0, img_h);
                        }
                    }
                }
                DragMode::ResizeP2 => {
                    let (_, _, ox2, oy2) = origin.get();
                    let delta_x = dx / scale; let delta_y = dy / scale;
                    let mut s = state.borrow_mut();
                    if let Some(idx) = s.selected_shape {
                        if let Some(shape) = s.shape_annotations.get_mut(idx) {
                            shape.x2 = (ox2 + delta_x).clamp(0.0, img_w);
                            shape.y2 = (oy2 + delta_y).clamp(0.0, img_h);
                        }
                    }
                }
            }
            canvas.queue_draw();
        }
    });

    drag.connect_drag_end({
        let state = state.clone();
        let canvas = canvas.clone();
        move |_, _, _| {
            let mut s = state.borrow_mut();
            if let DragMode::Draw = drag_mode.get() {
                if let Some((x1, y1, x2, y2)) = s.shape_draft.take() {
                    let shape = ShapeAnnotation {
                        kind: s.shape_kind,
                        x1, y1, x2, y2,
                        color: s.shape_color,
                        stroke_width: s.shape_stroke_width,
                    };
                    let idx = s.shape_annotations.len();
                    s.shape_annotations.push(shape);
                    s.selected_shape = Some(idx);
                }
            }
            drop(s);
            canvas.queue_draw();
        }
    });

    canvas_overlay.add_controller(drag);
}
