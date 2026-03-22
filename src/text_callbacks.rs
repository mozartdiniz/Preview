use gtk4::prelude::*;
use gtk4::{gdk, glib};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::coords;
use crate::state::State;
use crate::transforms;
use crate::widgets::Widgets;
use crate::Closures;

pub fn connect(
    w: &Widgets,
    state: Rc<RefCell<State>>,
    c: &Closures,
    draft_cursor_pos: Rc<Cell<i32>>,
) {
    let canvas = w.canvas.clone();
    let draft_entry = w.draft_entry.clone();

    draft_entry.connect_changed({
        let state = state.clone(); let canvas = canvas.clone();
        move |entry| {
            let text = entry.text().to_string();
            let (font_desc, scale) = {
                let s = state.borrow_mut();
                let fd = s.text_font_desc.clone()
                    .unwrap_or_else(|| gtk4::pango::FontDescription::from_string("Sans 24"));
                let sc = if s.fit_mode {
                    let (_, _, sc) = coords::fit_transform(s.img_width, s.img_height, canvas.width() as f64, canvas.height() as f64);
                    sc
                } else { s.zoom };
                drop(s); (fd, sc)
            };
            state.borrow_mut().draft_text = text.clone();
            let pc = canvas.pango_context();
            let layout = gtk4::pango::Layout::new(&pc);
            layout.set_font_description(Some(&font_desc));
            layout.set_text(&text);
            let (pw, ph) = layout.pixel_size();
            if !text.is_empty() {
                entry.set_width_request((pw as f64 * scale).ceil() as i32 + 4);
                entry.set_height_request((ph as f64 * scale).ceil() as i32 + 4);
            }
            let draft_center = state.borrow().draft_center;
            if let Some((cx, cy)) = draft_center {
                let half_w = pw as f64 / scale / 2.0;
                let half_h = ph as f64 / scale / 2.0;
                state.borrow_mut().draft_pos = Some((cx - half_w, cy - half_h));
            }
            canvas.queue_draw();
        }
    });

    draft_entry.connect_activate({
        let commit_draft = c.commit_draft.clone();
        move |_| commit_draft()
    });

    draft_entry.connect_notify_local(Some("cursor-position"), {
        let draft_cursor_pos = draft_cursor_pos.clone();
        let start_blink = c.start_blink.clone();
        move |entry, _| {
            draft_cursor_pos.set(entry.property::<i32>("cursor-position"));
            start_blink();
        }
    });

    let esc_ctrl = gtk4::EventControllerKey::new();
    esc_ctrl.connect_key_pressed({
        let state = state.clone(); let canvas = canvas.clone();
        let draft_entry = draft_entry.clone(); let stop_blink = c.stop_blink.clone();
        move |_, keyval, _, _| {
            if keyval == gdk::Key::Escape {
                stop_blink();
                let mut s = state.borrow_mut();
                s.draft_pos = None; s.draft_center = None; s.draft_text.clear(); drop(s);
                draft_entry.set_text(""); draft_entry.set_visible(false);
                canvas.queue_draw();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        }
    });
    draft_entry.add_controller(esc_ctrl);

    let ann_key_ctrl = gtk4::EventControllerKey::new();
    ann_key_ctrl.connect_key_pressed({
        let state = state.clone(); let canvas = canvas.clone();
        let draft_entry = draft_entry.clone();
        move |_, keyval, _, modifiers| {
            let s = state.borrow();
            if !s.text_tool_active || gtk4::prelude::WidgetExt::is_visible(&draft_entry) {
                return glib::Propagation::Proceed;
            }
            let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
            let selected = s.selected_ann; drop(s);
            match (keyval, ctrl) {
                (gdk::Key::Delete, false) | (gdk::Key::KP_Delete, false) => {
                    let mut s = state.borrow_mut();
                    if let Some(idx) = selected { s.annotations.remove(idx); s.selected_ann = None; }
                    drop(s); canvas.queue_draw(); glib::Propagation::Stop
                }
                (gdk::Key::c, true) | (gdk::Key::C, true) => {
                    let mut s = state.borrow_mut();
                    if let Some(idx) = selected { s.clipboard = s.annotations.get(idx).cloned(); }
                    glib::Propagation::Stop
                }
                (gdk::Key::v, true) | (gdk::Key::V, true) => {
                    let mut s = state.borrow_mut();
                    if let Some(ref mut cb) = s.clipboard {
                        cb.x += 10.0; cb.y += 10.0;
                        let ann = cb.clone();
                        s.annotations.push(ann);
                        s.selected_ann = Some(s.annotations.len() - 1);
                        drop(s); canvas.queue_draw();
                    }
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        }
    });
    w.window.add_controller(ann_key_ctrl);

    w.font_btn.connect_font_desc_notify({
        let state = state.clone(); let canvas = canvas.clone();
        move |btn| {
            let mut s = state.borrow_mut();
            s.text_font_desc = btn.font_desc();
            if let Some(idx) = s.selected_ann {
                if let Some(ann) = s.annotations.get_mut(idx) {
                    if let Some(fd) = btn.font_desc() { ann.font_desc = fd; }
                }
                drop(s); canvas.queue_draw();
            }
        }
    });

    w.color_btn.connect_rgba_notify({
        let state = state.clone(); let canvas = canvas.clone();
        move |btn| {
            let c = btn.rgba();
            let color = (c.red() as f64, c.green() as f64, c.blue() as f64, c.alpha() as f64);
            let mut s = state.borrow_mut();
            s.text_color = color;
            if let Some(idx) = s.selected_ann {
                if let Some(ann) = s.annotations.get_mut(idx) { ann.color = color; }
                drop(s); canvas.queue_draw();
            }
        }
    });

    w.rotation_spin.connect_value_changed({
        let state = state.clone(); let canvas = canvas.clone();
        move |spin| {
            let rad = spin.value().to_radians();
            let mut s = state.borrow_mut();
            s.text_rotation = rad;
            if let Some(idx) = s.selected_ann {
                if let Some(ann) = s.annotations.get_mut(idx) { ann.rotation = rad; }
            }
            drop(s); canvas.queue_draw();
        }
    });

    w.apply_crop_btn.connect_clicked({
        let state = state.clone(); let canvas = canvas.clone();
        let update_image = c.update_image.clone();
        let set_crop_mode = c.set_crop_mode.clone();
        let set_text_mode = c.set_text_mode.clone();
        move |_| {
            set_text_mode(false);
            let (crop_rect, img) = {
                let s = state.borrow();
                let vw = canvas.width() as f64; let vh = canvas.height() as f64;
                let (ox, oy, scale) = coords::fit_transform(s.img_width, s.img_height, vw, vh);
                let rect = s.drag_start.zip(s.drag_end).map(|((ax, ay), (bx, by))| {
                    let (x1, y1) = coords::widget_to_img(ax.min(bx), ay.min(by), s.img_width, s.img_height, ox, oy, scale);
                    let (x2, y2) = coords::widget_to_img(ax.max(bx), ay.max(by), s.img_width, s.img_height, ox, oy, scale);
                    (x1, y1, x2.saturating_sub(x1), y2.saturating_sub(y1))
                });
                (rect, s.image.as_ref().map(|i| i.clone()))
            };
            if let (Some((x, y, iw, ih)), Some(img)) = (crop_rect, img) {
                if let Some(cropped) = transforms::crop(&img, x, y, iw, ih) { update_image(cropped); }
            }
            set_crop_mode(false);
        }
    });
}
