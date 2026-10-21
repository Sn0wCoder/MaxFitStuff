//! Read-only hardware check: enumerate, open the control interface, and dump
//! everything the keyboard will tell us about itself. Writes nothing.

use maxfit_core::protocol::{PRODUCT_ID, VENDOR_ID};
use maxfit_device::{Device, enumerate};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("interfaces for {VENDOR_ID:04x}:{PRODUCT_ID:04x}");
    for i in enumerate(VENDOR_ID, PRODUCT_ID)? {
        println!(
            "  usage {:04X}/{:04X}  if{:<3} {}{}",
            i.usage_page,
            i.usage,
            i.interface_number,
            if i.is_control() { "<CONTROL> " } else { "" },
            i.path
        );
    }

    let mut dev = Device::open()?;
    println!("\nopened {}", dev.path());

    let info = dev.read_device_info()?;
    println!("  model    : {}", info.model);
    println!("  firmware : {}", info.firmware);
    println!("  built    : {}", info.build_date);
    println!("  serial   : {}", info.serial);
    println!("  raw      : {}", info.raw);

    match dev.read_capabilities()? {
        Some(caps) => {
            println!(
                "\ncapabilities: brightness 0..{}, speed 0..{}, {} effects",
                caps.brightness_max,
                caps.speed_max,
                caps.effects.len()
            );
            for e in caps.known_effects() {
                println!("  0x{:02X}  {}", e as u8, e.name());
            }
        }
        None => println!("\nno capability reply"),
    }
    Ok(())
}
