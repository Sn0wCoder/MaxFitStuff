#include "maxfit/Keyboard.h"

#include <chrono>
#include <thread>

namespace maxfit {
namespace {

// 18 matrix cells per packet -- 54 payload bytes, the largest multiple of 3
// that fits in the 58-byte payload area. The vendor software uses exactly this.
constexpr std::size_t kCellsPerPacket = 18;
constexpr std::size_t kBytesPerPacket = kCellsPerPacket * 3;

void settle() { std::this_thread::sleep_for(std::chrono::milliseconds(2)); }

}  // namespace

bool LedFrame::setKey(const std::string& name, Rgb c) {
    const KeyInfo* k = findKey(name);
    if (k == nullptr) return false;
    setKey(*k, c);
    return true;
}

void LedFrame::setAllKeys(Rgb c) {
    clear();
    for (const auto& k : keys()) at(k.ledIndex) = c;
}

std::vector<std::uint8_t> LedFrame::toBytes() const {
    std::vector<std::uint8_t> out;
    out.reserve(cells_.size() * 3);
    for (const auto& c : cells_) {
        out.push_back(c.r);
        out.push_back(c.g);
        out.push_back(c.b);
    }
    return out;
}

void Keyboard::open() {
    InterfaceInfo info;
    if (!findControlInterface(info)) {
        throw Error("no MAXFIT61 control interface found (is the keyboard plugged in?)");
    }
    open(info.path);
}

void Keyboard::open(const std::string& devicePath) { transport_.open(devicePath); }

DeviceInfo Keyboard::readDeviceInfo() {
    DeviceInfo out;

    // Page 0 carries "M252,01,KB,TY,XS61BRGB,V1.03.07"; the firmware then
    // volunteers page 1 with the build timestamp.
    Report reply{};
    if (transport_.request(makeRequest(Command::QueryDeviceInfo), Command::QueryDeviceInfo,
                           reply)) {
        if (replyPage(reply) == 0) {
            parseDeviceInfo(replyText(reply), out);
            Report extra{};
            if (transport_.read(extra, 250) && isReplyTo(extra, Command::QueryDeviceInfo) &&
                replyPage(extra) == 1) {
                out.buildDate = replyText(extra);
            }
        } else if (replyPage(reply) == 1) {
            out.buildDate = replyText(reply);
        }
    }

    Report snReply{};
    if (transport_.request(makeRequest(Command::QueryDeviceSerial), Command::QueryDeviceSerial,
                           snReply)) {
        out.serial = replyText(snReply);
    }
    return out;
}

std::optional<LightCapabilities> Keyboard::readLightCapabilities() {
    Report reply{};
    if (!transport_.request(makeRequest(Command::QueryLightModes), Command::QueryLightModes,
                            reply)) {
        return std::nullopt;
    }
    return parseLightCapabilities(reply);
}

void Keyboard::setLighting(const LightingState& s) {
    transport_.write(encodeSetLighting(s));
}

void Keyboard::uploadPerKeyColors(const LedFrame& frame) {
    streamColors(Command::SetPerKeyColors, 0, frame.toBytes());
}

void Keyboard::setPerKeyColors(const LedFrame& frame) {
    // Select the per-key effect first, otherwise an active animation keeps
    // overwriting whatever we upload.
    LightingState s;
    s.effect = Effect::Custom;
    s.brightness = 4;
    s.speed = 2;
    s.enabled = true;
    setLighting(s);
    settle();
    uploadPerKeyColors(frame);
}

void Keyboard::storeCustomProfile(std::uint8_t slot, const LedFrame& frame) {
    streamColors(Command::WriteCustomLedProfile, slot, frame.toBytes());
}

void Keyboard::streamColors(Command cmd,
                            std::uint8_t zone,
                            const std::vector<std::uint8_t>& bytes) {
    std::uint8_t page = 0;
    for (std::size_t offset = 0; offset < bytes.size(); offset += kBytesPerPacket) {
        const std::size_t chunk = std::min(kBytesPerPacket, bytes.size() - offset);
        transport_.write(makeRequest(cmd, zone, page, bytes.data() + offset, chunk));
        ++page;
        settle();
    }
}

}  // namespace maxfit
