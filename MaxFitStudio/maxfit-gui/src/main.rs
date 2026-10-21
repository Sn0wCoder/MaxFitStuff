//! MaxFit Studio - lighting control for the Fantech MAXFIT61 (MK857).
//!
//! An open replacement for the vendor's FANTECH app, built on a protocol
//! recovered by reverse engineering it. See ../README.md.

// Keep the console in debug builds; a release build is a plain GUI app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod board;
mod presets;
mod theme;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 760.0])
            .with_min_inner_size([820.0, 560.0])
            .with_title("MaxFit Studio"),
        vsync: true,
        ..Default::default()
    };

    eframe::run_native(
        "MaxFit Studio",
        options,
        Box::new(|cc| Ok(Box::new(app::MaxFitApp::new(cc)))),
    )
}
