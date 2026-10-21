//! Paints a hue sweep across the board using each key's real physical position,
//! then streams it with 0x0F. Exercises the whole write path end to end.
//!
//! Reset with: maxfitctl effect static --color white --brightness 4

use std::time::Instant;

use maxfit_core::{LedFrame, Rgb, keys};
use maxfit_device::Device;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut dev = Device::open()?;
    println!("opened {}", dev.path());

    let mut frame = LedFrame::new();
    for k in keys() {
        // Hue follows the key's true horizontal position, with a slight vertical
        // tilt so the rows do not all read as one flat band.
        frame[k.led] = Rgb::from_hsv(k.nx * 0.85 + k.ny * 0.12, 0.95, 1.0);
    }

    let t = Instant::now();
    dev.present_frame(&frame)?;
    let once = t.elapsed();

    // How fast can we actually stream? This is the ceiling for the effect engine.
    let n = 60;
    let t = Instant::now();
    for _ in 0..n {
        dev.present_frame(&frame)?;
    }
    let per = t.elapsed() / n;

    println!("first frame : {:.2} ms", once.as_secs_f32() * 1000.0);
    println!(
        "sustained   : {:.2} ms/frame  ({:.0} fps ceiling, 8 packets each)",
        per.as_secs_f32() * 1000.0,
        1.0 / per.as_secs_f32()
    );
    Ok(())
}
