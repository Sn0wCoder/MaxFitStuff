// KeyMap.h -- physical key / LED addressing for the MAXFIT61 (XS61BRGB).
//
// The firmware addresses LEDs as a fixed 6 x 22 matrix, row major:
//
//     ledIndex = ledRow * kMatrixCols + ledCol        (0 .. 131)
//
// Colour uploads stream 3 bytes (R,G,B) per matrix cell in that order, so a
// full frame is 132 * 3 = 396 bytes. Cells with no physical key underneath are
// still part of the stream and are simply sent as black.
//
// Row 0 is the function-key row, which a 61-key board does not have, so every
// MAXFIT61 key lives in rows 1..5. The mapping below was extracted from the
// vendor software's own KeyInfo_XS61BRGB.config (grid row/column) and
// cross-checked against customLightDefaultConfig_61.xml, whose indexX/indexY
// attributes are exactly this ledRow/ledCol pair.
#pragma once

#include <cstdint>
#include <string>
#include <vector>

namespace maxfit {

constexpr int kMatrixRows = 6;
constexpr int kMatrixCols = 22;
constexpr int kMatrixCells = kMatrixRows * kMatrixCols;  // 132
constexpr int kKeyCount = 61;

struct KeyInfo {
    const char* name;        // canonical, case-insensitive for lookup
    std::uint8_t hidUsage;   // USB HID keyboard usage id; 0xFF for Fn (not remappable)
    std::uint8_t ledRow;     // 1..5 on this model
    std::uint8_t ledCol;     // 0..21
    std::uint8_t ledIndex;   // ledRow * 22 + ledCol
};

// All 61 physical keys, in visual order (top-left to bottom-right).
const std::vector<KeyInfo>& keys();

// Case-insensitive name lookup. Also accepts a few aliases ("escape", "ctrl",
// "shift", "win", "'", ",", "-", ...). Returns nullptr when unknown.
const KeyInfo* findKey(const std::string& name);

// Lookup by matrix position; nullptr when that cell has no key.
const KeyInfo* keyAt(int ledRow, int ledCol);

}  // namespace maxfit
