//! Dark theme tuned so the keyboard is the brightest thing on screen.

use eframe::egui::{Color32, Context, CornerRadius, Stroke, Visuals};

pub fn apply(ctx: &Context) {
    let mut v = Visuals::dark();
    v.panel_fill = Color32::from_rgb(0x12, 0x13, 0x17);
    v.window_fill = Color32::from_rgb(0x15, 0x16, 0x1B);
    v.extreme_bg_color = Color32::from_rgb(0x0C, 0x0D, 0x10);
    v.widgets.noninteractive.bg_fill = Color32::from_rgb(0x1A, 0x1C, 0x22);
    v.widgets.inactive.bg_fill = Color32::from_rgb(0x22, 0x25, 0x2C);
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x2C, 0x30, 0x39);
    v.widgets.active.bg_fill = Color32::from_rgb(0x35, 0x3A, 0x45);
    v.selection.bg_fill = Color32::from_rgb(0x1E, 0x4F, 0x6E);
    v.selection.stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x6E, 0xC6, 0xFF));
    v.widgets.noninteractive.corner_radius = CornerRadius::same(5);
    v.widgets.inactive.corner_radius = CornerRadius::same(5);
    v.widgets.hovered.corner_radius = CornerRadius::same(5);
    v.widgets.active.corner_radius = CornerRadius::same(5);
    ctx.set_visuals(v);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = (8.0, 7.0).into();
    style.spacing.slider_width = 170.0;
    style.spacing.button_padding = (10.0, 5.0).into();
    ctx.set_style(style);
}
