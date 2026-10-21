# Fantech MAXFIT61 (MK857) vendor HID protocol

Reverse engineered from `Maxfit61_MK857_Software_V1.3_23110600.exe` and verified
against live hardware (model `XS61BRGB`, firmware `V1.03.07`).

## 1. How this was recovered

The vendor installer is **Inno Setup 5.6**; its payload is a **Qt 5** application
(`app/FANTECH.exe`, 32-bit, built 2022-06-27) that links **hidapi**. Only these
hidapi entry points are imported:

```
hid_init  hid_exit  hid_enumerate  hid_free_enumeration  hid_open_path
hid_write hid_read  hid_set_nonblocking  hid_close  hid_error
```

No `hid_*_feature_report`, so the whole protocol is plain output/input reports.
Every `hid_write` call site passes length `0x40`.

Two sources inside the binary made the decode straightforward:

* The app ships verbose English log strings (`USB serial port [Write Led] ...`,
  `Inquire [Device SN 0x15]`, `ReadFile 0x0d`), which name almost every command.
* Its Qt resource bundle (397 files, recoverable by walking the resource
  name/tree/data blobs at `0xAF4FA8` / `0xAF8F38` / `0x458898`) contains the
  per-model key layouts (`KeyInfo_XS61BRGB.config`) and default lighting
  profiles (`customLightDefaultConfig_61.xml`). The profile XML attribute order
  mirrors the 14-field `LightInfo` struct passed to the lighting encoder, which
  is what names the individual payload bytes.

## 2. Transport

The keyboard is a composite USB device, `VID 0x0416` (Nuvoton) / `PID 0xC345`,
product string `MAXFIT61 Mechanical Keyboard`. It exposes six HID interfaces;
exactly one is the control channel:

| Interface | Usage page | Usage | In | Out | Role |
|---|---|---|---|---|---|
| `MI_00` | 0x0001 | 0x0006 | 9 | 2 | boot keyboard |
| `MI_01 COL01` | 0x0001 | 0x0080 | 2 | 0 | system control |
| `MI_01 COL02` | 0x000C | 0x0001 | 4 | 0 | consumer control |
| `MI_01 COL03` | 0x0001 | 0x0006 | 21 | 0 | NKRO keyboard |
| `MI_01 COL04` | 0x0001 | 0x0002 | 7 | 0 | mouse (Fn layer) |
| **`MI_02`** | **0xFF1B** | **0x0091** | **64** | **64** | **vendor control** |

Select it by usage page/usage rather than by path. The vendor app instead
filters device paths for the substrings `mi_02` or `col06`, and correspondingly
uses report ID `0x01` normally but `0x06` for a `col06` interface. The MAXFIT61
is an `mi_02` device, so **report ID `0x01`**.

Open the interface with `FILE_FLAG_OVERLAPPED` and use timed reads: several
queries answer with more reports than you asked for, and some produce none.

## 3. Frame format

All traffic is fixed 64-byte reports, in both directions.

```
offset  field     notes
  0     reportId  always 0x01
  1     command   echoed back in the reply
  2     zone      LED zone, or custom-profile slot
  3     reserved  always 0
  4     page      packet index for multi-packet payloads
  5     length    count of meaningful bytes starting at offset 6
  6..63 payload   58 bytes maximum
```

## 4. Command set

Recovered by scanning for the compiler idiom that writes `buf[0]=1; buf[1]=cmd`
into a freshly zeroed 64-byte stack buffer, then naming each site from its
nearby log string.

| Cmd | Name | Notes |
|---|---|---|
| 0x01 | Handshake | startup liveness probe |
| 0x02 | Query side-light modes | |
| 0x04 | Write 6KRO key map | |
| 0x05 | Write NKRO key map | |
| 0x06 | Write multimedia map | |
| **0x07** | **Set lighting** | effect, colours, brightness, speed |
| 0x08 | Set side lighting | zone/colour stream, same shape as 0x0F |
| **0x09** | **Write custom LED profile** | per-key colours into onboard slot |
| **0x0A** | **Query light modes** | supported effects + speed/brightness maxima |
| 0x0B | **Enter bootloader** | ⚠ not implemented here — bricking risk |
| 0x0C | **Flash firmware block** | ⚠ not implemented here — bricking risk |
| **0x0D** | **Query device info** | model + firmware, then build date |
| 0x0F | **Set per-key colours** | applied immediately |
| 0x13 | Write mouse-key map | |
| 0x14 | Reset / restore profile | |
| **0x15** | **Query serial number** | |
| 0x16 | Write serial number | |
| 0x17 | Enter light synchronisation | |
| 0x18 | Write onboard data | |
| 0x19 | Onboard download (Fn layers) | |
| 0x41–0x48 | Mouse-specific | DPI, mouse lighting; N/A for this keyboard |

Commands `0x0B` and `0x0C` are deliberately absent from `Protocol.h`.

## 5. Queries

### 0x0D — device info

Request carries no payload. The firmware answers with **two** reports:

```
page 0: 01 0d 00 00 00 32  "M252,01,KB,TY,XS61BRGB,V1.03.07"
page 1: 01 0d 00 00 01 32  "Feb 24 2022,15:23:58"
```

The field before the `V...` version is the model name; the vendor app uses it to
pick which `KeyInfo_*.config` layout to load.

### 0x15 — serial number

```
01 15 00 00 00 20  "edc417234160467195115e5e0f18161d"
```

### 0x0A — light capabilities

```
01 0a 00 00 00 12 | 02 01 03 09 04 00 0e 0f 06 07 0b 0c 10 08  a5 5a  04 04
                    \__________ supported effect ids _________/  magic  │  │
                                                                 speedMax│
                                                                  brightnessMax
```

Parse rule (copied from the vendor app): append payload bytes to the effect list
until a `0xA5` **or** `0x5A` byte appears; the next two bytes after the marker
are `speedMax` and `brightnessMax`. This unit reports **14 effects, speedMax 4,
brightnessMax 4** — so brightness and speed are both 0..4, not 0..255.

Note the list is unordered with respect to effect id, and it omits Snake (0x05),
MusicalRhythm (0x0A) and Custom (0x0D).

## 6. Set lighting (0x07)

The payload is the vendor app's 14-field `LightInfo` struct flattened to bytes.
`length` is **always `0x0E`** regardless of how many bytes are populated — the
firmware reads the whole 14-byte window.

```
offset  field            range
  6     ledType          effect id, see below
  7     ledBrightness    0..4
  8     ledSpeed         0..4
  9     ledModeR         primary colour
 10     ledModeG
 11     ledModeB
 12     ledBgR           secondary colour (two-colour effects)
 13     ledBgG
 14     ledBgB
 15     ledDirection
 16     ledModeType      1 = let the firmware cycle colours
 17     ledOpenOrClose   0 = lighting ON, 1 = blank  (inverted!)
 18-19  (zero)
```

⚠ Byte 17 is **inverted** relative to what its name suggests. Sending `1`
switches the backlight off completely and it stays off until a frame with `0`
arrives — confirmed the hard way on live hardware.

Two firmware quirks the vendor app reproduces, and so does this driver:

* When `ledModeType == 1`, the colour block becomes the marker `FF 00 00 00 00 00`.
* Effect `Static` forces `ledModeType = 0` — static lighting never cycles.
* Effects `{Snake, Radar, Aurora, Ripple, SpeedRespond, Cross, Reactive}` always
  use the explicit colour and ignore `ledModeType`.

### Effect ids

The id is an index into the app's fixed effect table. The decisive confirmation
is that the app special-cases `ledType == 0x0A` to start its audio spectrum
analyser, and index 10 of that table is *Musical Rhythm*.

| id | effect | | id | effect |
|---|---|---|---|---|
| 0x00 | Static | | 0x09 | SpeedRespond |
| 0x01 | Neon | | 0x0A | MusicalRhythm |
| 0x02 | Breath | | 0x0B | Cross |
| 0x03 | Wave | | 0x0C | Reactive |
| 0x04 | Twinkle | | 0x0D | Custom |
| 0x05 | Snake | | 0x0E | AutoRipple |
| 0x06 | Radar | | 0x0F | Striation |
| 0x07 | Aurora | | 0x10 | Fireworks |
| 0x08 | Ripple | | | |

## 7. Per-key colours (0x0F and 0x09)

LEDs form a fixed **6 × 22 matrix**, row major:

```
ledIndex = ledRow * 22 + ledCol        0 .. 131
```

A full frame is `132 × 3 = 396` bytes of `R,G,B` per cell, streamed as:

```
byte 2 = zone    0 = main key matrix, 1 = secondary zone (33 cells)
byte 4 = page    0,1,2,... incrementing per packet
byte 5 = length  54 for packets 0..6, 18 for packet 7
byte 6.. = colour bytes
```

54 bytes is 18 cells, the largest multiple of 3 fitting the 58-byte payload
window, so a frame is **7 packets of 54 bytes plus a final 18-byte packet**.
Cells with no key under them are still part of the stream; send them black.

`0x0F` applies the frame immediately. `0x09` has the identical payload shape but
writes into onboard custom slot `byte 2` instead.

**Verified on hardware 2026-10-17:** `0x0F` takes effect on its own, with *no*
`0x07` effect change beforehand. Selecting effect `Custom` (`0x0D`) is **not**
required -- which matters, because this board does not advertise `0x0D` at all.
Earlier notes here suggested the Custom select was necessary; it is not.

Streaming is fast enough to treat the board as a framebuffer. Measured over 60
consecutive frames with **no inter-packet delay**: **8.02 ms per frame**, i.e. a
ceiling around **125 fps**. That is the USB floor for 8 reports at a 1 ms
interval, so the 2 ms `settle()` the C++ driver sleeps between packets is pure
overhead -- it costs roughly 44% of a 30 fps frame budget and buys nothing.

Bear in mind this is a composite device: the same full-speed pipe carries the
actual keystroke reports, so 30 fps (a quarter of the budget) is a saner default
than hammering it at the ceiling.

### MAXFIT61 key positions

Row 0 is the function-key row, which a 61-key board does not have, so all 61
keys sit in rows 1..5 and LED indices run 22..123. The mapping in
`Driver/src/KeyMap.cpp` is generated from `KeyInfo_XS61BRGB.config` (its grid
row/column, where `ledRow = configRow − 1`) and cross-checked against
`customLightDefaultConfig_61.xml`, whose `indexX`/`indexY` attributes are
exactly this `ledRow`/`ledCol` pair. Regenerate with `docs/gen_keymap.py`.

```
row 1  Esc 1 2 3 4 5 6 7 8 9 0 Minus Equal  .  Backspace     (cols 0-12, 14)
row 2  Tab Q W E R T Y U I O P [ ]     .  Backslash          (cols 0-12, 14)
row 3  Caps  .  A S D F G H J K L ; '   .  Enter             (cols 0, 2-12, 14)
row 4  LShift . Z X C V B N M , . /     .  RShift            (cols 0, 2-11, 14)
row 5  LCtrl LWin LAlt . . . Space . . . RAlt Fn Menu RCtrl  (cols 0-2, 6, 10-13)
```

Note `Fn` reports HID usage `-10` in the vendor config (stored as `0xFF` here):
it is handled entirely in firmware and is not remappable, though it does have an
LED.

## 8. Things still unmapped

* `ledDirection` accepts a value but the per-effect meaning was not enumerated.
* Byte 2 (`zone`) of `0x07` is always 0 for the keyboard; the side-light path
  (`0x08`) uses a second zone with 33 cells, unused on this model
  (`sideLightHave: 0`).
* Payload bytes 18–19 of `0x07` are always zero.
* The key-remap (`0x04`/`0x05`), macro and onboard-profile commands are
  identified but not decoded; the `XS61BRGB_Def{QWERTY,Dvorak,Mac}.xml`
  resources describe their data model if you want to go further.

## 9. Key remapping and macros — partial

Status: **the transport is decoded, the record encoding is not.** Nothing here
has been written to hardware. Recovered 2026-10-17 by disassembling the
installed `C:\Program Files (x86)\FANTECH MK857 Gaming Keyboard\FANTECH.exe`
(same binary as the installer payload, so no re-extraction needed).

### What `0x04` / `0x05` actually are

Not a 61-key table. `[Write Six Key]` (`0x04`, builder at `0x00649e80`) and
`[Write Full Key]` (`0x05`, builder at `0x00649fc0`) each send **one** report:

```
01 04|05 00 00 00 15 <13 bytes copied from the caller's buffer at [6..18]>
```

`length` is `0x15` (21) although only 13 bytes are populated — the same
declare-a-fixed-window habit as `0x07`. The single caller of the `0x05` builder
sits inside a reply dispatcher that switches on the *incoming* `buf[1]`, so
these look like rollover / per-binding messages rather than a bulk table.

### `0x18` — write onboard data (the remap transport)

Builder at `0x006f9f10`, two call sites inside `0x00714e80`.

```
offset  field
  2     data type / zone   caller argument
  4     page               incremented per packet
  5     length             size of the appended byte list
  6..   payload
```

The caller builds a byte list and **flushes every 14 records**, appending
**4 bytes per record**. 61 keys x 4 bytes = 244 bytes, which is 5 packets of 14
records — exactly the shape of a per-key binding table. The 4-byte record very
likely corresponds to the vendor profile XML's `operateType` + `Value` pair
(`<item keyCode keyChar operateType repeatNum mode Value/>`).

### `0x19` — onboard download (full profile push)

Builder at `0x006faec8` inside `0x006fa5a0`.

```
01 19 <profile> 00 <page> 3A <58 payload bytes>
```

Page 0 carries a header; later pages are bulk data. Despite the log string
`Onboard Download [Write 0x19 Recv]` (`0x0070db20`), this is the *host writing
to the keyboard*; the "Recv" appears to be the acknowledgement read, not a data
dump. **No read-back path has been confirmed**, which is why nothing has been
written yet.

### Still needed

1. The 4-byte record encoding, and the `0x18` type-byte values.
2. A way to read the current bindings back, or a verified restore path. The
   vendor's `onboardDeviceConfig/XS61BRGB_Def{QWERTY,Dvorak,Mac}.xml` describes
   the data model but ships with `<onboardBinding />` and `<bindMacro />`
   **empty**, so it cannot be used to infer the layout.
3. The quickest route is differential: capture `0x18` traffic with USBPcap while
   the vendor app remaps one key, then diff. That ends the guesswork in one
   session, versus an open-ended static grind.

⚠ Do not write `0x18` or `0x19` speculatively. A wrong binding table leaves a
keyboard that types the wrong characters, and unlike a lighting mistake there is
no known one-command fix.
