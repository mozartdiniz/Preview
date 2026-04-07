use gtk4::cairo;
use gtk4::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use std::cell::RefCell;

use crate::coords;
use crate::annotation;

pub fn setup(
    canvas: &gtk4::DrawingArea,
    state: Rc<RefCell<crate::state::State>>,
    cursor_blink_on: Rc<Cell<bool>>,
    draft_cursor_pos: Rc<Cell<i32>>,
) {
    canvas.set_draw_func({
        let state = state.clone();
        let cursor_blink_on = cursor_blink_on.clone();
        let draft_cursor_pos = draft_cursor_pos.clone();
        move |_da, cr, width, height| {
            let s = state.borrow();
            let Some(ref surface) = s.surface else { return };

            let vw = width as f64;
            let vh = height as f64;
            let img_w = s.img_width as f64;
            let img_h = s.img_height as f64;

            let (ox, oy, scale) = if s.fit_mode {
                coords::fit_transform(s.img_width, s.img_height, vw, vh)
            } else {
                (0.0, 0.0, s.zoom)
            };

            cr.save().unwrap();
            cr.translate(ox, oy);
            cr.scale(scale, scale);
            cr.set_source_surface(surface, 0.0, 0.0).unwrap();
            cr.source().set_filter(cairo::Filter::Bilinear);
            cr.paint().unwrap();

            for (i, shape) in s.shape_annotations.iter().enumerate() {
                annotation::draw_shape_annotation(cr, shape);
                if s.shape_tool_active && s.selected_shape == Some(i) {
                    let handle_r = 5.0 / scale;
                    let pad = 1.5 / scale;
                    cr.set_source_rgba(0.2, 0.6, 1.0, 0.9);
                    cr.set_line_width(1.5 / scale);
                    cr.set_dash(&[5.0 / scale, 4.0 / scale], 0.0);
                    annotation::draw_shape_annotation(cr, shape);  // dashed blue overlay
                    cr.set_dash(&[], 0.0);
                    for (hx, hy) in [(shape.x1, shape.y1), (shape.x2, shape.y2)] {
                        cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                        cr.arc(hx, hy, handle_r + pad, 0.0, std::f64::consts::TAU);
                        cr.fill().unwrap();
                        cr.set_source_rgba(0.2, 0.6, 1.0, 0.9);
                        cr.set_line_width(1.5 / scale);
                        cr.arc(hx, hy, handle_r + pad, 0.0, std::f64::consts::TAU);
                        cr.stroke().unwrap();
                    }
                }
            }

            if let Some((x1, y1, x2, y2)) = s.shape_draft {
                let draft_shape = annotation::ShapeAnnotation {
                    kind: s.shape_kind,
                    x1, y1, x2, y2,
                    color: s.shape_color,
                    stroke_width: s.shape_stroke_width,
                };
                annotation::draw_shape_annotation(cr, &draft_shape);
            }

            for (i, ann) in s.annotations.iter().enumerate() {
                annotation::draw_text_annotation(cr, ann);
                if s.text_tool_active && s.selected_ann == Some(i) {
                    let layout = pangocairo::functions::create_layout(cr);
                    layout.set_font_description(Some(&ann.font_desc));
                    layout.set_text(&ann.text);
                    let (tw, th) = layout.pixel_size();
                    let half_w = tw as f64 / 2.0;
                    let half_h = th as f64 / 2.0;
                    let pad = 4.0 / scale;
                    let handle_r = 5.0 / scale;
                    cr.save().unwrap();
                    cr.translate(ann.x + half_w, ann.y + half_h);
                    cr.rotate(ann.rotation);
                    let bx = -half_w - pad;
                    let by = -half_h - pad;
                    let bw = tw as f64 + pad * 2.0;
                    let bh = th as f64 + pad * 2.0;
                    cr.set_source_rgba(0.2, 0.6, 1.0, 0.9);
                    cr.set_line_width(1.5 / scale);
                    cr.set_dash(&[5.0 / scale, 4.0 / scale], 0.0);
                    cr.rectangle(bx, by, bw, bh);
                    cr.stroke().unwrap();
                    cr.set_dash(&[], 0.0);
                    for (cx, cy) in [(bx, by), (bx + bw, by), (bx, by + bh), (bx + bw, by + bh)] {
                        cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                        cr.arc(cx, cy, handle_r, 0.0, std::f64::consts::TAU);
                        cr.fill().unwrap();
                        cr.set_source_rgba(0.2, 0.6, 1.0, 0.9);
                        cr.set_line_width(1.5 / scale);
                        cr.arc(cx, cy, handle_r, 0.0, std::f64::consts::TAU);
                        cr.stroke().unwrap();
                    }
                    cr.restore().unwrap();
                }
            }

            if let Some((dx, dy)) = s.draft_pos {
                let font_desc = s.text_font_desc.clone()
                    .unwrap_or_else(|| gtk4::pango::FontDescription::from_string("Sans 24"));
                if !s.draft_text.is_empty() {
                    let preview = annotation::TextAnnotation {
                        x: dx, y: dy,
                        text: s.draft_text.clone(),
                        font_desc: font_desc.clone(),
                        color: s.text_color,
                        rotation: s.text_rotation,
                    };
                    annotation::draw_text_annotation(cr, &preview);
                }
                if cursor_blink_on.get() {
                    let layout = pangocairo::functions::create_layout(cr);
                    layout.set_font_description(Some(&font_desc));
                    layout.set_text(&s.draft_text);
                    let char_pos = draft_cursor_pos.get() as usize;
                    let byte_idx = s.draft_text
                        .char_indices().nth(char_pos)
                        .map(|(i, _)| i)
                        .unwrap_or(s.draft_text.len()) as i32;
                    let rect = layout.index_to_pos(byte_idx);
                    let cursor_x = rect.x() as f64 / gtk4::pango::SCALE as f64;
                    let (tw, ph) = layout.pixel_size();
                    let half_w = tw as f64 / 2.0;
                    let half_h = ph as f64 / 2.0;
                    let (r, g, b, a) = s.text_color;
                    cr.set_source_rgba(r, g, b, a);
                    cr.set_line_width(2.0 / scale);
                    cr.save().unwrap();
                    cr.translate(dx + half_w, dy + half_h);
                    cr.rotate(s.text_rotation);
                    cr.move_to(-half_w + cursor_x, -half_h);
                    cr.line_to(-half_w + cursor_x, half_h);
                    cr.stroke().unwrap();
                    cr.restore().unwrap();
                }
                if s.draft_text.is_empty() {
                    let dot = 4.0 / scale;
                    cr.set_source_rgba(1.0, 0.9, 0.0, 0.9);
                    cr.arc(dx, dy, dot, 0.0, std::f64::consts::TAU);
                    cr.fill().unwrap();
                }
            }
            cr.restore().unwrap();

            if s.in_crop {
                let rendered_w = img_w * scale;
                let rendered_h = img_h * scale;
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.5);
                if let (Some((ax, ay)), Some((bx, by))) = (s.drag_start, s.drag_end) {
                    let sx = ax.min(bx); let sy = ay.min(by);
                    let ex = ax.max(bx); let ey = ay.max(by);
                    cr.rectangle(ox, oy, rendered_w, sy - oy); cr.fill().unwrap();
                    cr.rectangle(ox, ey, rendered_w, oy + rendered_h - ey); cr.fill().unwrap();
                    cr.rectangle(ox, sy, sx - ox, ey - sy); cr.fill().unwrap();
                    cr.rectangle(ex, sy, ox + rendered_w - ex, ey - sy); cr.fill().unwrap();
                    cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
                    cr.set_line_width(1.5);
                    cr.rectangle(sx, sy, ex - sx, ey - sy); cr.stroke().unwrap();
                    cr.set_source_rgba(1.0, 1.0, 1.0, 0.35);
                    cr.set_line_width(0.5);
                    let sw = ex - sx; let sh = ey - sy;
                    for i in 1..3 {
                        let f = i as f64 / 3.0;
                        cr.move_to(sx + sw * f, sy); cr.line_to(sx + sw * f, ey); cr.stroke().unwrap();
                        cr.move_to(sx, sy + sh * f); cr.line_to(ex, sy + sh * f); cr.stroke().unwrap();
                    }
                } else {
                    cr.rectangle(ox, oy, rendered_w, rendered_h); cr.fill().unwrap();
                }
            }
        }
    });
}
