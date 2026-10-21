import sys; sys.path.insert(0,'tools')
from loadcfg import load
j=load('qrc/images/configDir/KeyInfo_XS61BRGB.config')
NAMES={
 (2,0):'Esc',(2,1):'1',(2,2):'2',(2,3):'3',(2,4):'4',(2,5):'5',(2,6):'6',(2,7):'7',(2,8):'8',(2,9):'9',
 (2,10):'0',(2,11):'Minus',(2,12):'Equal',(2,14):'Backspace',
 (3,0):'Tab',(3,1):'Q',(3,2):'W',(3,3):'E',(3,4):'R',(3,5):'T',(3,6):'Y',(3,7):'U',(3,8):'I',(3,9):'O',
 (3,10):'P',(3,11):'LeftBracket',(3,12):'RightBracket',(3,14):'Backslash',
 (4,0):'CapsLock',(4,2):'A',(4,3):'S',(4,4):'D',(4,5):'F',(4,6):'G',(4,7):'H',(4,8):'J',(4,9):'K',
 (4,10):'L',(4,11):'Semicolon',(4,12):'Quote',(4,14):'Enter',
 (5,0):'LeftShift',(5,2):'Z',(5,3):'X',(5,4):'C',(5,5):'V',(5,6):'B',(5,7):'N',(5,8):'M',
 (5,9):'Comma',(5,10):'Period',(5,11):'Slash',(5,14):'RightShift',
 (6,0):'LeftCtrl',(6,1):'LeftWin',(6,2):'LeftAlt',(6,6):'Space',(6,10):'RightAlt',(6,11):'Fn',
 (6,12):'Menu',(6,13):'RightCtrl',
}
rows=[]
for r in range(2,7):
    for c,e in enumerate(j['EveryKeyInfo_Row%02d'%r]):
        if e.get('PosX',-1)==-1: continue
        nm=NAMES[(r,c)]
        dec=e['Dec']
        rows.append((nm, dec if dec>=0 else 0xFF, r-1, c))
hdr = '''// KeyMap.cpp
// generated from the vendor software's own
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
'''
body = ''.join('    { "%s", 0x%02X, %d, %d, %d },\n' % (nm, dec, lr, lc, lr*22+lc)
               for nm, dec, lr, lc in rows)
ftr = '''};

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
    {"\\\\", "Backslash"}, {"|", "Backslash"}, {"pipe", "Backslash"},
    {";", "Semicolon"}, {":", "Semicolon"},
    {"'", "Quote"}, {"\\"", "Quote"}, {"apostrophe", "Quote"},
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
'''
open(r'G:\MaxFitStuff\FanTechMaxFitCollection\Driver\src\KeyMap.cpp','w',encoding='utf-8').write(hdr+body+ftr)
print('wrote KeyMap.cpp with %d keys'%len(rows))
