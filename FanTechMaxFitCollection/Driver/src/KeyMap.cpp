// KeyMap.cpp -- generated from the vendor software's own
// :/images/configDir/KeyInfo_XS61BRGB.config (grid row/column + HID usage) and
// cross-checked against :/images/configDir/customLightDefaultConfig_61.xml.
// Edit the generator, not this table, if the layout ever needs regenerating.
#include "maxfit/KeyMap.h"

#include <algorithm>
#include <cctype>
#include <unordered_map>

namespace maxfit {
namespace {

const std::vector<KeyInfo> kKeys = {
    { "Esc", 0x29, 1, 0, 22 },
    { "1", 0x1E, 1, 1, 23 },
    { "2", 0x1F, 1, 2, 24 },
    { "3", 0x20, 1, 3, 25 },
    { "4", 0x21, 1, 4, 26 },
    { "5", 0x22, 1, 5, 27 },
    { "6", 0x23, 1, 6, 28 },
    { "7", 0x24, 1, 7, 29 },
    { "8", 0x25, 1, 8, 30 },
    { "9", 0x26, 1, 9, 31 },
    { "0", 0x27, 1, 10, 32 },
    { "Minus", 0x2D, 1, 11, 33 },
    { "Equal", 0x2E, 1, 12, 34 },
    { "Backspace", 0x2A, 1, 14, 36 },
    { "Tab", 0x2B, 2, 0, 44 },
    { "Q", 0x14, 2, 1, 45 },
    { "W", 0x1A, 2, 2, 46 },
    { "E", 0x08, 2, 3, 47 },
    { "R", 0x15, 2, 4, 48 },
    { "T", 0x17, 2, 5, 49 },
    { "Y", 0x1C, 2, 6, 50 },
    { "U", 0x18, 2, 7, 51 },
    { "I", 0x0C, 2, 8, 52 },
    { "O", 0x12, 2, 9, 53 },
    { "P", 0x13, 2, 10, 54 },
    { "LeftBracket", 0x2F, 2, 11, 55 },
    { "RightBracket", 0x30, 2, 12, 56 },
    { "Backslash", 0x31, 2, 14, 58 },
    { "CapsLock", 0x39, 3, 0, 66 },
    { "A", 0x04, 3, 2, 68 },
    { "S", 0x16, 3, 3, 69 },
    { "D", 0x07, 3, 4, 70 },
    { "F", 0x09, 3, 5, 71 },
    { "G", 0x0A, 3, 6, 72 },
    { "H", 0x0B, 3, 7, 73 },
    { "J", 0x0D, 3, 8, 74 },
    { "K", 0x0E, 3, 9, 75 },
    { "L", 0x0F, 3, 10, 76 },
    { "Semicolon", 0x33, 3, 11, 77 },
    { "Quote", 0x34, 3, 12, 78 },
    { "Enter", 0x28, 3, 14, 80 },
    { "LeftShift", 0xE1, 4, 0, 88 },
    { "Z", 0x1D, 4, 2, 90 },
    { "X", 0x1B, 4, 3, 91 },
    { "C", 0x06, 4, 4, 92 },
    { "V", 0x19, 4, 5, 93 },
    { "B", 0x05, 4, 6, 94 },
    { "N", 0x11, 4, 7, 95 },
    { "M", 0x10, 4, 8, 96 },
    { "Comma", 0x36, 4, 9, 97 },
    { "Period", 0x37, 4, 10, 98 },
    { "Slash", 0x38, 4, 11, 99 },
    { "RightShift", 0xE5, 4, 14, 102 },
    { "LeftCtrl", 0xE0, 5, 0, 110 },
    { "LeftWin", 0xE3, 5, 1, 111 },
    { "LeftAlt", 0xE2, 5, 2, 112 },
    { "Space", 0x2C, 5, 6, 116 },
    { "RightAlt", 0xE6, 5, 10, 120 },
    { "Fn", 0xFF, 5, 11, 121 },
    { "Menu", 0x65, 5, 12, 122 },
    { "RightCtrl", 0xE4, 5, 13, 123 },
};

std::string normalize(const std::string& s) {
    std::string out;
    out.reserve(s.size());
    for (const unsigned char c : s) {
        if (c == '_' || c == '-' || c == ' ') continue;
        out.push_back(static_cast<char>(std::tolower(c)));
    }
    return out;
}

// Spellings people actually type, mapped onto canonical names.
const std::unordered_map<std::string, std::string> kAliases = {
    {"escape", "Esc"},
    {"back", "Backspace"}, {"bksp", "Backspace"},
    {"caps", "CapsLock"},
    {"return", "Enter"},
    {"lshift", "LeftShift"}, {"shift", "LeftShift"},
    {"rshift", "RightShift"},
    {"lctrl", "LeftCtrl"}, {"ctrl", "LeftCtrl"}, {"control", "LeftCtrl"},
    {"rctrl", "RightCtrl"},
    {"lalt", "LeftAlt"}, {"alt", "LeftAlt"},
    {"ralt", "RightAlt"}, {"altgr", "RightAlt"},
    {"lwin", "LeftWin"}, {"win", "LeftWin"}, {"super", "LeftWin"}, {"meta", "LeftWin"},
    {"apps", "Menu"}, {"context", "Menu"}, {"contextmenu", "Menu"},
    {"spacebar", "Space"},
    {"[", "LeftBracket"}, {"]", "RightBracket"},
    {"{", "LeftBracket"}, {"}", "RightBracket"},
    {"lbracket", "LeftBracket"}, {"rbracket", "RightBracket"},
    {"\\", "Backslash"}, {"|", "Backslash"}, {"pipe", "Backslash"},
    {";", "Semicolon"}, {":", "Semicolon"},
    {"'", "Quote"}, {"\"", "Quote"}, {"apostrophe", "Quote"},
    {",", "Comma"}, {"<", "Comma"},
    {".", "Period"}, {">", "Period"}, {"dot", "Period"},
    {"/", "Slash"}, {"?", "Slash"},
    {"-", "Minus"}, {"dash", "Minus"}, {"hyphen", "Minus"},
    {"=", "Equal"}, {"equals", "Equal"}, {"+", "Equal"},
};

}  // namespace

const std::vector<KeyInfo>& keys() { return kKeys; }

const KeyInfo* findKey(const std::string& name) {
    // Punctuation aliases must be looked up before normalize() strips '-'.
    {
        const auto raw = kAliases.find(name);
        if (raw != kAliases.end()) return findKey(raw->second);
    }
    const std::string want = normalize(name);
    if (want.empty()) return nullptr;

    for (const auto& k : kKeys) {
        if (normalize(k.name) == want) return &k;
    }
    const auto alias = kAliases.find(want);
    if (alias != kAliases.end()) {
        const std::string canonical = normalize(alias->second);
        for (const auto& k : kKeys) {
            if (normalize(k.name) == canonical) return &k;
        }
    }
    return nullptr;
}

const KeyInfo* keyAt(int ledRow, int ledCol) {
    for (const auto& k : kKeys) {
        if (k.ledRow == ledRow && k.ledCol == ledCol) return &k;
    }
    return nullptr;
}

}  // namespace maxfit
