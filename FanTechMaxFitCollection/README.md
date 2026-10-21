# FanTech MaxFit Collection

A replacement for the Fantech MK857 / MAXFIT61 lighting software. It talks to
the keyboard directly over its own HID protocol, so you don't need the vendor
app, a driver install, or anything running in the background.

Tested on: `MAXFIT61 Mechanical Keyboard`, `0416:C345`, model `XS61BRGB`,
firmware `V1.03.07`.

## What's in here

| Project | Type | What it does |
|---|---|---|
| `Driver` | static lib | the core bits: HID transport, protocol encoding, key/LED map |
| `Frontend` | console app | `maxfitctl`, the command-line tool you'll actually use |
| `Tests` | console app | `maxfit_tests`, protocol tests that don't need a keyboard plugged in |

There's also:

- `docs/PROTOCOL.md`: the full protocol writeup, including how I reverse
  engineered it.
- `docs/gen_keymap.py`: regenerates `Driver/src/KeyMap.cpp` from the vendor
  software's own layout file.

## Building

Open it in Visual Studio 2022 (v143 toolset), or build from a terminal:

```
msbuild FanTechMaxFitCollection.sln /p:Configuration=Release /p:Platform=x64
```

You'll find the binaries in `build\x64\Release\`.

Want 32-bit? Use `/p:Platform=x86`. (The solution calls it `x86`, the projects
call it `Win32`, so the output goes to `build\Win32\...`.)

No external dependencies. It only uses `setupapi` and `hid`, which come with
the Windows SDK.

To run the tests:

```
build\x64\Release\maxfit_tests.exe
```

## Usage

Getting info:

```
maxfitctl info       # model, firmware, serial, what lighting it supports
maxfitctl list       # all HID interfaces, with the control one marked
maxfitctl effects    # effect list, with the ones your board supports marked
maxfitctl keys       # key names and where they sit in the LED grid
```

Effects:

```
maxfitctl effect static --color #00AAFF
maxfitctl effect breath --color red --speed 3 --brightness 4
maxfitctl effect wave --random                  # let the keyboard cycle colours itself
maxfitctl effect reactive --color white --bg #101010
```

Per-key colours:

```
maxfitctl all #202020                           # whole keyboard one colour
maxfitctl key W=red A=red S=red D=red --base #0A0A0A
maxfitctl off
```

Colours can be written as `#RRGGBB`, `#RGB`, `RRGGBB`, `R,G,B`, or just a name
like `red`, `cyan`, `orange` or `purple`.

Brightness and speed go from `0` to `4`. That's not me being stingy; it's the
maximum the keyboard itself reports.

`all` and `key` switch the keyboard to its `Custom` effect before uploading,
since otherwise whatever animation is running would paint straight over your
colours. Add `--no-select` if you want to skip that, or `--store <slot>` to
save the colours to one of the keyboard's onboard slots instead of showing them
right away.

## Using it as a library

```cpp
#include "maxfit/Keyboard.h"

maxfit::Keyboard kb;
kb.open();                                  // throws maxfit::Error if no keyboard is found

auto info = kb.readDeviceInfo();            // model, firmware, serial
auto caps = kb.readLightCapabilities();     // supported effects, max speed/brightness

maxfit::LightingState s;
s.effect = maxfit::Effect::Breath;
s.color = {0, 170, 255};
s.brightness = 4;
s.speed = 3;
kb.setLighting(s);

maxfit::LedFrame frame;
frame.setAllKeys({10, 10, 10});
frame.setKey("Escape", {255, 0, 0});
kb.setPerKeyColors(frame);
```

## Is this safe?

For lighting, yes.

The firmware also has commands for entering the bootloader (`0x0B`) and
flashing firmware (`0x0C`). I left those out on purpose. Getting them wrong
bricks the keyboard, and there's no point when Fantech's own
`Fantech_Maxfit61_Patch_*.exe` already handles firmware updates.

Lighting settings are saved on the keyboard itself, so they stick around after
a reboot and after `maxfitctl` closes. Nothing in this project touches the
serial number, key mappings or onboard profiles.