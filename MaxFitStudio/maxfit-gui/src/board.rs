//! The keyboard widget: all 61 keys drawn to scale from the vendor's own
//! keycap geometry, painted with whatever the LEDs are currently showing.

use std::collections::HashSet;

use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    Vec2,
};
use maxfit_core::{MATRIX_CELLS, Rgb, keys, layout};

/// Breathing room around the keys, in the vendor's pixel units.
const PAD: f32 = 12.0;
/// Unlit keycap colour, so the board stays readable when the LEDs are off.
const KEYCAP_DARK: Color32 = Color32::from_rgb(0x2A, 0x2D, 0x34);
const BEZEL: Color32 = Color32::from_rgb(0x15, 0x16, 0x1A);

#[derive(Default)]
pub struct BoardAction {
    pub hovered: Option<usize>,
    /// LED indices the pointer painted this frame (click or drag).
    pub painted: Vec<usize>,
    /// LED indices the pointer cleared this frame (right-click or drag).
    pub cleared: Vec<usize>,
}

fn to_color32(c: Rgb) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

/// Perceived brightness, for deciding whether a legend should be dark or light.
fn luma(c: Rgb) -> f32 {
    (0.299 * f32::from(c.r) + 0.587 * f32::from(c.g) + 0.114 * f32::from(c.b)) / 255.0
}

/// Draws the board and returns what the pointer did.
///
/// `colors` is the live LED state, so this view is an exact mirror of the
/// hardware rather than an approximation of it.
pub fn show(
    ui: &mut Ui,
    colors: &[Rgb; MATRIX_CELLS],
    selected: &HashSet<usize>,
    interactive: bool,
) -> (Response, BoardAction) {
    let lay = layout();
    let board_w = lay.bounds.w + PAD * 2.0;
    let board_h = lay.bounds.h + PAD * 2.0;

    // Fill the available width, keeping the true aspect ratio of the board.
    let avail = ui.available_width().max(200.0);
    let scale = (avail / board_w).min(2.2);
    let size = Vec2::new(board_w * scale, board_h * scale);

    let sense = if interactive {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, CornerRadius::same(8), BEZEL);

    let origin = rect.min + Vec2::splat(PAD * scale);
    let key_rect = |k: &maxfit_core::Key| {
        Rect::from_min_size(
            origin + Vec2::new(k.x - lay.bounds.x, k.y - lay.bounds.y) * scale,
            Vec2::new(k.w, k.h) * scale,
        )
    };

    let pointer = resp.hover_pos();
    let mut action = BoardAction::default();

    // Which key is under the cursor. 61 rects is far too few to need an index.
    if let Some(p) = pointer {
        action.hovered = keys()
            .iter()
            .find(|k| key_rect(k).contains(p))
            .map(|k| k.led);
    }

    if interactive
        && let Some(p) = resp.interact_pointer_pos()
        && let Some(k) = keys().iter().find(|k| key_rect(k).contains(p))
    {
        let primary = resp.is_pointer_button_down_on() || resp.clicked();
        if resp.secondary_clicked() {
            action.cleared.push(k.led);
        } else if primary {
            action.painted.push(k.led);
        }
    }

    let radius = CornerRadius::same((3.0 * scale).clamp(2.0, 6.0) as u8);
    let font_main = FontId::proportional((11.0 * scale).clamp(6.0, 16.0));
    let font_sub = FontId::proportional((8.5 * scale).clamp(5.0, 12.0));

    for k in keys() {
        let r = key_rect(k);
        let led = colors[k.led];

        // An unlit key still has to read as a key, so fall back to the dark
        // keycap colour rather than drawing a black hole.
        let fill = if led.is_black() {
            KEYCAP_DARK
        } else {
            to_color32(led)
        };
        painter.rect_filled(r, radius, fill);

        // A lit key gets a soft top highlight so it reads as backlit plastic
        // rather than a flat swatch.
        if !led.is_black() {
            let top = Rect::from_min_max(r.min, Pos2::new(r.max.x, r.min.y + r.height() * 0.42));
            painter.rect_filled(top, radius, Color32::from_white_alpha(18));
        }

        let is_sel = selected.contains(&k.led);
        let is_hot = action.hovered == Some(k.led);
        if is_sel || is_hot {
            let stroke = if is_sel {
                Stroke::new(2.0_f32.max(scale * 1.6), Color32::from_rgb(0x6E, 0xC6, 0xFF))
            } else {
                Stroke::new(1.0_f32.max(scale), Color32::from_white_alpha(140))
            };
            painter.rect_stroke(r, radius, stroke, StrokeKind::Inside);
        } else {
            painter.rect_stroke(
                r,
                radius,
                Stroke::new(1.0_f32, Color32::from_black_alpha(90)),
                StrokeKind::Inside,
            );
        }

        // Legends only once the keys are big enough to read.
        if scale < 0.45 {
            continue;
        }
        let ink = if led.is_black() {
            Color32::from_rgb(0xB8, 0xBE, 0xC8)
        } else if luma(led) > 0.55 {
            Color32::from_rgb(0x10, 0x10, 0x14)
        } else {
            Color32::from_rgb(0xF2, 0xF4, 0xF8)
        };

        if k.shift_legend.is_empty() {
            painter.text(r.center(), Align2::CENTER_CENTER, &k.legend, font_main.clone(), ink);
        } else {
            // Shifted character above the base one, the way it is on the cap.
            let q = r.height() * 0.22;
            painter.text(
                r.center() - Vec2::new(0.0, q),
                Align2::CENTER_CENTER,
                &k.shift_legend,
                font_sub.clone(),
                ink,
            );
            painter.text(
                r.center() + Vec2::new(0.0, q),
                Align2::CENTER_CENTER,
                &k.legend,
                font_main.clone(),
                ink,
            );
        }
    }

    (resp, action)
}
