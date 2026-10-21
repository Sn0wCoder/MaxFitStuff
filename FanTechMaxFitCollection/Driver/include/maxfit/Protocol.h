// Protocol.h -- wire format for the Fantech MAXFIT61 (MK857 / XS61BRGB) vendor HID interface.
//
// Recovered by reverse engineering FANTECH.exe v1.3 (Qt5 + hidapi) and verified
// against live hardware. See docs/PROTOCOL.md for the full derivation.
//
// Every exchange is a fixed 64-byte HID report on the vendor interface
// (usage page 0xFF1B, usage 0x91, interface MI_02):
//
//   [0] report id   always 0x01
//   [1] command     see Command; echoed back in the reply
//   [2] zone        command specific (LED zone / custom-profile slot)
//   [3] reserved    always 0
//   [4] page        packet index for multi-packet payloads
//   [5] length      number of meaningful bytes starting at [6]
//   [6..63]         payload (58 bytes max)
#pragma once

#include <array>
#include <cstdint>
#include <string>
#include <vector>

namespace maxfit {

constexpr std::uint16_t kVendorId = 0x0416;   // Nuvoton
constexpr std::uint16_t kProductId = 0xC345;  // MAXFIT61 Mechanical Keyboard
constexpr std::uint16_t kUsagePage = 0xFF1B;  // vendor defined
constexpr std::uint16_t kUsage = 0x0091;

constexpr std::size_t kReportSize = 64;
constexpr std::size_t kPayloadOffset = 6;
constexpr std::size_t kMaxPayload = kReportSize - kPayloadOffset;  // 58
constexpr std::uint8_t kReportId = 0x01;

using Report = std::array<std::uint8_t, kReportSize>;

// Commands implemented by the firmware. Only the ones this driver uses are
// named; the full set recovered from the vendor software is in docs/PROTOCOL.md.
//
// DANGER: commands 0x0B (enter bootloader) and 0x0C (flash firmware block) are
// deliberately NOT exposed here. Issuing them incorrectly bricks the keyboard.
enum class Command : std::uint8_t {
    Handshake = 0x01,
    QuerySideLightModes = 0x02,
    WriteSixKeyMap = 0x04,
    WriteFullKeyMap = 0x05,
    WriteMultimedia = 0x06,
    SetLighting = 0x07,  // effect + colours + brightness + speed
    SetSideLighting = 0x08,
    WriteCustomLedProfile = 0x09,  // per-key colours into a stored "Custom N" slot
    QueryLightModes = 0x0A,        // supported effects + speed/brightness maxima
    QueryDeviceInfo = 0x0D,        // model name + firmware version, then build date
    SetPerKeyColors = 0x0F,        // per-key colours applied immediately
    QueryDeviceSerial = 0x15,
};

// Lighting effect ids. The value is what travels in payload byte 0 of
// Command::SetLighting. Ordering is the vendor software's fixed effect table;
// id 0x0A is special-cased there to drive the PC-side audio analyser, which is
// what pins this mapping down.
enum class Effect : std::uint8_t {
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
};

const char* effectName(Effect e);
// Case-insensitive lookup; returns false if the name is unknown.
bool effectFromName(const std::string& name, Effect& out);
std::vector<Effect> allEffects();

struct Rgb {
    std::uint8_t r = 0, g = 0, b = 0;
};

// Payload of Command::SetLighting. Field order mirrors the vendor software's
// 14-int LightInfo struct, whose names come straight out of its profile XML
// (ledType, ledBrightness, ledSpeed, ledModeR/G/B, ledBgR/G/B, ledDirection,
// ledModeType, ledOpenOrClose).
struct LightingState {
    Effect effect = Effect::Static;
    std::uint8_t brightness = 4;  // 0..brightnessMax (4 on this model)
    std::uint8_t speed = 2;       // 0..speedMax      (4 on this model)
    Rgb color;                    // foreground / primary
    Rgb background;               // secondary, used by two-colour effects
    std::uint8_t direction = 0;
    bool randomColor = false;  // ledModeType: cycle colours instead of using `color`
    // ledOpenOrClose. Note the wire encoding is inverted: the firmware wants 0
    // for "lit" and 1 for "blank". encodeSetLighting() handles that.
    bool enabled = true;
};

// What the keyboard reports for Command::QueryLightModes.
struct LightCapabilities {
    std::vector<Effect> effects;
    std::uint8_t speedMax = 0;
    std::uint8_t brightnessMax = 0;
};

struct DeviceInfo {
    std::string raw;       // e.g. "M252,01,KB,TY,XS61BRGB,V1.03.07"
    std::string model;     // "XS61BRGB"
    std::string firmware;  // "V1.03.07"
    std::string buildDate; // "Feb 24 2022,15:23:58"
    std::string serial;
};

// ---- framing -------------------------------------------------------------

// Build a request. `payload` may be empty; length is written to byte 5.
Report makeRequest(Command cmd,
                   std::uint8_t zone = 0,
                   std::uint8_t page = 0,
                   const std::uint8_t* payload = nullptr,
                   std::size_t payloadLen = 0);

inline Report makeRequest(Command cmd,
                          std::uint8_t zone,
                          std::uint8_t page,
                          const std::vector<std::uint8_t>& payload) {
    return makeRequest(cmd, zone, page, payload.data(), payload.size());
}

// True when `r` is a well-formed reply to `cmd`.
bool isReplyTo(const Report& r, Command cmd);

inline std::uint8_t replyPage(const Report& r) { return r[4]; }
inline std::uint8_t replyLength(const Report& r) {
    return r[5] <= kMaxPayload ? r[5] : static_cast<std::uint8_t>(kMaxPayload);
}
// Payload of a reply as text, trimmed at the first NUL.
std::string replyText(const Report& r);

// ---- command encoders ---------------------------------------------------

Report encodeSetLighting(const LightingState& s);

// Parse a Command::QueryLightModes reply. Layout is
// <effect ids...> 0xA5 0x5A <speedMax> <brightnessMax>.
LightCapabilities parseLightCapabilities(const Report& r);

// Parse the "M252,01,KB,TY,XS61BRGB,V1.03.07" page-0 reply into model/firmware.
void parseDeviceInfo(const std::string& raw, DeviceInfo& out);

}  // namespace maxfit
