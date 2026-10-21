//! Preset library: curated ones that ship with the app, plus whatever the user
//! saves.
//!
//! User presets are one JSON file each under `%APPDATA%\MaxFitStudio\presets`,
//! which makes them trivially shareable — drop a file in the folder and it
//! shows up.

use std::path::PathBuf;
use std::sync::LazyLock;

use maxfit_core::effects::{AnimParams, AnimationKind};
use maxfit_core::Rgb;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub kind: AnimationKind,
    pub params: AnimParams,
}

fn preset(
    name: &str,
    kind: AnimationKind,
    color_a: Rgb,
    color_b: Rgb,
    speed: f32,
    scale: f32,
    angle: f32,
) -> Preset {
    Preset {
        name: name.to_string(),
        kind,
        params: AnimParams {
            color_a,
            color_b,
            speed,
            scale,
            angle,
            brightness: 1.0,
        },
    }
}

/// Hand-tuned starting points. Each one is a real answer to "what should my
/// keyboard look like", rather than a parameter dump.
pub static BUILTIN: LazyLock<Vec<Preset>> = LazyLock::new(|| {
    use AnimationKind as K;
    vec![
        preset("Rainbow Flow", K::Rainbow, Rgb::WHITE, Rgb::BLACK, 0.12, 1.0, 0.0),
        preset("Hyperdrive", K::Rainbow, Rgb::WHITE, Rgb::BLACK, 0.9, 2.2, 0.0),
        preset(
            "Ocean Drift",
            K::Gradient,
            Rgb::new(0, 40, 120),
            Rgb::new(0, 220, 255),
            0.10,
            1.0,
            12.0,
        ),
        preset(
            "Sunset",
            K::Gradient,
            Rgb::new(255, 90, 0),
            Rgb::new(200, 0, 140),
            0.08,
            1.0,
            -8.0,
        ),
        preset(
            "Vapourwave",
            K::Gradient,
            Rgb::new(255, 60, 190),
            Rgb::new(70, 230, 255),
            0.18,
            1.3,
            20.0,
        ),
        preset("Campfire", K::Fire, Rgb::WHITE, Rgb::BLACK, 0.5, 1.0, 0.0),
        preset(
            "Matrix",
            K::Rain,
            Rgb::new(0, 255, 90),
            Rgb::BLACK,
            0.35,
            1.0,
            0.0,
        ),
        preset(
            "Lava Lamp",
            K::Plasma,
            Rgb::WHITE,
            Rgb::BLACK,
            0.10,
            0.8,
            0.0,
        ),
        preset(
            "Heartbeat",
            K::Breathe,
            Rgb::new(255, 30, 40),
            Rgb::BLACK,
            0.55,
            1.0,
            0.0,
        ),
        preset(
            "Cyber Sweep",
            K::Sweep,
            Rgb::new(0, 230, 255),
            Rgb::new(8, 8, 20),
            0.45,
            1.0,
            0.0,
        ),
        preset(
            "Deep Work",
            K::Solid,
            Rgb::new(255, 150, 40),
            Rgb::BLACK,
            0.0,
            1.0,
            0.0,
        ),
    ]
});

pub fn presets_dir() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(appdata).join("MaxFitStudio").join("presets"))
}

/// Turns a preset name into a safe file stem.
fn slug(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect()
}

pub fn load_user() -> Vec<Preset> {
    let Some(dir) = presets_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new(); // No folder yet simply means no saved presets.
    };

    let mut out = Vec::new();
    for e in entries.flatten() {
        if e.path().extension().is_none_or(|x| x != "json") {
            continue;
        }
        // One unreadable file must not hide the rest.
        if let Ok(text) = std::fs::read_to_string(e.path())
            && let Ok(p) = serde_json::from_str::<Preset>(&text)
        {
            out.push(p);
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub fn save_user(p: &Preset) {
    let Some(dir) = presets_dir() else { return };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    if let Ok(text) = serde_json::to_string_pretty(p) {
        let _ = std::fs::write(dir.join(format!("{}.json", slug(&p.name))), text);
    }
}

pub fn delete_user(p: &Preset) {
    if let Some(dir) = presets_dir() {
        let _ = std::fs::remove_file(dir.join(format!("{}.json", slug(&p.name))));
    }
}
