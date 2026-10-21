# MaxFit Studio

A replacement for the Fantech MAXFIT61 (MK857) lighting software, written in
Rust with [egui](https://github.com/emilk/egui).

Fantech's own app gives you a fixed set of firmware effects and one fairly
clunky static "custom" layer. MaxFit Studio gives you all of that, plus effects
rendered on your PC. It turns out the keyboard is basically a 132-pixel RGB
screen that can handle around **125 frames per second**, so there's a lot more
you can do with it than the vendor lets on.

It's built on the protocol I reverse engineered in
[`../FanTechMaxFitCollection`](../FanTechMaxFitCollection). The full writeup is
in [`Docs/PROTOCOL.md`](../FanTechMaxFitCollection/Docs/PROTOCOL.md).

## Features

**Firmware effects.** All 14 effects your board says it supports, with colour,
brightness, speed and direction. These run on the keyboard itself, so they keep
going after you close the app.

**PC-rendered animations.** This is the part the vendor software can't do.
Rainbow, gradient, wave, plasma, fire, matrix rain, breathe, sweep and solid,
all drawn on your PC and streamed to the keyboard live. They use the actual
physical positions of the keycaps, so a diagonal gradient really runs
diagonally across the board instead of jumping around the LED grid.

**Paint mode.** Click or drag over keys to colour them one by one, then save
the result to one of the keyboard's onboard slots. It'll stay on the keyboard
even when you plug it into a machine without the app.

**Presets.** Eleven built-in ones, and you can save your own. Each one is its
own JSON file in `%APPDATA%\MaxFitStudio\presets`, so sharing a preset is as
simple as sending someone the file.

## Running it

```powershell
.\cargo.ps1 run -p maxfit-gui --release
```

### Why `cargo.ps1` instead of `cargo`?

If you've got Rust installed through both Chocolatey and rustup (I did), you
might trip over this. Chocolatey's Rust targets `x86_64-pc-windows-gnu`, and if
it comes first on your `PATH`, a plain `cargo build` quietly uses it and then
falls over inside `windows-sys` with:

```
error calling dlltool 'dlltool.exe': program not found
```

It looks like you're missing a tool, but really it's just the wrong toolchain.
`cargo.ps1` forces `RUSTC` and `RUSTDOC` to point at rustup's MSVC toolchain.

If you only have rustup, plain `cargo` works fine. If you have both, either run
`choco uninstall rust`, or move `%USERPROFILE%\.cargo\bin` above
`C:\ProgramData\chocolatey\bin` in your `PATH`.

## Project layout

| Crate | What it does |
|---|---|
| `maxfit-core` | Protocol encoding, key layout, colour maths, effect engine. Does no I/O at all, so you can test it without a keyboard. |
| `maxfit-device` | HID transport, the device worker thread and frame timing. The only crate that talks to USB. |
| `maxfit-gui` | The app itself. Doesn't know anything about the protocol. |

`tools/gen_keys.py` rebuilds `maxfit-core/data/keys61.json` from
`vendor-data/KeyInfo_XS61BRGB.config`, which is the vendor software's own
layout file. That's where every key's HID usage, LED index and keycap position
comes from.

## Design notes

**Effects run on their own thread, not inside the GUI.** egui only runs its
update loop while the window is visible, so if the effects were driven from the
UI they'd freeze as soon as you minimised. The worker thread keeps its own
clock instead.

**Frame timing uses a high-resolution waitable timer.** On Windows,
`thread::sleep` rounds to the ~15.6 ms system tick, so a simple 60 Hz loop ends
up running at about 32 Hz with lots of jitter. That's easy to mistake for a USB
speed problem. `maxfit-device/src/timer.rs` declares the three kernel32
functions it needs by hand instead of pulling in the `windows` crate, which
keeps `raw-dylib` (and its need for `dlltool`) out of the build.

**30 fps by default, not 60.** The keyboard is a composite USB device, so your
actual keystrokes share the same pipe as the lighting data. 30 fps uses roughly
a quarter of the bandwidth and leaves plenty of room. The slider goes up to 120
if you want to push it.

**No delay between packets.** The C++ driver waits 2 ms between each of the 8
colour packets, which eats about 44% of a 30 fps frame. There's no need for it:
the writes already block, and USB enforces its own timing. Without the delay I
measured 8.02 ms per frame.

**It can't send flash commands, by design.** `0x0B` (enter bootloader) and
`0x0C` (flash firmware) simply don't exist in the `Command` enum, so there's no
way for any encoder to produce them. The test
`no_encoder_can_emit_a_flash_command` checks this across every encoder's
possible inputs.

**Byte 17 is backwards.** `ledOpenOrClose` is `0` for lights on and `1` for
lights off, the opposite of what you'd guess. Get it wrong and the backlight
goes dark until a frame with the byte cleared arrives, which looks exactly like
the keyboard died. To guard against that, `LightingState::enabled` defaults to
`true` (including when a preset leaves it out), and the worker won't shut down
while lighting is disabled. If your board ever does go dark, hit
**Restore lights**.

## Tests

```powershell
.\cargo.ps1 test
```

21 tests covering the packet format, the LED grid, key geometry and colour
parsing. None of them need hardware. They're ported from the C++ driver's test
suite, so both versions are held to the exact same bytes.

There are also two examples that do talk to the keyboard. Both are mostly
read-only:

```powershell
.\cargo.ps1 run -p maxfit-device --example probe     # read-only: identity + capabilities
.\cargo.ps1 run -p maxfit-device --example rainbow   # draws a rainbow and measures throughput
```