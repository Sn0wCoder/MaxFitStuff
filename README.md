# MaxFitStuff

[![CI](https://github.com/Sn0wCoder/MaxFitStuff/actions/workflows/ci.yml/badge.svg)](https://github.com/Sn0wCoder/MaxFitStuff/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Sn0wCoder/MaxFitStuff?include_prereleases&label=release)](https://github.com/Sn0wCoder/MaxFitStuff/releases)
![Platform](https://img.shields.io/badge/platform-Windows-0078D6?logo=windows)
![Rust](https://img.shields.io/badge/Rust-egui-B7410E?logo=rust)
![C++](https://img.shields.io/badge/C%2B%2B-MSVC-00599C?logo=cplusplus)
![Firmware](https://img.shields.io/badge/tested%20on-V1.03.07-lightgrey)

A lightweight, open replacement for Fantech's software for the **MAXFIT61 (MK857)** keyboard. No vendor app, no background service, no driver install. It talks to the keyboard directly over its own HID protocol, which I reverse engineered from the official software.

Tested on: `MAXFIT61 Mechanical Keyboard`, `0416:C345`, model `XS61BRGB`, firmware `V1.03.07`.

## What's in here

| Folder | What it is |
|---|---|
| [`MaxFitStudio`](MaxFitStudio) | **The app.** A Rust/egui GUI with firmware effects, live PC-rendered animations, per-key painting and presets. Start here if you just want to control your lights. |
| [`FanTechMaxFitCollection`](FanTechMaxFitCollection) | **The driver.** A C++ library plus `maxfitctl`, a command-line tool. Use it if you want to script the keyboard or build something of your own on top. |

The full protocol writeup, covering how it was reversed and what every byte does, is in [`FanTechMaxFitCollection/Docs/PROTOCOL.md`](FanTechMaxFitCollection/Docs/PROTOCOL.md).

## Quick start

**GUI** (needs Rust via rustup):

```powershell
cd MaxFitStudio
.\cargo.ps1 run -p maxfit-gui --release
```

**Command line** (needs Visual Studio 2022):

```powershell
cd FanTechMaxFitCollection
msbuild FanTechMaxFitCollection.sln /p:Configuration=Release /p:Platform=x64
build\x64\Release\maxfitctl.exe info
```

Prebuilt binaries are on the [Releases](../../releases) page.

## Why

The official FANTECH app is a 2022 Qt bundle that gives you a handful of fixed effects and one clunky custom layer. It turns out the keyboard is actually a 132-pixel RGB screen that can run at about 125 fps, which the vendor software never takes advantage of.

## Safety

Only lighting is written to the keyboard. The firmware's bootloader and flash commands (`0x0B` / `0x0C`) are deliberately left out of both projects, so neither can brick your board. Key remapping is documented but not implemented yet. See the protocol doc for why.

That said, this is unofficial software built by reverse engineering, and it's only been tested on the hardware and firmware listed above. You use it at your own risk. I'm not responsible for anything that happens to your keyboard, including if it stops working.

## Not affiliated

This project has nothing to do with Fantech. "Fantech", "MAXFIT61" and "MK857" are their trademarks, used here only to say which hardware this works with.

## License

Apache 2.0, see [LICENSE](LICENSE). Use it however you like, closed source included. Just keep the license and credit this repo.

The files in `MaxFitStudio/vendor-data/` come from Fantech's software and aren't covered by this license.