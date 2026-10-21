"""
Usage:  python -I tools/gen_keys.py
"""
import json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
SRC = os.path.join(ROOT, "vendor-data", "KeyInfo_XS61BRGB.config")
DST = os.path.join(ROOT, "maxfit-core", "data", "keys61.json")

COLS = 22


def load_vendor_config(path):
    """The vendor file is JSON except for leading zeros and trailing commas."""
    text = open(path, "rb").read().decode("utf-8", "replace")
    text = re.sub(r"(:\s*)-?0+(\d)", lambda m: m.group(1) + ("-" if "-" in m.group(0) else "") + m.group(2), text)
    text = re.sub(r":\s*0+(?=[,\s}])", ": 0", text)
    text = re.sub(r",(\s*[}\]])", r"\1", text)
    return json.loads(text)


# Canonical name per (configRow, configCol). Lookup is case-insensitive later.
NAMES = {
    (2, 0): "Esc", (2, 1): "1", (2, 2): "2", (2, 3): "3", (2, 4): "4", (2, 5): "5",
    (2, 6): "6", (2, 7): "7", (2, 8): "8", (2, 9): "9", (2, 10): "0",
    (2, 11): "Minus", (2, 12): "Equal", (2, 14): "Backspace",
    (3, 0): "Tab", (3, 1): "Q", (3, 2): "W", (3, 3): "E", (3, 4): "R", (3, 5): "T",
    (3, 6): "Y", (3, 7): "U", (3, 8): "I", (3, 9): "O", (3, 10): "P",
    (3, 11): "LeftBracket", (3, 12): "RightBracket", (3, 14): "Backslash",
    (4, 0): "CapsLock", (4, 2): "A", (4, 3): "S", (4, 4): "D", (4, 5): "F",
    (4, 6): "G", (4, 7): "H", (4, 8): "J", (4, 9): "K", (4, 10): "L",
    (4, 11): "Semicolon", (4, 12): "Quote", (4, 14): "Enter",
    (5, 0): "LeftShift", (5, 2): "Z", (5, 3): "X", (5, 4): "C", (5, 5): "V",
    (5, 6): "B", (5, 7): "N", (5, 8): "M", (5, 9): "Comma", (5, 10): "Period",
    (5, 11): "Slash", (5, 14): "RightShift",
    (6, 0): "LeftCtrl", (6, 1): "LeftWin", (6, 2): "LeftAlt", (6, 6): "Space",
    (6, 10): "RightAlt", (6, 11): "Fn", (6, 12): "Menu", (6, 13): "RightCtrl",
}

# What gets painted on the keycap: (main legend, shifted legend).
# Alphanumerics fall through to the vendor's own Character string.
LEGENDS = {
    "Esc": ("Esc", ""), "Minus": ("-", "_"), "Equal": ("=", "+"),
    "Backspace": ("←", ""), "Tab": ("Tab", ""),
    "LeftBracket": ("[", "{"), "RightBracket": ("]", "}"), "Backslash": ("\\", "|"),
    "CapsLock": ("Caps", ""), "Semicolon": (";", ":"), "Quote": ("'", "\""),
    "Enter": ("Enter", ""), "LeftShift": ("Shift", ""), "RightShift": ("Shift", ""),
    "Comma": (",", "<"), "Period": (".", ">"), "Slash": ("/", "?"),
    "LeftCtrl": ("Ctrl", ""), "LeftWin": ("Win", ""), "LeftAlt": ("Alt", ""),
    "Space": ("", ""), "RightAlt": ("Alt", ""), "Fn": ("Fn", ""),
    "Menu": ("Menu", ""), "RightCtrl": ("Ctrl", ""),
}

# Extra spellings people actually type.
ALIASES = {
    "escape": "Esc", "back": "Backspace", "bksp": "Backspace", "caps": "CapsLock",
    "return": "Enter", "lshift": "LeftShift", "shift": "LeftShift",
    "rshift": "RightShift", "lctrl": "LeftCtrl", "ctrl": "LeftCtrl",
    "control": "LeftCtrl", "rctrl": "RightCtrl", "lalt": "LeftAlt", "alt": "LeftAlt",
    "ralt": "RightAlt", "altgr": "RightAlt", "lwin": "LeftWin", "win": "LeftWin",
    "super": "LeftWin", "meta": "LeftWin", "spacebar": "Space", "app": "Menu",
    "context": "Menu", "-": "Minus", "=": "Equal", "[": "LeftBracket",
    "]": "RightBracket", "\\": "Backslash", ";": "Semicolon", "'": "Quote",
    ",": "Comma", ".": "Period", "/": "Slash",
}


def main():
    cfg = load_vendor_config(SRC)
    keys = []
    for cfg_row in range(2, 7):
        entries = cfg["EveryKeyInfo_Row%02d" % cfg_row]
        for cfg_col, e in enumerate(entries):
            if e.get("PosX", -1) == -1:
                continue  # no physical key in this grid slot
            name = NAMES[(cfg_row, cfg_col)]
            led_row = cfg_row - 1
            led = led_row * COLS + cfg_col
            assert 0 <= led < COLS * 6, (name, led)

            main_legend, shift_legend = LEGENDS.get(name, (None, None))
            if main_legend is None:
                ch = e.get("Character", name)
                main_legend = ch[0] if ch else name
                shift_legend = ch[1] if len(ch) > 1 else ""

            dec = e["Dec"]
            keys.append({
                "name": name,
                "legend": main_legend,
                "shift_legend": shift_legend,
                # Fn is handled in firmware and reports -10; it is not remappable.
                "hid": dec if dec >= 0 else 0xFF,
                "row": led_row,
                "col": cfg_col,
                "led": led,
                "x": e["PosX"], "y": e["PosY"],
                "w": e["Width"], "h": e["Height"],
            })

    assert len(keys) == 61, "expected 61 keys, got %d" % len(keys)
    assert len({k["led"] for k in keys}) == 61, "duplicate LED index"

    x0 = min(k["x"] for k in keys)
    y0 = min(k["y"] for k in keys)
    x1 = max(k["x"] + k["w"] for k in keys)
    y1 = max(k["y"] + k["h"] for k in keys)

    out = {
        "_generated_by": "tools/gen_keys.py from vendor-data/KeyInfo_XS61BRGB.config",
        "model": cfg["Keyboard"].get("KeyboardIntroduction", "61Key"),
        "matrix_rows": 6,
        "matrix_cols": COLS,
        "bounds": {"x": x0, "y": y0, "w": x1 - x0, "h": y1 - y0},
        "aliases": ALIASES,
        "keys": keys,
    }
    os.makedirs(os.path.dirname(DST), exist_ok=True)
    with open(DST, "w", encoding="utf-8", newline="\n") as f:
        json.dump(out, f, indent=1, ensure_ascii=False)
        f.write("\n")
    print("wrote %s: %d keys, bounds %dx%d at (%d,%d)"
          % (DST, len(keys), x1 - x0, y1 - y0, x0, y0))


if __name__ == "__main__":
    sys.exit(main())
