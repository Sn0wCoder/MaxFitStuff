//! The MAXFIT61 vendor HID wire format.
//!
//! Recovered by reverse engineering FANTECH.exe v1.3 (Qt5 + hidapi) and verified
//! against live hardware. See `FanTechMaxFitCollection/Docs/PROTOCOL.md` for the
//! full derivation; this module is a faithful port of the C++ `Driver`.
//!
//! Every exchange is a fixed 64-byte HID report on the vendor interface
//! (usage page 0xFF1B, usage 0x91, interface MI_02):
//!
//! ```text
//! [0] report id   always 0x01
//! [1] command     echoed back in the reply
//! [2] zone        command specific (LED zone / custom-profile slot)
//! [3] reserved    always 0
//! [4] page        packet index for multi-packet payloads
//! [5] length      number of meaningful bytes starting at [6]
//! [6..63]         payload (58 bytes max)
//! ```

use serde::{Deserialize, Serialize};

use crate::color::Rgb;

pub const VENDOR_ID: u16 = 0x0416; // Nuvoton
pub const PRODUCT_ID: u16 = 0xC345; // MAXFIT61 Mechanical Keyboard
pub const USAGE_PAGE: u16 = 0xFF1B; // vendor defined
pub const USAGE: u16 = 0x0091;

pub const REPORT_SIZE: usize = 64;
pub const PAYLOAD_OFFSET: usize = 6;
pub const MAX_PAYLOAD: usize = REPORT_SIZE - PAYLOAD_OFFSET; // 58
pub const REPORT_ID: u8 = 0x01;

pub type Report = [u8; REPORT_SIZE];

/// Firmware commands this driver is willing to send.
///
/// Commands `0x0B` (enter bootloader) and `0x0C` (flash firmware block) have no
/// variant **by design** -- getting them wrong bricks the keyboard, and the
/// vendor's own updater already handles firmware. Because [`make_request`] takes
/// a `Command` and never a raw `u8`, emitting one would require editing this
/// enum. The `no_encoder_can_emit_a_flash_command` test pins that down.
#[non_exhaustive]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[repr(u8)]
pub enum Command {
    Handshake = 0x01,
    QuerySideLightModes = 0x02,
    WriteSixKeyMap = 0x04,
    WriteFullKeyMap = 0x05,
    WriteMultimedia = 0x06,
    /// Effect + colours + brightness + speed.
    SetLighting = 0x07,
    SetSideLighting = 0x08,
    /// Per-key colours into a stored "Custom N" slot.
    WriteCustomLedProfile = 0x09,
    /// Supported effects + speed/brightness maxima.
    QueryLightModes = 0x0A,
    /// Model name + firmware version, then build date.
    QueryDeviceInfo = 0x0D,
    /// Per-key colours applied immediately.
    SetPerKeyColors = 0x0F,
    QueryDeviceSerial = 0x15,
    /// "Enter light synchronisation" -- host takes over the LEDs. Not needed on
    /// this unit (0x0F applies live on its own) but identified in the firmware.
    EnterLightSync = 0x17,
}

/// Rejects anything not in [`Command`], which includes the two flash commands.
impl TryFrom<u8> for Command {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        Ok(match v {
            0x01 => Command::Handshake,
            0x02 => Command::QuerySideLightModes,
            0x04 => Command::WriteSixKeyMap,
            0x05 => Command::WriteFullKeyMap,
            0x06 => Command::WriteMultimedia,
            0x07 => Command::SetLighting,
            0x08 => Command::SetSideLighting,
            0x09 => Command::WriteCustomLedProfile,
            0x0A => Command::QueryLightModes,
            0x0D => Command::QueryDeviceInfo,
            0x0F => Command::SetPerKeyColors,
            0x15 => Command::QueryDeviceSerial,
            0x17 => Command::EnterLightSync,
            _ => return Err(()),
        })
    }
}

/// Firmware lighting effects. The value travels in payload byte 0 of
/// [`Command::SetLighting`]. Ordering is the vendor software's fixed effect
/// table; id `0x0A` is special-cased there to drive its PC-side audio analyser,
/// which is what pins this mapping down.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Effect {
    Static = 0x00,
    Neon = 0x01,
    Breath = 0x02,
    Wave = 0x03,
    Twinkle = 0x04,
    Snake = 0x05,
    Radar = 0x06,
    Aurora = 0x07,
    Ripple = 0x08,
    SpeedRespond = 0x09,
    MusicalRhythm = 0x0A,
    Cross = 0x0B,
    Reactive = 0x0C,
    Custom = 0x0D,
    AutoRipple = 0x0E,
    Striation = 0x0F,
    Fireworks = 0x10,
}

/// Index == effect id; the vendor software keeps this exact order.
pub const EFFECTS: &[(Effect, &str)] = &[
    (Effect::Static, "Static"),
    (Effect::Neon, "Neon"),
    (Effect::Breath, "Breath"),
    (Effect::Wave, "Wave"),
    (Effect::Twinkle, "Twinkle"),
    (Effect::Snake, "Snake"),
    (Effect::Radar, "Radar"),
    (Effect::Aurora, "Aurora"),
    (Effect::Ripple, "Ripple"),
    (Effect::SpeedRespond, "SpeedRespond"),
    (Effect::MusicalRhythm, "MusicalRhythm"),
    (Effect::Cross, "Cross"),
    (Effect::Reactive, "Reactive"),
    (Effect::Custom, "Custom"),
    (Effect::AutoRipple, "AutoRipple"),
    (Effect::Striation, "Striation"),
    (Effect::Fireworks, "Fireworks"),
];

impl Effect {
    pub fn name(self) -> &'static str {
        EFFECTS.get(self as usize).map_or("Unknown", |(_, n)| *n)
    }

    /// Case-insensitive lookup by name.
    pub fn from_name(name: &str) -> Option<Effect> {
        let want = name.trim().to_ascii_lowercase();
        EFFECTS
            .iter()
            .find(|(_, n)| n.to_ascii_lowercase() == want)
            .map(|(e, _)| *e)
    }

    pub fn all() -> impl Iterator<Item = Effect> {
        EFFECTS.iter().map(|(e, _)| *e)
    }

    /// These effects always use the explicit colour and ignore `random_color`.
    pub fn ignores_random_color(self) -> bool {
        matches!(
            self,
            Effect::Snake
                | Effect::Radar
                | Effect::Aurora
                | Effect::Ripple
                | Effect::SpeedRespond
                | Effect::Cross
                | Effect::Reactive
        )
    }
}

impl TryFrom<u8> for Effect {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        EFFECTS.get(v as usize).map(|(e, _)| *e).ok_or(())
    }
}

/// Payload of [`Command::SetLighting`]. Field order mirrors the vendor software's
/// 14-field `LightInfo` struct, whose names come straight out of its profile XML
/// (`ledType`, `ledBrightness`, `ledSpeed`, `ledModeR/G/B`, `ledBgR/G/B`,
/// `ledDirection`, `ledModeType`, `ledOpenOrClose`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct LightingState {
    pub effect: Effect,
    /// 0..=brightness_max (4 on this model).
    pub brightness: u8,
    /// 0..=speed_max (4 on this model).
    pub speed: u8,
    /// Foreground / primary colour.
    pub color: Rgb,
    /// Secondary, used by two-colour effects.
    pub background: Rgb,
    pub direction: u8,
    /// `ledModeType`: let the firmware cycle colours instead of using `color`.
    pub random_color: bool,
    /// `ledOpenOrClose`. The wire encoding is **inverted**: the firmware wants 0
    /// for "lit" and 1 for "blank". [`encode_set_lighting`] handles that, and
    /// this defaults to `true` so a round-trip through serde can never blank the
    /// board by omission.
    #[serde(default = "enabled_default")]
    pub enabled: bool,
}

fn enabled_default() -> bool {
    true
}

impl Default for LightingState {
    fn default() -> Self {
        Self {
            effect: Effect::Static,
            brightness: 4,
            speed: 2,
            color: Rgb::WHITE,
            background: Rgb::BLACK,
            direction: 0,
            random_color: false,
            enabled: true,
        }
    }
}

impl LightingState {
    /// The known-good "put the lights back on" state. Wired to the panic button,
    /// the tray menu and the CLI, because a blanked board looks like dead
    /// hardware and this is the only thing that fixes it.
    pub fn panic() -> Self {
        Self::default()
    }
}

/// What the keyboard reports for [`Command::QueryLightModes`].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct LightCapabilities {
    pub effects: Vec<u8>,
    pub speed_max: u8,
    pub brightness_max: u8,
}

impl LightCapabilities {
    pub fn supports(&self, e: Effect) -> bool {
        self.effects.contains(&(e as u8))
    }

    /// Only the ids that map to an effect we know a name for.
    pub fn known_effects(&self) -> Vec<Effect> {
        self.effects
            .iter()
            .filter_map(|b| Effect::try_from(*b).ok())
            .collect()
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DeviceInfo {
    /// e.g. "M252,01,KB,TY,XS61BRGB,V1.03.07"
    pub raw: String,
    pub model: String,
    pub firmware: String,
    pub build_date: String,
    pub serial: String,
}

// ---- framing -------------------------------------------------------------

/// Builds a request. `payload` is clamped to the 58-byte window.
pub fn make_request(cmd: Command, zone: u8, page: u8, payload: &[u8]) -> Report {
    let n = payload.len().min(MAX_PAYLOAD);
    let mut r = [0u8; REPORT_SIZE];
    r[0] = REPORT_ID;
    r[1] = cmd as u8;
    r[2] = zone;
    r[3] = 0;
    r[4] = page;
    r[5] = n as u8;
    r[PAYLOAD_OFFSET..PAYLOAD_OFFSET + n].copy_from_slice(&payload[..n]);
    r
}

/// True when `r` is a well-formed reply to `cmd`.
pub fn is_reply_to(r: &Report, cmd: Command) -> bool {
    r[0] == REPORT_ID && r[1] == cmd as u8
}

pub fn reply_page(r: &Report) -> u8 {
    r[4]
}

pub fn reply_len(r: &Report) -> usize {
    (r[5] as usize).min(MAX_PAYLOAD)
}

/// Payload of a reply as text, trimmed at the first NUL.
pub fn reply_text(r: &Report) -> String {
    let len = reply_len(r);
    let bytes = &r[PAYLOAD_OFFSET..PAYLOAD_OFFSET + len];
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(len);
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

// ---- command encoders ---------------------------------------------------

/// Encodes [`Command::SetLighting`].
///
/// The `length` field is **always** `0x0E` regardless of how many bytes are
/// populated -- the firmware reads the whole 14-byte window.
pub fn encode_set_lighting(s: &LightingState) -> Report {
    let mut p = [0u8; 14];
    p[0] = s.effect as u8;
    p[1] = s.brightness;
    p[2] = s.speed;
    p[3] = s.color.r;
    p[4] = s.color.g;
    p[5] = s.color.b;
    p[6] = s.background.r;
    p[7] = s.background.g;
    p[8] = s.background.b;
    p[9] = s.direction;
    p[10] = u8::from(s.random_color);
    // Inverted on the wire: 0 lights the board, 1 blanks it. Confirmed on
    // hardware -- sending 1 here switches the backlight off entirely, and it
    // stays off until a frame with this byte clear arrives.
    p[11] = u8::from(!s.enabled);
    // p[12], p[13] stay zero.

    // Static never cycles colours, and when random_color is set the firmware
    // wants the marker 0xFF in the primary-red slot with the rest cleared --
    // both behaviours are copied from the vendor software.
    if s.effect == Effect::Static {
        p[10] = 0;
    } else if p[10] == 1 {
        p[3] = 0xFF;
        p[4] = 0;
        p[5] = 0;
        p[6] = 0;
        p[7] = 0;
        p[8] = 0;
    }

    let mut r = make_request(Command::SetLighting, 0, 0, &p);
    r[5] = 0x0E; // always 14, even though not all bytes are populated
    r
}

/// Parses a [`Command::QueryLightModes`] reply.
///
/// Layout is `<effect ids...> 0xA5 0x5A <speed_max> <brightness_max>`; the rule
/// is copied from the vendor app, which appends bytes until it sees either
/// marker value.
pub fn parse_light_capabilities(r: &Report) -> LightCapabilities {
    let mut caps = LightCapabilities::default();
    let len = reply_len(r);
    let mut saw_marker = false;
    for i in 0..len {
        let b = r[PAYLOAD_OFFSET + i];
        if b == 0xA5 || b == 0x5A {
            saw_marker = true;
            continue;
        }
        if saw_marker {
            caps.speed_max = b;
            if i + 1 < len {
                caps.brightness_max = r[PAYLOAD_OFFSET + i + 1];
            }
            break;
        }
        caps.effects.push(b);
    }
    caps
}

/// Parses the page-0 device-info string into model and firmware.
///
/// The firmware field is the one starting with `V` and containing a dot; the
/// model is the field just before it.
pub fn parse_device_info(raw: &str) -> DeviceInfo {
    let mut out = DeviceInfo {
        raw: raw.to_string(),
        ..Default::default()
    };
    let fields: Vec<&str> = raw.split(',').collect();

    for (i, f) in fields.iter().enumerate() {
        if f.starts_with(['V', 'v']) && f.contains('.') {
            out.firmware = (*f).to_string();
            if i > 0 {
                out.model = fields[i - 1].to_string();
            }
            break;
        }
    }
    if out.model.is_empty() && fields.len() >= 2 {
        out.model = fields[fields.len() - 2].to_string();
    }
    out
}
