// Protocol tests. These run without a keyboard attached: they pin the wire
// format and the LED addressing against what was recovered from the vendor
// software, so a refactor cannot silently change the bytes we send.
#include <cstdio>
#include <iostream>
#include <string>
#include <vector>

#include "maxfit/Keyboard.h"

namespace {

int g_failures = 0;
int g_checks = 0;

void check(bool ok, const std::string& what) {
    ++g_checks;
    if (!ok) {
        ++g_failures;
        std::cout << "  FAIL  " << what << "\n";
    }
}

template <typename A, typename B>
void checkEq(const A& actual, const B& expected, const std::string& what) {
    ++g_checks;
    if (!(actual == static_cast<A>(expected))) {
        ++g_failures;
        std::cout << "  FAIL  " << what << ": got " << +actual << ", expected " << +expected
                  << "\n";
    }
}

using namespace maxfit;

void testFraming() {
    std::cout << "framing\n";
    const std::uint8_t payload[] = {0xAA, 0xBB, 0xCC};
    const Report r = makeRequest(Command::QueryDeviceInfo, 2, 5, payload, sizeof(payload));

    checkEq(r.size(), std::size_t{64}, "report is 64 bytes");
    checkEq(r[0], 0x01, "byte 0 is the report id");
    checkEq(r[1], 0x0D, "byte 1 is the command");
    checkEq(r[2], 2, "byte 2 is the zone");
    checkEq(r[3], 0, "byte 3 is reserved");
    checkEq(r[4], 5, "byte 4 is the page");
    checkEq(r[5], 3, "byte 5 is the payload length");
    checkEq(r[6], 0xAA, "payload starts at byte 6");
    checkEq(r[8], 0xCC, "payload is contiguous");
    checkEq(r[9], 0, "tail is zero filled");

    check(isReplyTo(r, Command::QueryDeviceInfo), "isReplyTo matches the command");
    check(!isReplyTo(r, Command::SetLighting), "isReplyTo rejects another command");

    // A payload longer than the 58-byte window must be clamped, not overflow.
    std::vector<std::uint8_t> big(200, 0x7F);
    const Report clamped = makeRequest(Command::SetPerKeyColors, 0, 0, big);
    checkEq(clamped[5], 58, "oversized payload is clamped to 58");
    checkEq(clamped[63], 0x7F, "clamped payload fills to the last byte");
}

void testSetLighting() {
    std::cout << "set lighting (0x07)\n";
    LightingState s;
    s.effect = Effect::Breath;
    s.brightness = 3;
    s.speed = 1;
    s.color = {0x11, 0x22, 0x33};
    s.background = {0x44, 0x55, 0x66};
    s.direction = 1;
    s.randomColor = false;
    s.enabled = true;

    const Report r = encodeSetLighting(s);
    checkEq(r[1], 0x07, "command is 0x07");
    checkEq(r[5], 0x0E, "length field is always 14");
    checkEq(r[6], 0x02, "effect id in payload byte 0");
    checkEq(r[7], 3, "brightness in payload byte 1");
    checkEq(r[8], 1, "speed in payload byte 2");
    checkEq(r[9], 0x11, "primary R");
    checkEq(r[10], 0x22, "primary G");
    checkEq(r[11], 0x33, "primary B");
    checkEq(r[12], 0x44, "background R");
    checkEq(r[13], 0x55, "background G");
    checkEq(r[14], 0x66, "background B");
    checkEq(r[15], 1, "direction");
    checkEq(r[16], 0, "ledModeType clear when not cycling");
    // Inverted on the wire, verified on hardware: 0 lights up, 1 blanks.
    checkEq(r[17], 0, "ledOpenOrClose is 0 when enabled");

    // randomColor replaces the colour block with the 0xFF marker.
    s.randomColor = true;
    const Report rnd = encodeSetLighting(s);
    checkEq(rnd[16], 1, "ledModeType set when cycling");
    checkEq(rnd[9], 0xFF, "cycling writes the 0xFF marker");
    checkEq(rnd[10], 0, "cycling clears the rest of the colour block");
    checkEq(rnd[14], 0, "cycling clears the background too");

    // Static is forced out of colour-cycling mode by the firmware's own rule.
    s.effect = Effect::Static;
    s.randomColor = true;
    s.color = {1, 2, 3};
    const Report st = encodeSetLighting(s);
    checkEq(st[16], 0, "Static never cycles colours");
    checkEq(st[9], 1, "Static keeps its explicit colour");

    s.enabled = false;
    checkEq(encodeSetLighting(s)[17], 1, "ledOpenOrClose is 1 when disabled");
}

void testCapabilityParsing() {
    std::cout << "capability parsing (0x0A)\n";
    // Exactly what the keyboard replied on the wire.
    const std::uint8_t payload[] = {0x02, 0x01, 0x03, 0x09, 0x04, 0x00, 0x0E, 0x0F, 0x06,
                                    0x07, 0x0B, 0x0C, 0x10, 0x08, 0xA5, 0x5A, 0x04, 0x04};
    Report r = makeRequest(Command::QueryLightModes, 0, 0, payload, sizeof(payload));

    const LightCapabilities caps = parseLightCapabilities(r);
    checkEq(caps.effects.size(), std::size_t{14}, "14 effects reported");
    checkEq(caps.speedMax, 4, "speedMax is 4");
    checkEq(caps.brightnessMax, 4, "brightnessMax is 4");
    check(caps.effects.front() == Effect::Breath, "first reported effect is Breath (0x02)");
    check(caps.effects.back() == Effect::Ripple, "last reported effect is Ripple (0x08)");
}

void testDeviceInfoParsing() {
    std::cout << "device info parsing (0x0D)\n";
    DeviceInfo info;
    parseDeviceInfo("M252,01,KB,TY,XS61BRGB,V1.03.07", info);
    check(info.model == "XS61BRGB", "model is XS61BRGB");
    check(info.firmware == "V1.03.07", "firmware is V1.03.07");
}

void testEffectNames() {
    std::cout << "effect names\n";
    Effect e = Effect::Static;
    check(effectFromName("breath", e) && e == Effect::Breath, "lookup is case-insensitive");
    check(effectFromName("MusicalRhythm", e) && static_cast<int>(e) == 0x0A,
          "MusicalRhythm is id 0x0A");
    check(!effectFromName("nope", e), "unknown name is rejected");
    check(std::string(effectName(Effect::Fireworks)) == "Fireworks", "name round-trips");
    checkEq(allEffects().size(), std::size_t{17}, "17 effects in the table");
}

void testKeyMap() {
    std::cout << "key map\n";
    checkEq(keys().size(), std::size_t{61}, "61 physical keys");
    checkEq(kMatrixCells, 132, "132 matrix cells");

    // Q sits at indexX=2, indexY=1 in the vendor's custom-light XML.
    const KeyInfo* q = findKey("Q");
    check(q != nullptr, "Q exists");
    if (q != nullptr) {
        checkEq(q->ledRow, 2, "Q is on LED row 2");
        checkEq(q->ledCol, 1, "Q is on LED column 1");
        checkEq(q->ledIndex, 45, "Q LED index is 2*22+1");
        checkEq(q->hidUsage, 0x14, "Q HID usage is 0x14");
    }

    const KeyInfo* esc = findKey("esc");
    check(esc != nullptr && esc->ledIndex == 22, "Esc is the first cell of row 1");
    check(findKey("ESCAPE") != nullptr, "alias 'escape' resolves");
    check(findKey("ctrl") == findKey("LeftCtrl"), "alias 'ctrl' means LeftCtrl");
    check(findKey("'") == findKey("Quote"), "punctuation alias resolves");
    check(findKey("-") == findKey("Minus"), "'-' alias survives normalisation");
    check(findKey("nosuchkey") == nullptr, "unknown key returns nullptr");

    const KeyInfo* space = findKey("space");
    check(space != nullptr && space->ledRow == 5 && space->ledCol == 6, "Space is at row 5 col 6");

    // Every key must land in range and be unique.
    std::vector<int> seen;
    for (const auto& k : keys()) {
        check(k.ledIndex < kMatrixCells, std::string("ledIndex in range for ") + k.name);
        checkEq(k.ledIndex, k.ledRow * kMatrixCols + k.ledCol,
                std::string("ledIndex consistent for ") + k.name);
        check(std::find(seen.begin(), seen.end(), k.ledIndex) == seen.end(),
              std::string("ledIndex unique for ") + k.name);
        seen.push_back(k.ledIndex);
    }

    check(keyAt(2, 1) == q, "keyAt finds Q");
    check(keyAt(0, 0) == nullptr, "row 0 has no keys on a 61-key board");
}

void testLedFrame() {
    std::cout << "LED frame\n";
    LedFrame frame;
    const auto black = frame.toBytes();
    checkEq(black.size(), std::size_t{132 * 3}, "frame flattens to 396 bytes");
    bool allZero = true;
    for (const auto b : black) {
        if (b != 0) allZero = false;
    }
    check(allZero, "a fresh frame is black");

    check(frame.setKey("W", Rgb{1, 2, 3}), "setKey accepts a known key");
    check(!frame.setKey("bogus", Rgb{1, 2, 3}), "setKey rejects an unknown key");

    const KeyInfo* w = findKey("W");
    const auto bytes = frame.toBytes();
    const std::size_t off = static_cast<std::size_t>(w->ledIndex) * 3;
    checkEq(bytes[off + 0], 1, "W red byte");
    checkEq(bytes[off + 1], 2, "W green byte");
    checkEq(bytes[off + 2], 3, "W blue byte");

    frame.setAllKeys(Rgb{9, 9, 9});
    const auto all = frame.toBytes();
    // Row 0 has no keys, so it must stay black even after setAllKeys.
    checkEq(all[0], 0, "unused matrix cells stay black");
    checkEq(all[static_cast<std::size_t>(findKey("Esc")->ledIndex) * 3], 9, "Esc is painted");

    // Chunking: 396 bytes at 54 per packet is 7 full packets plus 18 bytes.
    checkEq(all.size() / 54, std::size_t{7}, "7 full colour packets");
    checkEq(all.size() % 54, std::size_t{18}, "plus a final 18-byte packet");
}

}  // namespace

int main() {
    std::cout << "maxfit protocol tests\n\n";
    testFraming();
    testSetLighting();
    testCapabilityParsing();
    testDeviceInfoParsing();
    testEffectNames();
    testKeyMap();
    testLedFrame();

    std::cout << "\n" << (g_checks - g_failures) << "/" << g_checks << " checks passed\n";
    if (g_failures != 0) {
        std::cout << g_failures << " FAILED\n";
        return 1;
    }
    std::cout << "OK\n";
    return 0;
}
