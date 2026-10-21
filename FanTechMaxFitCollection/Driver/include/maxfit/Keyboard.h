// Keyboard.h -- high level control surface for the MAXFIT61.
#pragma once

#include <array>
#include <cstdint>
#include <optional>
#include <string>
#include <vector>

#include "HidTransport.h"
#include "KeyMap.h"
#include "Protocol.h"

namespace maxfit {

// A full per-key colour frame: one Rgb per 6x22 matrix cell.
class LedFrame {
public:
    LedFrame() { clear(); }

    void clear(Rgb c = Rgb{}) { cells_.fill(c); }

    Rgb& at(int index) { return cells_.at(static_cast<std::size_t>(index)); }
    const Rgb& at(int index) const { return cells_.at(static_cast<std::size_t>(index)); }

    Rgb& at(int row, int col) { return at(row * kMatrixCols + col); }

    // No-op when `name` is not a key on this board; returns false then.
    bool setKey(const std::string& name, Rgb c);
    void setKey(const KeyInfo& k, Rgb c) { at(k.ledIndex) = c; }

    // Every physical key, leaving unused matrix cells black.
    void setAllKeys(Rgb c);

    // Flattened R,G,B stream in matrix order: kMatrixCells * 3 bytes.
    std::vector<std::uint8_t> toBytes() const;

private:
    std::array<Rgb, kMatrixCells> cells_;
};

class Keyboard {
public:
    // Opens the first attached MAXFIT61 control interface. Throws Error when no
    // keyboard is present or it cannot be opened.
    void open();
    void open(const std::string& devicePath);
    void close() { transport_.close(); }
    bool isOpen() const { return transport_.isOpen(); }

    const std::string& path() const { return transport_.path(); }
    bool readOnly() const { return transport_.readOnly(); }

    // ---- queries (read-only, safe) ----
    DeviceInfo readDeviceInfo();
    std::optional<LightCapabilities> readLightCapabilities();

    // ---- lighting ----
    // Applies a built-in effect with its colours/brightness/speed.
    void setLighting(const LightingState& s);

    // Switches to the per-key Custom effect and uploads `frame`.
    void setPerKeyColors(const LedFrame& frame);

    // Uploads `frame` without touching the current effect.
    void uploadPerKeyColors(const LedFrame& frame);

    // Stores `frame` into onboard custom slot `slot` (0-based) instead of
    // applying it live.
    void storeCustomProfile(std::uint8_t slot, const LedFrame& frame);

    HidTransport& transport() { return transport_; }

private:
    // Streams `bytes` as Command::SetPerKeyColors / WriteCustomLedProfile
    // packets: 54 payload bytes (18 cells) each, page index incrementing.
    void streamColors(Command cmd, std::uint8_t zone, const std::vector<std::uint8_t>& bytes);

    HidTransport transport_;
};

}  // namespace maxfit
