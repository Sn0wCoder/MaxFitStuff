//! PC-rendered lighting effects.
//!
//! This is the thing the vendor software cannot do. Because command `0x0F`
//! applies a per-key colour frame immediately, the keyboard is really a 132-cell
//! RGB framebuffer that sustains ~125 frames/second. So instead of picking from
//! the firmware's fixed 17 effects, we render whatever we like on the PC and
//! stream it.
//!
//! Effects work in *physical* space: every key carries a normalised centre
//! (`nx`, `ny`) derived from the vendor's own keycap geometry, so a gradient
//! crosses the board at a true angle rather than stepping through a sparse
//! matrix. `ny` grows downward, matching the vendor coordinate system.

use serde::{Deserialize, Serialize};

use crate::color::Rgb;
use crate::frame::LedFrame;
use crate::keys::{self, Key};

/// Which animation to run. Serialised into presets by name.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationKind {
    /// Two colours blended across the board at an angle.
    Gradient,
    /// Full hue sweep -- the one that looks like the box art.
    Rainbow,
    /// Travelling brightness wave.
    Wave,
    /// Classic interference plasma.
    Plasma,
    /// Flames rising from the bottom row.
    Fire,
    /// Falling glyph trails, one per column.
    Rain,
    /// The whole board pulsing together.
    Breathe,
    /// A bright bar sweeping across and wrapping.
    Sweep,
    /// A single flat colour, rendered host-side.
    Solid,
}

impl AnimationKind {
    pub fn name(self) -> &'static str {
        match self {
            AnimationKind::Gradient => "Gradient",
            AnimationKind::Rainbow => "Rainbow",
            AnimationKind::Wave => "Wave",
            AnimationKind::Plasma => "Plasma",
            AnimationKind::Fire => "Fire",
            AnimationKind::Rain => "Rain",
            AnimationKind::Breathe => "Breathe",
            AnimationKind::Sweep => "Sweep",
            AnimationKind::Solid => "Solid",
        }
    }

    pub const ALL: &'static [AnimationKind] = &[
        AnimationKind::Rainbow,
        AnimationKind::Gradient,
        AnimationKind::Wave,
        AnimationKind::Plasma,
        AnimationKind::Fire,
        AnimationKind::Rain,
        AnimationKind::Breathe,
        AnimationKind::Sweep,
        AnimationKind::Solid,
    ];

    /// Whether `color_b` is meaningful for this effect, so the UI can hide it.
    pub fn uses_color_b(self) -> bool {
        matches!(self, AnimationKind::Gradient | AnimationKind::Sweep)
    }

    /// Whether the effect derives its own hues and ignores `color_a`.
    pub fn is_multicolor(self) -> bool {
        matches!(
            self,
            AnimationKind::Rainbow | AnimationKind::Plasma | AnimationKind::Fire
        )
    }
}

/// Shared tuning for every animation. One struct keeps presets and the
/// parameter UI uniform; effects simply ignore what they do not use.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct AnimParams {
    pub color_a: Rgb,
    pub color_b: Rgb,
    /// Cycles per second. Negative runs the effect backwards.
    pub speed: f32,
    /// Feature size; larger means more repetitions across the board.
    pub scale: f32,
    /// Direction in degrees, for the effects that travel.
    pub angle: f32,
    /// Master output multiplier, 0..1.
    pub brightness: f32,
}

impl Default for AnimParams {
    fn default() -> Self {
        Self {
            color_a: Rgb::new(0, 170, 255),
            color_b: Rgb::new(255, 0, 170),
            speed: 0.25,
            scale: 1.0,
            angle: 0.0,
            brightness: 1.0,
        }
    }
}

/// Tiny xorshift PRNG. Avoids a dependency for the handful of effects that
/// need jitter, and being deterministic makes effects reproducible.
struct Rng(u32);

impl Rng {
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// Uniform in 0.0..1.0.
    fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1 << 24) as f32
    }
}

/// Smooth, cheap value noise over a 2D point, good enough for plasma and fire.
fn wobble(x: f32, y: f32, t: f32) -> f32 {
    let a = (x * 2.3 + t).sin();
    let b = (y * 3.1 - t * 0.8).sin();
    let c = ((x + y) * 1.7 + t * 0.6).sin();
    let d = ((x * x + y * y).sqrt() * 4.0 - t * 1.3).sin();
    (a + b + c + d) * 0.25 // -1..1
}

/// Maps 0..1 onto a black -> red -> orange -> yellow -> white ramp.
fn fire_palette(v: f32) -> Rgb {
    let v = v.clamp(0.0, 1.0);
    match v {
        v if v < 0.35 => Rgb::lerp(Rgb::BLACK, Rgb::new(180, 20, 0), v / 0.35),
        v if v < 0.65 => Rgb::lerp(Rgb::new(180, 20, 0), Rgb::new(255, 110, 0), (v - 0.35) / 0.30),
        v if v < 0.88 => Rgb::lerp(Rgb::new(255, 110, 0), Rgb::new(255, 215, 40), (v - 0.65) / 0.23),
        v => Rgb::lerp(Rgb::new(255, 215, 40), Rgb::WHITE, (v - 0.88) / 0.12),
    }
}

/// Projects a key onto the travel axis, so angle actually rotates the effect.
fn axis(k: &Key, angle_deg: f32) -> f32 {
    let (s, c) = angle_deg.to_radians().sin_cos();
    k.nx * c + k.ny * s
}

/// Per-column state for [`AnimationKind::Rain`].
struct Drop {
    head: f32,
    speed: f32,
}

/// Holds an animation plus whatever state it needs between frames.
pub struct Animator {
    pub kind: AnimationKind,
    pub params: AnimParams,
    rng: Rng,
    drops: Vec<Drop>,
}

impl Animator {
    pub fn new(kind: AnimationKind, params: AnimParams) -> Self {
        let mut rng = Rng(0x5EED_1234);
        let drops = (0..keys::MATRIX_COLS)
            .map(|_| Drop {
                head: -rng.next_f32() * 3.0,
                speed: 0.6 + rng.next_f32() * 1.4,
            })
            .collect();
        Self {
            kind,
            params,
            rng,
            drops,
        }
    }

    /// Renders one frame. `t` is seconds since the effect started, `dt` the time
    /// since the previous frame.
    pub fn render(&mut self, t: f32, dt: f32, out: &mut LedFrame) {
        out.clear();
        let p = self.params;
        let keys = keys::keys();

        match self.kind {
            AnimationKind::Solid => {
                for k in keys {
                    out[k.led] = p.color_a;
                }
            }

            AnimationKind::Gradient => {
                for k in keys {
                    // Triangle wave so the gradient ping-pongs instead of
                    // snapping back to colour A at the wrap point.
                    let u = (axis(k, p.angle) * p.scale + t * p.speed).rem_euclid(1.0);
                    let tri = if u < 0.5 { u * 2.0 } else { (1.0 - u) * 2.0 };
                    out[k.led] = Rgb::lerp(p.color_a, p.color_b, tri);
                }
            }

            AnimationKind::Rainbow => {
                for k in keys {
                    let h = axis(k, p.angle) * p.scale + t * p.speed;
                    out[k.led] = Rgb::from_hsv(h, 0.95, 1.0);
                }
            }

            AnimationKind::Wave => {
                for k in keys {
                    let u = axis(k, p.angle) * p.scale - t * p.speed;
                    // 0..1 brightness envelope travelling along the axis.
                    let v = (u * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                    out[k.led] = p.color_a.scale(v);
                }
            }

            AnimationKind::Plasma => {
                for k in keys {
                    let v = wobble(k.nx * 3.0 * p.scale, k.ny * 3.0 * p.scale, t * p.speed * 4.0);
                    out[k.led] = Rgb::from_hsv(v * 0.5 + t * p.speed * 0.1, 0.9, 1.0);
                }
            }

            AnimationKind::Fire => {
                for k in keys {
                    // ny grows downward, so the bottom row is the hottest.
                    let heat = k.ny.powf(1.5);
                    let flicker =
                        wobble(k.nx * 6.0 * p.scale, k.ny * 3.0, t * p.speed * 6.0) * 0.35;
                    out[k.led] = fire_palette(heat + flicker);
                }
            }

            AnimationKind::Rain => {
                // Advance one head per column, then light keys near it.
                for d in &mut self.drops {
                    d.head += dt * d.speed * p.speed * 8.0;
                    if d.head > keys::MATRIX_ROWS as f32 + 2.0 {
                        d.head = -self.rng.next_f32() * 3.0;
                        d.speed = 0.6 + self.rng.next_f32() * 1.4;
                    }
                }
                for k in keys {
                    let d = &self.drops[k.col as usize];
                    let dist = d.head - f32::from(k.row);
                    if (0.0..4.0).contains(&dist) {
                        // Brightest at the head, fading along the tail.
                        let v = 1.0 - dist / 4.0;
                        out[k.led] = if dist < 0.5 {
                            Rgb::lerp(p.color_a, Rgb::WHITE, 0.7)
                        } else {
                            p.color_a.scale(v * v)
                        };
                    }
                }
            }

            AnimationKind::Breathe => {
                let v = ((t * p.speed * std::f32::consts::TAU).sin() * 0.5 + 0.5).powf(1.8);
                // Never fully dark: a board that blinks off reads as broken.
                let v = 0.06 + v * 0.94;
                for k in keys {
                    out[k.led] = p.color_a.scale(v);
                }
            }

            AnimationKind::Sweep => {
                let head = (t * p.speed).rem_euclid(1.0);
                for k in keys {
                    let u = axis(k, p.angle);
                    // Wrapped distance, so the bar crosses the seam cleanly.
                    let mut d = (u - head).abs();
                    d = d.min(1.0 - d);
                    let v = (1.0 - d * 8.0 / p.scale.max(0.05)).clamp(0.0, 1.0);
                    out[k.led] = Rgb::lerp(p.color_b, p.color_a, v);
                }
            }
        }

        if p.brightness < 0.999 {
            for k in keys {
                out[k.led] = out[k.led].scale(p.brightness);
            }
        }
    }
}
