use gtk4::prelude::*;
use gtk4::{gdk, glib};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::coords;
use crate::hit_test;
use crate::state::State;
use crate::widgets::Widgets;

pub fn setup_text_click(
    w: &Widgets,
    state: Rc<RefCell<State>>,
    commit_draft: Rc<dyn Fn()>,
    set_text_mode: Rc<dyn Fn(bool)>,
    start_blink: Rc<dyn Fn()>,
    draft_cursor_pos: Rc<Cell<i32>>,
) {
    let canvas = w.canvas.clone();
    let draft_entry = w.draft_entry.clone();
    let font_btn = w.font_btn.clone();
    let color_btn = w.color_btn.clone();
    let rotation_spin = w.rotation_spin.clone();

    let text_click = gtk4::GestureClick::new();
    text_click.connect_released({
        let state = state.clone();
        let canvas = canvas.clone();
        let draft_entry = draft_entry.clone();
        let commit_draft = commit_draft.clone();
        let set_text_mode = set_text_mode.clone();
        let start_blink = start_blink.clone();
        let draft_cursor_pos = draft_cursor_pos.clone();
        move |g, n, x, y| {
            let (active, has_image, fit_mode, img_w, img_h) = {
                let s = state.borrow();
                (s.text_tool_active, s.image.is_some(), s.fit_mode, s.img_width, s.img_height)
            };
            if !has_image { g.set_state(gtk4::EventSequenceState::Denied); return; }
            if !active {
                let has_hit = {
                    let vw = canvas.width() as f64; let vh = canvas.height() as f64;
                    let s = state.borrow();
                    let (ox, oy, scale) = if s.fit_mode { coords::fit_transform(s.img_width, s.img_height, vw, vh) } else { (0.0, 0.0, s.zoom) };
                    hit_test::hit_test_annotation(&s.annotations, x, y, ox, oy, scale, &canvas.pango_context()).is_some()
                };
                if has_hit { set_text_mode(true); } else { g.set_state(gtk4::EventSequenceState::Denied); return; }
            }
            let vw = canvas.width() as f64; let vh = canvas.height() as f64;
            let (ox, oy, scale) = if fit_mode {
                coords::fit_transform(img_w, img_h, vw, vh)
            } else { (0.0, 0.0, state.borrow().zoom) };
            let hit = {
                let s = state.borrow();
                hit_test::hit_test_annotation(&s.annotations, x, y, ox, oy, scale, &canvas.pango_context())
            };
            if let Some(idx) = hit {
                commit_draft();
                if n >= 2 {
                    let (ann_x, ann_y, ann_text) = {
                        let s = state.borrow();
                        let ann = &s.annotations[idx];
                        (ann.x, ann.y, ann.text.clone())
                    };
                    let (ann_cx, ann_cy) = {
                        let pc = canvas.pango_context();
                        let layout = gtk4::pango::Layout::new(&pc);
                        let s = state.borrow();
                        let fd = s.annotations[idx].font_desc.clone();
                        drop(s);
                        layout.set_font_description(Some(&fd));
                        layout.set_text(&ann_text);
                        let (tw, th) = layout.pixel_size();
                        (ann_x + tw as f64 / scale / 2.0, ann_y + th as f64 / scale / 2.0)
                    };
                    {
                        let mut s = state.borrow_mut();
                        s.annotations.remove(idx);
                        s.draft_pos = Some((ann_x, ann_y));
                        s.draft_center = Some((ann_cx, ann_cy));
                        s.draft_text = ann_text.clone();
                        s.selected_ann = None;
                    }
                    draft_entry.set_text(&ann_text);
                    let sx = (ox + ann_x * scale) as i32;
                    let sy = (oy + ann_y * scale) as i32;
                    let ent = draft_entry.clone();
                    let blink = start_blink.clone();
                    let cpos = draft_cursor_pos.clone();
                    glib::idle_add_local_once(move || {
                        ent.set_margin_start(sx); ent.set_margin_top(sy);
                        ent.set_visible(true); ent.grab_focus();
                        let end = ent.text().chars().count() as i32;
                        ent.select_region(end, end); cpos.set(end); blink();
                    });
                } else {
                    let (ann_font, ann_color, ann_rotation) = {
                        let s = state.borrow();
                        let ann = &s.annotations[idx];
                        (ann.font_desc.clone(), ann.color, ann.rotation)
                    };
                    state.borrow_mut().selected_ann = Some(idx);
                    font_btn.set_font_desc(&ann_font);
                    color_btn.set_rgba(&gdk::RGBA::new(
                        ann_color.0 as f32, ann_color.1 as f32, ann_color.2 as f32, ann_color.3 as f32,
                    ));
                    rotation_spin.set_value(ann_rotation.to_degrees());
                }
                canvas.queue_draw();
                return;
            }
            commit_draft();
            let img_x = (x - ox) / scale;
            let img_y = (y - oy) / scale;
            {
                let mut s = state.borrow_mut();
                s.draft_pos = Some((img_x, img_y));
                s.draft_center = Some((img_x, img_y));
                s.draft_text.clear(); s.selected_ann = None; s.text_rotation = 0.0;
            }
            rotation_spin.set_value(0.0);
            draft_entry.set_text("");
            let ent = draft_entry.clone();
            let blink = start_blink.clone();
            let cpos = draft_cursor_pos.clone();
            glib::idle_add_local_once(move || {
                ent.set_margin_start(x as i32); ent.set_margin_top(y as i32);
                ent.set_visible(true); ent.grab_focus();
                cpos.set(ent.property::<i32>("cursor-position")); blink();
            });
            canvas.queue_draw();
        }
    });
    w.canvas_overlay.add_controller(text_click);
}

pub fn setup_text_drag(w: &Widgets, state: Rc<RefCell<State>>) {
    let canvas = w.canvas.clone();
    let rotation_spin = w.rotation_spin.clone();

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
                let vw = canvas.width() as f64; let vh = canvas.height() as f64;
                let (ox, oy, scale) = if s.fit_mode { coords::fit_transform(s.img_width, s.img_height, vw, vh) } else { (0.0, 0.0, s.zoom) };
                let (mode, hit) = if let Some(idx) = s.selected_ann {
                    let ann = &s.annotations[idx];
                    if hit_test::hit_test_rotation_handle(ann, x, y, ox, oy, scale, &pango_ctx) {
                        let layout = gtk4::pango::Layout::new(&pango_ctx);
                        layout.set_font_description(Some(&ann.font_desc));
                        layout.set_text(&ann.text);
                        let (tw, th) = layout.pixel_size();
                        let anchor_wx = ox + ann.x * scale + tw as f64 / 2.0;
                        let anchor_wy = oy + ann.y * scale + th as f64 / 2.0;
                        (DragMode::Rotate, Some((idx, ann.x, ann.y, anchor_wx, anchor_wy, ann.rotation)))
                    } else {
                        let h = hit_test::hit_test_annotation(&s.annotations, x, y, ox, oy, scale, &pango_ctx)
                            .map(|i| (i, s.annotations[i].x, s.annotations[i].y, 0.0, 0.0, 0.0));
                        (DragMode::Move, h)
                    }
                } else {
                    let h = hit_test::hit_test_annotation(&s.annotations, x, y, ox, oy, scale, &pango_ctx)
                        .map(|i| (i, s.annotations[i].x, s.annotations[i].y, 0.0, 0.0, 0.0));
                    (DragMode::Move, h)
                };
                (s.text_tool_active, mode, hit)
            };
            if !active { g.set_state(gtk4::EventSequenceState::Denied); return; }
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
                    DragMode::Move => { s.move_origin = Some((orig_x, orig_y)); }
                }
                drop(s); canvas.queue_draw();
            } else { g.set_state(gtk4::EventSequenceState::Denied); }
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
                coords::fit_transform(s.img_width, s.img_height, canvas.width() as f64, canvas.height() as f64)
            } else { (0.0, 0.0, s.zoom) };
            if s.rotation_drag {
                if let Some(idx) = s.selected_ann {
                    let (ax, ay) = s.rotation_drag_anchor;
                    let (bx, by) = s.rotation_drag_begin;
                    let init_rot = s.rotation_drag_initial_rotation;
                    let angle_begin = (by - ay).atan2(bx - ax);
                    let cur_x = bx + dx; let cur_y = by + dy;
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
                let nx = ox + dx / scale; let ny = oy + dy / scale;
                drop(s);
                let mut s = state.borrow_mut();
                s.annotations[idx].x = nx; s.annotations[idx].y = ny;
                drop(s); canvas.queue_draw();
            }
        }
    });
    text_drag.connect_drag_end({
        let state = state.clone();
        move |_, _, _| {
            let mut s = state.borrow_mut();
            s.move_origin = None; s.rotation_drag = false;
        }
    });
    w.canvas_overlay.add_controller(text_drag);
}
