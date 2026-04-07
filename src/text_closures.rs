use gtk4::prelude::*;
use gtk4::{gdk, glib};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::annotation::{self, ShapeKind};
use crate::state::State;
use crate::widgets::Widgets;

pub fn make_set_crop_mode(
    w: &Widgets,
    state: Rc<RefCell<State>>,
    apply_zoom: Rc<dyn Fn()>,
) -> Rc<dyn Fn(bool)> {
    let canvas = w.canvas.clone();
    let crop_bar = w.crop_bar.clone();
    let shape_tool_bar = w.shape_tool_bar.clone();
    let edit_group = w.edit_group.clone();
    let apply_crop_btn = w.apply_crop_btn.clone();
    Rc::new(move |active: bool| {
        {
            let mut s = state.borrow_mut();
            s.in_crop = active; s.drag_start = None; s.drag_end = None;
            if active {
                s.fit_mode = true;
                s.shape_tool_active = false; s.shape_draft = None;
            }
        }
        if active { shape_tool_bar.set_visible(false); }
        crop_bar.set_visible(active);
        edit_group.set_sensitive(!active);
        apply_crop_btn.set_sensitive(false);
        if active { apply_zoom(); }
        canvas.queue_draw();
    })
}

pub fn make_blink_handlers(
    canvas: gtk4::DrawingArea,
    cursor_blink_on: Rc<Cell<bool>>,
    blink_source: Rc<Cell<Option<glib::SourceId>>>,
) -> (Rc<dyn Fn()>, Rc<dyn Fn()>) {
    let start_blink: Rc<dyn Fn()> = Rc::new({
        let cursor_blink_on = cursor_blink_on.clone();
        let blink_source = blink_source.clone();
        let canvas = canvas.clone();
        move || {
            if let Some(id) = blink_source.take() { id.remove(); }
            cursor_blink_on.set(true);
            canvas.queue_draw();
            let id = glib::timeout_add_local(std::time::Duration::from_millis(530), {
                let cursor_blink_on = cursor_blink_on.clone();
                let canvas = canvas.clone();
                move || {
                    cursor_blink_on.set(!cursor_blink_on.get());
                    canvas.queue_draw();
                    glib::ControlFlow::Continue
                }
            });
            blink_source.set(Some(id));
        }
    });

    let stop_blink: Rc<dyn Fn()> = Rc::new({
        let cursor_blink_on = cursor_blink_on.clone();
        let blink_source = blink_source.clone();
        let canvas = canvas.clone();
        move || {
            if let Some(id) = blink_source.take() { id.remove(); }
            cursor_blink_on.set(false);
            canvas.queue_draw();
        }
    });

    (start_blink, stop_blink)
}

pub fn make_commit_draft(
    w: &Widgets,
    state: Rc<RefCell<State>>,
    stop_blink: Rc<dyn Fn()>,
) -> Rc<dyn Fn()> {
    let canvas = w.canvas.clone();
    let draft_entry = w.draft_entry.clone();
    let undo_btn = w.undo_btn.clone();
    Rc::new(move || {
        stop_blink();
        let will_commit = { let s = state.borrow(); s.draft_pos.is_some() && !s.draft_text.is_empty() };
        if will_commit { state.borrow_mut().push_undo(); undo_btn.set_sensitive(true); }
        let mut s = state.borrow_mut();
        s.draft_center = None;
        if let Some((dx, dy)) = s.draft_pos.take() {
            let text = std::mem::take(&mut s.draft_text);
            if !text.is_empty() {
                let font_desc = s.text_font_desc.clone()
                    .unwrap_or_else(|| gtk4::pango::FontDescription::from_string("Sans 24"));
                let color = s.text_color;
                let rotation = s.text_rotation;
                s.annotations.push(annotation::TextAnnotation { x: dx, y: dy, text, font_desc, color, rotation });
            }
        } else {
            s.draft_text.clear();
        }
        drop(s);
        draft_entry.set_text("");
        draft_entry.set_visible(false);
        canvas.queue_draw();
    })
}

pub fn make_set_text_mode(
    w: &Widgets,
    state: Rc<RefCell<State>>,
    commit_draft: Rc<dyn Fn()>,
) -> Rc<dyn Fn(bool)> {
    let canvas = w.canvas.clone();
    let text_tool_bar = w.text_tool_bar.clone();
    let shape_tool_bar = w.shape_tool_bar.clone();
    let edit_group = w.edit_group.clone();
    Rc::new(move |active: bool| {
        if !active {
            commit_draft();
            let mut s = state.borrow_mut();
            s.selected_ann = None;
            s.property_undo_pushed = false;
            drop(s);
        } else {
            // Deactivate shape mode when entering text mode
            let mut s = state.borrow_mut();
            s.shape_tool_active = false; s.shape_draft = None;
            s.selected_shape = None; s.shape_property_undo_pushed = false;
            drop(s);
            shape_tool_bar.set_visible(false);
        }
        state.borrow_mut().text_tool_active = active;
        text_tool_bar.set_visible(active);
        edit_group.set_sensitive(!active);
        canvas.queue_draw();
    })
}

pub fn make_set_shape_tool(
    w: &Widgets,
    state: Rc<RefCell<State>>,
    commit_draft: Rc<dyn Fn()>,
) -> Rc<dyn Fn(Option<ShapeKind>)> {
    let canvas = w.canvas.clone();
    let text_tool_bar = w.text_tool_bar.clone();
    let shape_tool_bar = w.shape_tool_bar.clone();
    let edit_group = w.edit_group.clone();
    let shape_color_btn = w.shape_color_btn.clone();
    let shape_stroke_spin = w.shape_stroke_spin.clone();
    Rc::new(move |kind: Option<ShapeKind>| {
        match kind {
            Some(k) => {
                // Deactivate text mode
                commit_draft();
                {
                    let mut s = state.borrow_mut();
                    s.text_tool_active = false;
                    s.selected_ann = None;
                    s.property_undo_pushed = false;
                    s.shape_tool_active = true;
                    s.shape_kind = k;
                    s.shape_draft = None;
                }
                let (color, stroke) = {
                    let s = state.borrow();
                    (s.shape_color, s.shape_stroke_width)
                };
                text_tool_bar.set_visible(false);
                shape_tool_bar.set_visible(true);
                edit_group.set_sensitive(false);
                shape_color_btn.set_rgba(&gdk::RGBA::new(
                    color.0 as f32, color.1 as f32, color.2 as f32, color.3 as f32,
                ));
                shape_stroke_spin.set_value(stroke);
            }
            None => {
                {
                    let mut s = state.borrow_mut();
                    s.shape_tool_active = false;
                    s.shape_draft = None;
                    s.selected_shape = None;
                    s.shape_property_undo_pushed = false;
                }
                shape_tool_bar.set_visible(false);
                edit_group.set_sensitive(true);
            }
        }
        canvas.queue_draw();
    })
}
