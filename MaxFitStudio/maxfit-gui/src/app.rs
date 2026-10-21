//! MaxFit Studio: the application shell.

use std::collections::HashSet;

use eframe::egui::{self, Color32, RichText, Ui};
use maxfit_core::effects::{AnimParams, AnimationKind};
use maxfit_core::{
    DeviceInfo, Effect, LedFrame, LightCapabilities, LightingState, MATRIX_CELLS, Rgb, keys,
};
use maxfit_device::{DeviceCmd, DeviceEvt, DeviceLink, EngineStats};

use crate::board;
use crate::presets::{self, Preset};

const ACCENT: Color32 = Color32::from_rgb(0x6E, 0xC6, 0xFF);
const GOOD: Color32 = Color32::from_rgb(0x66, 0xD9, 0x9B);
const BAD: Color32 = Color32::from_rgb(0xFF, 0x8A, 0x7A);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The keyboard's own 17 effects, run by its firmware.
    Firmware,
    /// PC-rendered animations streamed frame by frame.
    Animation,
    /// A hand-painted static per-key layout.
    Paint,
}

pub struct MaxFitApp {
    link: DeviceLink,

    connected: bool,
    info: Option<DeviceInfo>,
    caps: Option<LightCapabilities>,
    status: String,
    status_bad: bool,

    mode: Mode,

    lighting: LightingState,
    anim_kind: AnimationKind,
    anim_params: AnimParams,
    anim_running: bool,
    target_fps: u16,

    paint: LedFrame,
    paint_color: Rgb,
    selection: HashSet<usize>,

    stats: EngineStats,
    preview: [Rgb; MATRIX_CELLS],
    user_presets: Vec<Preset>,
    new_preset_name: String,
}

impl MaxFitApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::theme::apply(&cc.egui_ctx);

        // The worker wakes the UI when it publishes a frame.
        let ctx = cc.egui_ctx.clone();
        let link = DeviceLink::spawn(move || ctx.request_repaint());
        link.send(DeviceCmd::Connect);

        let mut paint = LedFrame::new();
        paint.set_all_keys(Rgb::new(0x10, 0x10, 0x14));

        Self {
            link,
            connected: false,
            info: None,
            caps: None,
            status: "looking for keyboard...".into(),
            status_bad: false,
            mode: Mode::Animation,
            lighting: LightingState::default(),
            anim_kind: AnimationKind::Rainbow,
            anim_params: AnimParams::default(),
            anim_running: false,
            target_fps: 30,
            paint,
            paint_color: Rgb::new(0, 170, 255),
            selection: HashSet::new(),
            stats: EngineStats::default(),
            preview: [Rgb::BLACK; MATRIX_CELLS],
            user_presets: presets::load_user(),
            new_preset_name: String::new(),
        }
    }

    fn drain_events(&mut self) {
        while let Ok(evt) = self.link.events.try_recv() {
            match evt {
                DeviceEvt::Connected { info, caps, .. } => {
                    self.status = format!("{} - firmware {}", info.model, info.firmware);
                    self.status_bad = false;
                    self.connected = true;
                    self.info = Some(*info);
                    self.caps = caps;
                }
                DeviceEvt::Disconnected { reason } => {
                    self.connected = false;
                    self.anim_running = false;
                    self.status = format!("disconnected - {reason}");
                    self.status_bad = true;
                }
                DeviceEvt::Error(e) => {
                    self.status = e;
                    self.status_bad = true;
                }
                DeviceEvt::Stats(s) => self.stats = s,
            }
        }

        if let Ok(p) = self.link.preview.lock() {
            self.preview = p.frame.cells;
        }
    }

    /// What the on-screen board should show.
    ///
    /// For animations this is the exact frame that went to the hardware. For
    /// firmware effects the keyboard renders them itself and will not tell us
    /// what it is showing, so the board falls back to the chosen colour.
    fn board_colors(&self) -> [Rgb; MATRIX_CELLS] {
        match self.mode {
            Mode::Paint => self.paint.cells,
            Mode::Animation if self.anim_running => self.preview,
            Mode::Animation => {
                let mut f = LedFrame::new();
                let mut anim =
                    maxfit_core::Animator::new(self.anim_kind, self.anim_params);
                anim.render(0.0, 0.0, &mut f);
                f.cells
            }
            Mode::Firmware => {
                let mut f = LedFrame::new();
                if self.lighting.enabled {
                    if self.lighting.random_color {
                        for k in keys() {
                            f[k.led] = Rgb::from_hsv(k.nx, 0.9, 1.0);
                        }
                    } else {
                        let v = f32::from(self.lighting.brightness.max(1)) / 4.0;
                        f.set_all_keys(self.lighting.color.scale(v));
                    }
                }
                f.cells
            }
        }
    }

    fn apply_lighting(&mut self) {
        self.anim_running = false;
        self.link.send(DeviceCmd::SetLighting(self.lighting));
    }

    fn start_animation(&mut self) {
        self.anim_running = true;
        self.link.send(DeviceCmd::SetTargetFps(self.target_fps));
        self.link
            .send(DeviceCmd::StartAnimation(self.anim_kind, self.anim_params));
    }

    fn push_paint(&mut self) {
        self.link
            .send(DeviceCmd::ShowFrame(Box::new(self.paint.clone())));
    }
}

impl eframe::App for MaxFitApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();

        // Only ask the worker for repaints while the board is actually on screen.
        self.link
            .set_preview_wanted(self.anim_running && self.mode == Mode::Animation);

        egui::TopBottomPanel::top("top").show(ctx, |ui| self.top_bar(ui));
        egui::SidePanel::left("controls")
            .default_width(330.0)
            .width_range(300.0..=420.0)
            .show(ctx, |ui| self.controls(ui));
        egui::CentralPanel::default().show(ctx, |ui| self.center(ui));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // The worker restores a lit state; never leave the board blanked.
        self.link.shutdown();
    }
}

impl MaxFitApp {
    fn top_bar(&mut self, ui: &mut Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.heading(RichText::new("MaxFit Studio").color(ACCENT));
            ui.add_space(8.0);

            let dot = if self.connected { GOOD } else { BAD };
            ui.label(RichText::new("\u{25CF}").color(dot));
            ui.label(
                RichText::new(&self.status)
                    .color(if self.status_bad { BAD } else { Color32::GRAY }),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("Restore lights")
                    .on_hover_text(
                        "Static white at full brightness.\n\
                         Use this if the board ever goes dark - it is not broken, \
                         a frame just blanked it.",
                    )
                    .clicked()
                {
                    self.anim_running = false;
                    self.lighting = LightingState::panic();
                    self.link.send(DeviceCmd::Restore);
                }
                if self.anim_running {
                    ui.label(
                        RichText::new(format!(
                            "{:.0} fps  |  {:.1} ms/frame",
                            self.stats.actual_fps, self.stats.write_ms
                        ))
                        .color(Color32::GRAY)
                        .monospace(),
                    );
                }
            });
        });
        ui.add_space(6.0);
    }

    fn controls(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.mode, Mode::Firmware, "Firmware");
            ui.selectable_value(&mut self.mode, Mode::Animation, "Animations");
            ui.selectable_value(&mut self.mode, Mode::Paint, "Paint");
        });
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| match self.mode {
            Mode::Firmware => self.firmware_panel(ui),
            Mode::Animation => self.animation_panel(ui),
            Mode::Paint => self.paint_panel(ui),
        });
    }

    fn firmware_panel(&mut self, ui: &mut Ui) {
        ui.label(
            RichText::new(
                "Effects the keyboard runs by itself. They keep going with the app closed.",
            )
            .color(Color32::GRAY)
            .small(),
        );
        ui.add_space(8.0);

        // Only offer what this board actually advertises, so nothing silently
        // does nothing.
        let available: Vec<Effect> = match &self.caps {
            Some(c) if !c.effects.is_empty() => c.known_effects(),
            _ => Effect::all().collect(),
        };

        let mut changed = false;
        egui::Grid::new("fx").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label("Effect");
            egui::ComboBox::from_id_salt("effect")
                .selected_text(self.lighting.effect.name())
                .show_ui(ui, |ui| {
                    for e in &available {
                        if ui
                            .selectable_value(&mut self.lighting.effect, *e, e.name())
                            .changed()
                        {
                            changed = true;
                        }
                    }
                });
            ui.end_row();
        });

        changed |= color_row(ui, "Colour", &mut self.lighting.color);
        if !self.lighting.effect.ignores_random_color() {
            changed |= color_row(ui, "Secondary", &mut self.lighting.background);
        }

        let bmax = self.caps.as_ref().map_or(4, |c| c.brightness_max.max(1));
        let smax = self.caps.as_ref().map_or(4, |c| c.speed_max.max(1));
        changed |= ui
            .add(egui::Slider::new(&mut self.lighting.brightness, 0..=bmax).text("Brightness"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut self.lighting.speed, 0..=smax).text("Speed"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut self.lighting.direction, 0..=3).text("Direction"))
            .changed();
        changed |= ui
            .checkbox(&mut self.lighting.random_color, "Let the firmware cycle colours")
            .changed();

        ui.add_space(10.0);
        if ui.button("Apply").clicked() || changed {
            self.apply_lighting();
        }

        ui.add_space(4.0);
        if ui
            .button("Turn lighting off")
            .on_hover_text("Blanks the board. 'Restore lights' brings it back.")
            .clicked()
        {
            self.lighting.enabled = false;
            self.apply_lighting();
            self.lighting.enabled = true;
        }
    }

    fn animation_panel(&mut self, ui: &mut Ui) {
        ui.label(
            RichText::new(
                "Rendered on your PC and streamed to the board, about 125 frames a second. \
                 The vendor software cannot do this.",
            )
            .color(Color32::GRAY)
            .small(),
        );
        ui.add_space(8.0);

        ui.horizontal_wrapped(|ui| {
            for k in AnimationKind::ALL {
                if ui
                    .selectable_label(self.anim_kind == *k, k.name())
                    .clicked()
                {
                    self.anim_kind = *k;
                    if self.anim_running {
                        self.link.send(DeviceCmd::SetAnimationKind(*k));
                    }
                }
            }
        });

        ui.add_space(10.0);
        let mut changed = false;
        if !self.anim_kind.is_multicolor() {
            changed |= color_row(ui, "Colour", &mut self.anim_params.color_a);
        }
        if self.anim_kind.uses_color_b() {
            changed |= color_row(ui, "Second colour", &mut self.anim_params.color_b);
        }

        changed |= ui
            .add(egui::Slider::new(&mut self.anim_params.speed, -2.0..=2.0).text("Speed"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut self.anim_params.scale, 0.2..=4.0).text("Scale"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut self.anim_params.angle, -180.0..=180.0).text("Angle"))
            .changed();
        changed |= ui
            .add(
                egui::Slider::new(&mut self.anim_params.brightness, 0.0..=1.0).text("Brightness"),
            )
            .changed();

        if changed && self.anim_running {
            self.link
                .send(DeviceCmd::SetAnimationParams(self.anim_params));
        }

        ui.add_space(8.0);
        if ui
            .add(egui::Slider::new(&mut self.target_fps, 10..=120).text("Target fps"))
            .on_hover_text(
                "30 is plenty and leaves the USB bus free for your keystrokes. \
                 The board tops out near 125.",
            )
            .changed()
            && self.anim_running
        {
            self.link.send(DeviceCmd::SetTargetFps(self.target_fps));
        }

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if self.anim_running {
                if ui.button("Stop").clicked() {
                    self.anim_running = false;
                    self.link.send(DeviceCmd::StopAnimation);
                }
            } else if ui.button("Start").clicked() {
                self.start_animation();
            }
        });

        ui.add_space(14.0);
        ui.separator();
        ui.label(RichText::new("Presets").strong());
        ui.add_space(4.0);

        let mut chosen: Option<Preset> = None;
        ui.horizontal_wrapped(|ui| {
            for p in presets::BUILTIN.iter() {
                if ui.button(p.name.as_str()).clicked() {
                    chosen = Some(p.clone());
                }
            }
        });

        if !self.user_presets.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new("Yours").color(Color32::GRAY).small());
            let mut remove: Option<usize> = None;
            for (i, p) in self.user_presets.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.button(p.name.as_str()).clicked() {
                        chosen = Some(p.clone());
                    }
                    if ui.small_button("\u{2715}").on_hover_text("Delete").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                let p = self.user_presets.remove(i);
                presets::delete_user(&p);
            }
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.new_preset_name)
                    .hint_text("name")
                    .desired_width(150.0),
            );
            if ui.button("Save current").clicked() && !self.new_preset_name.trim().is_empty() {
                let p = Preset {
                    name: self.new_preset_name.trim().to_string(),
                    kind: self.anim_kind,
                    params: self.anim_params,
                };
                presets::save_user(&p);
                self.user_presets.push(p);
                self.new_preset_name.clear();
            }
        });

        if let Some(p) = chosen {
            self.anim_kind = p.kind;
            self.anim_params = p.params;
            self.start_animation();
        }
    }

    fn paint_panel(&mut self, ui: &mut Ui) {
        ui.label(
            RichText::new("Click or drag keys on the board. Right-click clears one.")
                .color(Color32::GRAY)
                .small(),
        );
        ui.add_space(8.0);

        color_row(ui, "Brush", &mut self.paint_color);

        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Fill all").clicked() {
                self.paint.set_all_keys(self.paint_color);
                self.push_paint();
            }
            if ui.button("Clear").clicked() {
                self.paint.clear();
                self.push_paint();
            }
            if ui.button("Rainbow").clicked() {
                for k in keys() {
                    self.paint[k.led] = Rgb::from_hsv(k.nx * 0.85 + k.ny * 0.12, 0.95, 1.0);
                }
                self.push_paint();
            }
        });

        ui.add_space(6.0);
        if ui
            .button("Fill selection")
            .on_hover_text("Keys you have clicked stay selected until you clear the selection.")
            .clicked()
        {
            for led in &self.selection {
                self.paint[*led] = self.paint_color;
            }
            self.push_paint();
        }
        if ui.button("Clear selection").clicked() {
            self.selection.clear();
        }

        ui.add_space(14.0);
        ui.separator();
        ui.label(RichText::new("Save to the keyboard").strong());
        ui.label(
            RichText::new(
                "Writes this layout into the board's own memory, so it survives on a \
                 PC with no software.",
            )
            .color(Color32::GRAY)
            .small(),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for slot in 0..3u8 {
                if ui.button(format!("Slot {}", slot + 1)).clicked() {
                    self.link.send(DeviceCmd::StoreFrame {
                        slot,
                        frame: Box::new(self.paint.clone()),
                    });
                }
            }
        });
    }

    fn center(&mut self, ui: &mut Ui) {
        ui.add_space(10.0);
        let colors = self.board_colors();
        let interactive = self.mode == Mode::Paint;
        let (_, action) = board::show(ui, &colors, &self.selection, interactive);

        if interactive && (!action.painted.is_empty() || !action.cleared.is_empty()) {
            for led in &action.painted {
                self.paint[*led] = self.paint_color;
                self.selection.insert(*led);
            }
            for led in &action.cleared {
                self.paint[*led] = Rgb::BLACK;
                self.selection.remove(led);
            }
            self.push_paint();
        }

        ui.add_space(10.0);
        if let Some(led) = action.hovered
            && let Some(k) = maxfit_core::layout().at_led(led)
        {
            ui.label(
                RichText::new(format!(
                    "{}   LED {}  (row {}, col {})   HID 0x{:02X}",
                    k.name, k.led, k.row, k.col, k.hid
                ))
                .monospace()
                .color(Color32::GRAY),
            );
        } else if let Some(info) = &self.info {
            ui.label(
                RichText::new(format!(
                    "{}   {}   serial {}",
                    info.model, info.firmware, info.serial
                ))
                .monospace()
                .color(Color32::DARK_GRAY),
            );
        }
    }
}

/// Colour swatch + hex, returning whether the user changed it.
fn color_row(ui: &mut Ui, label: &str, c: &mut Rgb) -> bool {
    let mut arr = [c.r, c.g, c.b];
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        if ui.color_edit_button_srgb(&mut arr).changed() {
            changed = true;
        }
        ui.label(RichText::new(c.to_hex()).monospace().color(Color32::GRAY));
    });
    if changed {
        *c = Rgb::new(arr[0], arr[1], arr[2]);
    }
    changed
}
