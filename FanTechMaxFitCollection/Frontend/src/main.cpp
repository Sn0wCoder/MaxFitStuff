// maxfitctl -- command line control for the Fantech MAXFIT61 (MK857).
//
// Replaces the vendor's "FANTECH MK857 Gaming Keyboard" app for lighting
// control. Talks the keyboard's native vendor HID protocol directly; see
// docs/PROTOCOL.md.
#include <algorithm>
#include <cctype>
#include <cstdio>
#include <cstdlib>
#include <iostream>
#include <map>
#include <sstream>
#include <string>
#include <vector>

#include "maxfit/Keyboard.h"

namespace {

using namespace maxfit;

const std::map<std::string, Rgb> kNamedColors = {
    {"black", {0, 0, 0}},        {"off", {0, 0, 0}},
    {"white", {255, 255, 255}},  {"red", {255, 0, 0}},
    {"green", {0, 255, 0}},      {"blue", {0, 0, 255}},
    {"yellow", {255, 255, 0}},   {"cyan", {0, 255, 255}},
    {"magenta", {255, 0, 255}},  {"orange", {255, 90, 0}},
    {"purple", {140, 0, 255}},   {"pink", {255, 60, 160}},
    {"teal", {0, 180, 160}},     {"lime", {170, 255, 0}},
};

std::string lower(std::string s) {
    std::transform(s.begin(), s.end(), s.begin(),
                   [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
    return s;
}

bool parseHexByte(const std::string& s, std::size_t pos, std::uint8_t& out) {
    auto nibble = [](char c, int& v) {
        if (c >= '0' && c <= '9') { v = c - '0'; return true; }
        if (c >= 'a' && c <= 'f') { v = c - 'a' + 10; return true; }
        if (c >= 'A' && c <= 'F') { v = c - 'A' + 10; return true; }
        return false;
    };
    int hi = 0, lo = 0;
    if (pos + 1 >= s.size()) return false;
    if (!nibble(s[pos], hi) || !nibble(s[pos + 1], lo)) return false;
    out = static_cast<std::uint8_t>((hi << 4) | lo);
    return true;
}

// Accepts "#RRGGBB", "RRGGBB", "#RGB", "R,G,B" and the names above.
bool parseColor(const std::string& text, Rgb& out) {
    if (text.empty()) return false;

    const auto named = kNamedColors.find(lower(text));
    if (named != kNamedColors.end()) {
        out = named->second;
        return true;
    }

    if (text.find(',') != std::string::npos) {
        std::istringstream in(text);
        std::string part;
        int vals[3] = {-1, -1, -1};
        for (int i = 0; i < 3 && std::getline(in, part, ','); ++i) {
            try {
                vals[i] = std::stoi(part);
            } catch (const std::exception&) {
                return false;
            }
        }
        for (const int v : vals) {
            if (v < 0 || v > 255) return false;
        }
        out = {static_cast<std::uint8_t>(vals[0]), static_cast<std::uint8_t>(vals[1]),
               static_cast<std::uint8_t>(vals[2])};
        return true;
    }

    std::string hex = text;
    if (!hex.empty() && hex[0] == '#') hex.erase(0, 1);

    if (hex.size() == 3) {
        std::string expanded;
        for (const char c : hex) {
            expanded.push_back(c);
            expanded.push_back(c);
        }
        hex = expanded;
    }
    if (hex.size() != 6) return false;
    return parseHexByte(hex, 0, out.r) && parseHexByte(hex, 2, out.g) &&
           parseHexByte(hex, 4, out.b);
}

std::string formatColor(const Rgb& c) {
    char buf[16];
    std::snprintf(buf, sizeof(buf), "#%02X%02X%02X", c.r, c.g, c.b);
    return buf;
}

// Simple "--flag" / "--key value" splitter. Positionals keep their order.
struct Args {
    std::vector<std::string> positional;
    std::map<std::string, std::string> options;

    bool has(const std::string& key) const { return options.count(key) != 0; }

    std::string value(const std::string& key, const std::string& fallback = {}) const {
        const auto it = options.find(key);
        return it == options.end() ? fallback : it->second;
    }

    bool number(const std::string& key, int& out) const {
        const auto it = options.find(key);
        if (it == options.end()) return false;
        try {
            out = std::stoi(it->second);
        } catch (const std::exception&) {
            return false;
        }
        return true;
    }
};

// Flags that never consume the next token.
bool isBooleanFlag(const std::string& name) {
    return name == "random" || name == "off" || name == "on" || name == "store" ||
           name == "no-select" || name == "help";
}

Args parseArgs(int argc, char** argv, int start) {
    Args args;
    for (int i = start; i < argc; ++i) {
        std::string tok = argv[i];
        if (tok.rfind("--", 0) == 0) {
            std::string name = tok.substr(2);
            const auto eq = name.find('=');
            if (eq != std::string::npos) {
                args.options[lower(name.substr(0, eq))] = name.substr(eq + 1);
                continue;
            }
            name = lower(name);
            if (isBooleanFlag(name)) {
                args.options[name] = "1";
            } else if (i + 1 < argc) {
                args.options[name] = argv[++i];
            } else {
                args.options[name] = "1";
            }
        } else {
            args.positional.push_back(tok);
        }
    }
    return args;
}

void printUsage() {
    std::cout <<
        R"(maxfitctl -- Fantech MAXFIT61 (MK857) lighting control

USAGE
  maxfitctl <command> [options]

COMMANDS
  list                    Show every HID interface the keyboard exposes.
  info                    Model, firmware, serial and lighting capabilities.
  effects                 List effects; marks the ones this keyboard reports.
  keys                    Print the key names and their LED matrix positions.

  effect <name> [opts]    Apply a built-in effect.
      --color <c>         Primary colour      (default white)
      --bg <c>            Secondary colour, used by two-colour effects
      --brightness <0-4>  Default 4
      --speed <0-4>       Default 2
      --direction <n>     Effect direction, default 0
      --random            Let the firmware cycle colours instead of --color
      --off               Send the effect with the lighting disabled

  all <colour>            Light every key one colour (per-key mode).
  key <KEY=colour> ...    Per-key colours, e.g. key W=red A=red S=red D=red
      --base <c>          Colour for all other keys (default black)
      --store <slot>      Write to onboard custom slot instead of applying live
  off                     Turn the lighting off.

COLOURS
  #RRGGBB, #RGB, RRGGBB, "R,G,B", or a name:
)";
    std::cout << "  ";
    bool first = true;
    for (const auto& [name, unused] : kNamedColors) {
        (void)unused;
        if (!first) std::cout << ", ";
        std::cout << name;
        first = false;
    }
    std::cout << "\n\nEXAMPLES\n"
                 "  maxfitctl info\n"
                 "  maxfitctl effect static --color #00AAFF\n"
                 "  maxfitctl effect breath --color red --speed 3 --brightness 4\n"
                 "  maxfitctl effect wave --random\n"
                 "  maxfitctl all #202020\n"
                 "  maxfitctl key W=white A=white S=white D=white --base #101010\n"
                 "  maxfitctl off\n";
}

int cmdList() {
    const auto interfaces = enumerateInterfaces();
    if (interfaces.empty()) {
        std::cout << "No device " << std::hex << kVendorId << ":" << kProductId << std::dec
                  << " found.\n";
        return 1;
    }
    std::cout << "Interfaces for " << (interfaces.front().product.empty()
                                           ? "MAXFIT61"
                                           : interfaces.front().product)
              << ":\n\n";
    for (const auto& i : interfaces) {
        const bool control = i.usagePage == kUsagePage && i.usage == kUsage &&
                             i.outputLength == kReportSize;
        std::printf("  %04X:%04X  usagePage 0x%04X  usage 0x%04X  in %-3u out %-3u %s\n",
                    i.vendorId, i.productId, i.usagePage, i.usage, i.inputLength,
                    i.outputLength, control ? "<-- control" : "");
        std::cout << "      " << i.path << "\n";
    }
    return 0;
}

int cmdInfo() {
    Keyboard kb;
    kb.open();
    std::cout << "Path     : " << kb.path() << "\n";
    if (kb.readOnly()) {
        std::cout << "WARNING  : opened read-only; writes will fail.\n";
    }

    const DeviceInfo info = kb.readDeviceInfo();
    std::cout << "Model    : " << (info.model.empty() ? "?" : info.model) << "\n";
    std::cout << "Firmware : " << (info.firmware.empty() ? "?" : info.firmware) << "\n";
    std::cout << "Built    : " << (info.buildDate.empty() ? "?" : info.buildDate) << "\n";
    std::cout << "Serial   : " << (info.serial.empty() ? "?" : info.serial) << "\n";
    std::cout << "Raw id   : " << info.raw << "\n";

    if (const auto caps = kb.readLightCapabilities()) {
        std::cout << "\nLighting capabilities\n";
        std::cout << "  brightness levels : 0.." << static_cast<int>(caps->brightnessMax) << "\n";
        std::cout << "  speed levels      : 0.." << static_cast<int>(caps->speedMax) << "\n";
        std::cout << "  effects (" << caps->effects.size() << "):\n";
        for (const Effect e : caps->effects) {
            std::printf("      0x%02X  %s\n", static_cast<unsigned>(e), effectName(e));
        }
    } else {
        std::cout << "\n(no reply to the lighting capability query)\n";
    }
    return 0;
}

int cmdEffects() {
    std::vector<Effect> supported;
    try {
        Keyboard kb;
        kb.open();
        if (const auto caps = kb.readLightCapabilities()) supported = caps->effects;
    } catch (const Error&) {
        // Listing the table without a keyboard attached is still useful.
    }

    std::cout << "  id    effect            supported\n";
    for (const Effect e : allEffects()) {
        const bool ok = std::find(supported.begin(), supported.end(), e) != supported.end();
        std::printf("  0x%02X  %-17s %s\n", static_cast<unsigned>(e), effectName(e),
                    supported.empty() ? "?" : (ok ? "yes" : "no"));
    }
    if (supported.empty()) {
        std::cout << "\n(keyboard not reachable; 'supported' unknown)\n";
    }
    return 0;
}

int cmdKeys() {
    std::cout << "61 keys, LED matrix is " << kMatrixRows << " rows x " << kMatrixCols
              << " cols (" << kMatrixCells << " cells).\n"
              << "ledIndex = row * " << kMatrixCols << " + col\n\n";
    std::cout << "  key            hid   row  col  led\n";
    for (const auto& k : keys()) {
        std::printf("  %-14s 0x%02X  %3u  %3u  %3u\n", k.name, k.hidUsage, k.ledRow, k.ledCol,
                    k.ledIndex);
    }
    return 0;
}

int cmdEffect(const Args& args) {
    if (args.positional.empty()) {
        std::cerr << "error: effect name required (try 'maxfitctl effects')\n";
        return 2;
    }
    LightingState s;
    if (!effectFromName(args.positional[0], s.effect)) {
        std::cerr << "error: unknown effect '" << args.positional[0]
                  << "' (try 'maxfitctl effects')\n";
        return 2;
    }

    s.color = {255, 255, 255};
    if (args.has("color")) {
        if (!parseColor(args.value("color"), s.color)) {
            std::cerr << "error: cannot parse --color '" << args.value("color") << "'\n";
            return 2;
        }
    }
    if (args.has("bg")) {
        if (!parseColor(args.value("bg"), s.background)) {
            std::cerr << "error: cannot parse --bg '" << args.value("bg") << "'\n";
            return 2;
        }
    }

    int n = 0;
    if (args.number("brightness", n)) {
        if (n < 0 || n > 4) {
            std::cerr << "error: --brightness must be 0..4\n";
            return 2;
        }
        s.brightness = static_cast<std::uint8_t>(n);
    }
    if (args.number("speed", n)) {
        if (n < 0 || n > 4) {
            std::cerr << "error: --speed must be 0..4\n";
            return 2;
        }
        s.speed = static_cast<std::uint8_t>(n);
    }
    if (args.number("direction", n)) s.direction = static_cast<std::uint8_t>(n);
    s.randomColor = args.has("random");
    s.enabled = !args.has("off");

    Keyboard kb;
    kb.open();
    kb.setLighting(s);

    std::cout << "Applied " << effectName(s.effect);
    if (s.randomColor) {
        std::cout << ", colour cycling";
    } else {
        std::cout << ", colour " << formatColor(s.color);
    }
    std::cout << ", brightness " << static_cast<int>(s.brightness) << ", speed "
              << static_cast<int>(s.speed);
    if (!s.enabled) std::cout << " (lighting off)";
    std::cout << ".\n";
    return 0;
}

int applyFrame(const Args& args, const LedFrame& frame, const std::string& what) {
    Keyboard kb;
    kb.open();

    int slot = -1;
    if (args.has("store")) {
        if (!args.number("store", slot) || slot < 0 || slot > 255) {
            std::cerr << "error: --store needs a slot number\n";
            return 2;
        }
        kb.storeCustomProfile(static_cast<std::uint8_t>(slot), frame);
        std::cout << "Stored " << what << " into custom slot " << slot << ".\n";
        return 0;
    }

    if (args.has("no-select")) {
        kb.uploadPerKeyColors(frame);
    } else {
        kb.setPerKeyColors(frame);
    }
    std::cout << "Applied " << what << ".\n";
    return 0;
}

int cmdAll(const Args& args) {
    if (args.positional.empty()) {
        std::cerr << "error: colour required, e.g. 'maxfitctl all #00AAFF'\n";
        return 2;
    }
    Rgb c{};
    if (!parseColor(args.positional[0], c)) {
        std::cerr << "error: cannot parse colour '" << args.positional[0] << "'\n";
        return 2;
    }
    LedFrame frame;
    frame.setAllKeys(c);
    return applyFrame(args, frame, "all keys " + formatColor(c));
}

int cmdKey(const Args& args) {
    if (args.positional.empty()) {
        std::cerr << "error: expected KEY=colour pairs, e.g. 'maxfitctl key W=red A=red'\n";
        return 2;
    }

    Rgb base{};
    if (args.has("base") && !parseColor(args.value("base"), base)) {
        std::cerr << "error: cannot parse --base '" << args.value("base") << "'\n";
        return 2;
    }

    LedFrame frame;
    frame.setAllKeys(base);

    int painted = 0;
    for (const auto& spec : args.positional) {
        const auto eq = spec.find('=');
        if (eq == std::string::npos) {
            std::cerr << "error: '" << spec << "' is not KEY=colour\n";
            return 2;
        }
        const std::string keyName = spec.substr(0, eq);
        const std::string colorText = spec.substr(eq + 1);

        Rgb c{};
        if (!parseColor(colorText, c)) {
            std::cerr << "error: cannot parse colour '" << colorText << "' for key '" << keyName
                      << "'\n";
            return 2;
        }
        if (!frame.setKey(keyName, c)) {
            std::cerr << "error: unknown key '" << keyName << "' (try 'maxfitctl keys')\n";
            return 2;
        }
        ++painted;
    }
    return applyFrame(args, frame, std::to_string(painted) + " key(s)");
}

int cmdOff() {
    LightingState s;
    s.effect = Effect::Static;
    s.brightness = 0;
    s.color = {0, 0, 0};
    s.enabled = false;

    Keyboard kb;
    kb.open();
    kb.setLighting(s);
    std::cout << "Lighting off.\n";
    return 0;
}

}  // namespace

int main(int argc, char** argv) {
    if (argc < 2) {
        printUsage();
        return 1;
    }

    const std::string command = lower(argv[1]);
    if (command == "-h" || command == "--help" || command == "help") {
        printUsage();
        return 0;
    }

    const Args args = parseArgs(argc, argv, 2);

    try {
        if (command == "list") return cmdList();
        if (command == "info") return cmdInfo();
        if (command == "effects") return cmdEffects();
        if (command == "keys") return cmdKeys();
        if (command == "effect") return cmdEffect(args);
        if (command == "all") return cmdAll(args);
        if (command == "key") return cmdKey(args);
        if (command == "off") return cmdOff();

        std::cerr << "error: unknown command '" << argv[1] << "'\n\n";
        printUsage();
        return 2;
    } catch (const Error& e) {
        std::cerr << "error: " << e.what() << "\n";
        return 1;
    } catch (const std::exception& e) {
        std::cerr << "error: " << e.what() << "\n";
        return 1;
    }
}
