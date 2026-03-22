use gtk4::prelude::*;
use gtk4::glib;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::annotation;
use crate::state::State;
use crate::widgets::Widgets;

pub fn make_set_crop_mode(
    w: &Widgets,
    state: Rc<RefCell<State>>,
    apply_zoom: Rc<dyn Fn()>,
) -> Rc<dyn Fn(bool)> {
    let canvas = w.canvas.clone();
    let crop_bar = w.crop_bar.clone();
    let edit_group = w.edit_group.clone();
    let apply_crop_btn = w.apply_crop_btn.clone();
    Rc::new(move |active: bool| {
        {
            let mut s = state.borrow_mut();
            s.in_crop = active; s.drag_start = None; s.drag_end = None;
            if active { s.fit_mode = true; }
        }
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
    Rc::new(move || {
        stop_blink();
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
    let edit_group = w.edit_group.clone();
    Rc::new(move |active: bool| {
        if !active {
            commit_draft();
            state.borrow_mut().selected_ann = None;
        }
        state.borrow_mut().text_tool_active = active;
        text_tool_bar.set_visible(active);
        edit_group.set_sensitive(!active);
        canvas.queue_draw();
    })
}
