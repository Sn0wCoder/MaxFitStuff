//! RGB colour: parsing, formatting, and the small amount of colour maths the
//! effect engine needs.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Colour names accepted wherever a colour can be typed.
pub const NAMED: &[(&str, Rgb)] = &[
    ("black", Rgb::new(0, 0, 0)),
    ("off", Rgb::new(0, 0, 0)),
    ("white", Rgb::new(255, 255, 255)),
    ("red", Rgb::new(255, 0, 0)),
    ("green", Rgb::new(0, 128, 0)),
    ("lime", Rgb::new(0, 255, 0)),
    ("blue", Rgb::new(0, 0, 255)),
    ("cyan", Rgb::new(0, 255, 255)),
    ("teal", Rgb::new(0, 128, 128)),
    ("magenta", Rgb::new(255, 0, 255)),
    ("pink", Rgb::new(255, 105, 180)),
    ("purple", Rgb::new(128, 0, 255)),
    ("orange", Rgb::new(255, 110, 0)),
    ("yellow", Rgb::new(255, 255, 0)),
    ("gold", Rgb::new(255, 190, 0)),
    ("mint", Rgb::new(0, 255, 160)),
    ("azure", Rgb::new(0, 170, 255)),
    ("violet", Rgb::new(160, 0, 255)),
];

impl Rgb {
    pub const BLACK: Rgb = Rgb::new(0, 0, 0);
    pub const WHITE: Rgb = Rgb::new(255, 255, 255);

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn is_black(self) -> bool {
        self.r == 0 && self.g == 0 && self.b == 0
    }

    /// Accepts `#RRGGBB`, `#RGB`, bare `RRGGBB`, `r,g,b`, or a name from [`NAMED`].
    pub fn parse(s: &str) -> Option<Rgb> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        let lower = s.to_ascii_lowercase();
        if let Some(&(_, c)) = NAMED.iter().find(|(n, _)| *n == lower) {
            return Some(c);
        }

        if s.contains(',') {
            let parts: Vec<_> = s.split(',').map(str::trim).collect();
            if parts.len() == 3 {
                let v: Option<Vec<u8>> = parts.iter().map(|p| p.parse::<u8>().ok()).collect();
                if let Some(v) = v {
                    return Some(Rgb::new(v[0], v[1], v[2]));
                }
            }
            return None;
        }

        let hex = s.strip_prefix('#').unwrap_or(s);
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        match hex.len() {
            // #RGB is shorthand: each nibble doubled, so `f0a` is `ff00aa`.
            3 => {
                let d = |i: usize| -> u8 {
                    let n = u8::from_str_radix(&hex[i..i + 1], 16).unwrap_or(0);
                    n * 17
                };
                Some(Rgb::new(d(0), d(1), d(2)))
            }
            6 => {
                let d = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
                Some(Rgb::new(d(0), d(2), d(4)))
            }
            _ => None,
        }
    }

    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    pub fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
        Rgb::new(m(a.r, b.r), m(a.g, b.g), m(a.b, b.b))
    }

    /// Multiplies each channel, for dimming. `f` is clamped to 0..=1.
    pub fn scale(self, f: f32) -> Rgb {
        let f = f.clamp(0.0, 1.0);
        let m = |x: u8| (f32::from(x) * f).round() as u8;
        Rgb::new(m(self.r), m(self.g), m(self.b))
    }

    /// `h` is in turns (0..1 wraps), `s` and `v` are 0..1.
    pub fn from_hsv(h: f32, s: f32, v: f32) -> Rgb {
        let h = h.rem_euclid(1.0) * 6.0;
        let s = s.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);
        let i = h.floor();
        let f = h - i;
        let p = v * (1.0 - s);
        let q = v * (1.0 - s * f);
        let t = v * (1.0 - s * (1.0 - f));
        let (r, g, b) = match i as i32 % 6 {
            0 => (v, t, p),
            1 => (q, v, p),
            2 => (p, v, t),
            3 => (p, q, v),
            4 => (t, p, v),
            _ => (v, p, q),
        };
        let m = |x: f32| (x * 255.0).round().clamp(0.0, 255.0) as u8;
        Rgb::new(m(r), m(g), m(b))
    }
}

impl std::fmt::Display for Rgb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hex())
    }
}
