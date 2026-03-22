use gtk4::cairo;
use gtk4::pango;
use image::DynamicImage;
use std::path::PathBuf;

use crate::annotation::TextAnnotation;

// ── History ───────────────────────────────────────────────────────────────────

const MAX_UNDO: usize = 20;

pub struct HistoryEntry {
    pub image: Option<DynamicImage>,
    pub img_width: i32,
    pub img_height: i32,
    pub annotations: Vec<TextAnnotation>,
}

// ── State ─────────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct State {
    // Zoom / display
    pub zoom: f64,
    pub fit_mode: bool,
    pub img_width: i32,
    pub img_height: i32,
    // Cairo surface used by the draw func (derived from `image`)
    pub surface: Option<cairo::ImageSurface>,
    // Editable pixel data (None for display-only formats like SVG)
    pub image: Option<DynamicImage>,
    // Original file path (for Save overwrite)
    pub file_path: Option<PathBuf>,
    // Crop rubber-band in widget (viewport) coordinates — fit-mode only
    pub in_crop: bool,
    pub drag_start: Option<(f64, f64)>,
    pub drag_end: Option<(f64, f64)>,
    // Text annotation tool
    pub annotations: Vec<TextAnnotation>,
    pub text_tool_active: bool,
    pub text_font_desc: Option<pango::FontDescription>,
    pub text_color: (f64, f64, f64, f64),
    pub text_rotation: f64,  // radians
    // Draft annotation being typed (image-space coords)
    pub draft_pos: Option<(f64, f64)>,    // top-left of text (updated each keystroke)
    pub draft_center: Option<(f64, f64)>, // fixed visual centre (set at click/re-edit)
    pub draft_text: String,
    // Selected annotation index (for drag-to-move)
    pub selected_ann: Option<usize>,
    pub move_origin: Option<(f64, f64)>,  // annotation's image coords at drag start
    // Rotation drag
    pub rotation_drag: bool,
    pub rotation_drag_anchor: (f64, f64),    // text centre in widget space
    pub rotation_drag_begin: (f64, f64),     // widget pos where drag started
    pub rotation_drag_initial_rotation: f64,
    // Copy/paste clipboard
    pub clipboard: Option<TextAnnotation>,
    // Undo/redo history
    pub undo_stack: Vec<HistoryEntry>,
    pub redo_stack: Vec<HistoryEntry>,
    // Suppress undo/annotation updates during programmatic UI sync and rotation drags
    pub syncing_ui: bool,
    // True once undo has been pushed for the current annotation property-edit session;
    // reset whenever a different annotation is selected or text mode is deactivated.
    pub property_undo_pushed: bool,
}

impl State {
    pub fn new() -> Self {
        Self {
            zoom: 1.0,
            fit_mode: true,
            text_color: (1.0, 0.0, 0.0, 1.0),
            ..Default::default()
        }
    }

    pub fn has_image(&self) -> bool {
        self.img_width > 0
    }

    pub fn push_undo(&mut self) {
        self.undo_stack.push(HistoryEntry {
            image: self.image.clone(),
            img_width: self.img_width,
            img_height: self.img_height,
            annotations: self.annotations.clone(),
        });
        if self.undo_stack.len() > MAX_UNDO {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }
}
