#include "maxfit/Protocol.h"

#include <algorithm>
#include <cctype>
#include <cstring>

namespace maxfit {
namespace {

struct EffectEntry {
    Effect id;
    const char* name;
};

// Index == effect id; the vendor software keeps this exact order.
const EffectEntry kEffects[] = {
    {Effect::Static, "Static"},
    {Effect::Neon, "Neon"},
    {Effect::Breath, "Breath"},
    {Effect::Wave, "Wave"},
    {Effect::Twinkle, "Twinkle"},
    {Effect::Snake, "Snake"},
    {Effect::Radar, "Radar"},
    {Effect::Aurora, "Aurora"},
    {Effect::Ripple, "Ripple"},
    {Effect::SpeedRespond, "SpeedRespond"},
    {Effect::MusicalRhythm, "MusicalRhythm"},
    {Effect::Cross, "Cross"},
    {Effect::Reactive, "Reactive"},
    {Effect::Custom, "Custom"},
    {Effect::AutoRipple, "AutoRipple"},
    {Effect::Striation, "Striation"},
    {Effect::Fireworks, "Fireworks"},
};

std::string lower(const std::string& s) {
    std::string out = s;
    std::transform(out.begin(), out.end(), out.begin(),
                   [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
    return out;
}

}  // namespace

const char* effectName(Effect e) {
    const auto idx = static_cast<std::size_t>(e);
    if (idx < std::size(kEffects)) return kEffects[idx].name;
    return "Unknown";
}

bool effectFromName(const std::string& name, Effect& out) {
    const std::string want = lower(name);
    for (const auto& entry : kEffects) {
        if (lower(entry.name) == want) {
            out = entry.id;
            return true;
        }
    }
    return false;
}

std::vector<Effect> allEffects() {
    std::vector<Effect> out;
    out.reserve(std::size(kEffects));
    for (const auto& e : kEffects) out.push_back(e.id);
    return out;
}

Report makeRequest(Command cmd,
                   std::uint8_t zone,
                   std::uint8_t page,
                   const std::uint8_t* payload,
                   std::size_t payloadLen) {
    if (payloadLen > kMaxPayload) payloadLen = kMaxPayload;
    Report r{};
    r.fill(0);
    r[0] = kReportId;
    r[1] = static_cast<std::uint8_t>(cmd);
    r[2] = zone;
    r[3] = 0;
    r[4] = page;
    r[5] = static_cast<std::uint8_t>(payloadLen);
    if (payload != nullptr && payloadLen > 0) {
        std::memcpy(r.data() + kPayloadOffset, payload, payloadLen);
    }
    return r;
}

bool isReplyTo(const Report& r, Command cmd) {
    return r[0] == kReportId && r[1] == static_cast<std::uint8_t>(cmd);
}

std::string replyText(const Report& r) {
    const std::size_t len = replyLength(r);
    std::string out;
    out.reserve(len);
    for (std::size_t i = 0; i < len; ++i) {
        const char c = static_cast<char>(r[kPayloadOffset + i]);
        if (c == '\0') break;
        out.push_back(c);
    }
    return out;
}

Report encodeSetLighting(const LightingState& s) {
    // Fixed 14-byte payload; the firmware reads bytes [6..19] regardless of how
    // many the host actually fills in.
    std::uint8_t p[14] = {};
    p[0] = static_cast<std::uint8_t>(s.effect);
    p[1] = s.brightness;
    p[2] = s.speed;
    p[3] = s.color.r;
    p[4] = s.color.g;
    p[5] = s.color.b;
    p[6] = s.background.r;
    p[7] = s.background.g;
    p[8] = s.background.b;
    p[9] = s.direction;
    p[10] = s.randomColor ? 1 : 0;
    // Inverted on the wire: 0 lights the board, 1 blanks it. Confirmed on
    // hardware -- sending 1 here switches the backlight off entirely.
    p[11] = s.enabled ? 0 : 1;
    // p[12], p[13] stay zero.

    // Static never cycles colours, and when randomColor is set the firmware
    // wants the marker 0xFF in the primary-red slot with the rest cleared --
    // both behaviours are copied from the vendor software.
    if (s.effect == Effect::Static) {
        p[10] = 0;
    } else if (p[10] == 1) {
        p[3] = 0xFF;
        p[4] = p[5] = p[6] = p[7] = p[8] = 0;
    }

    Report r = makeRequest(Command::SetLighting, 0, 0, p, sizeof(p));
    r[5] = 0x0E;  // always 14, even though not all bytes are populated
    return r;
}

LightCapabilities parseLightCapabilities(const Report& r) {
    LightCapabilities caps;
    const std::size_t len = replyLength(r);
    bool sawMarker = false;
    for (std::size_t i = 0; i < len; ++i) {
        const std::uint8_t b = r[kPayloadOffset + i];
        if (b == 0xA5 || b == 0x5A) {
            sawMarker = true;
            continue;
        }
        if (sawMarker) {
            caps.speedMax = b;
            if (i + 1 < len) caps.brightnessMax = r[kPayloadOffset + i + 1];
            break;
        }
        caps.effects.push_back(static_cast<Effect>(b));
    }
    return caps;
}

void parseDeviceInfo(const std::string& raw, DeviceInfo& out) {
    out.raw = raw;
    // "M252,01,KB,TY,XS61BRGB,V1.03.07" -- model is the field before the
    // version, and the version is the field starting with 'V'.
    std::vector<std::string> fields;
    std::string cur;
    for (const char c : raw) {
        if (c == ',') {
            fields.push_back(cur);
            cur.clear();
        } else {
            cur.push_back(c);
        }
    }
    if (!cur.empty()) fields.push_back(cur);

    for (std::size_t i = 0; i < fields.size(); ++i) {
        if (!fields[i].empty() && (fields[i][0] == 'V' || fields[i][0] == 'v') &&
            fields[i].find('.') != std::string::npos) {
            out.firmware = fields[i];
            if (i > 0) out.model = fields[i - 1];
            break;
        }
    }
    if (out.model.empty() && fields.size() >= 2) {
        out.model = fields[fields.size() - 2];
    }
}

}  // namespace maxfit
