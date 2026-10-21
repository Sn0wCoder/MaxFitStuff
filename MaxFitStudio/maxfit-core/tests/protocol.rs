//! Wire-format and layout tests. These run without a keyboard attached.
//!
//! The assertions are carried over from the C++ driver's suite
//! (`FanTechMaxFitCollection/Tests/src/main.cpp`), so the Rust and C++
//! implementations stay pinned to the same bytes. Add here and there together.

use maxfit_core::keys::MATRIX_ROWS;
use maxfit_core::*;

// ---- framing -------------------------------------------------------------

#[test]
fn framing_lays_out_the_header() {
    let r = make_request(Command::QueryDeviceInfo, 2, 5, &[0xAA, 0xBB, 0xCC]);

    assert_eq!(r.len(), 64, "report is 64 bytes");
    assert_eq!(r[0], 0x01, "byte 0 is the report id");
    assert_eq!(r[1], 0x0D, "byte 1 is the command");
    assert_eq!(r[2], 2, "byte 2 is the zone");
    assert_eq!(r[3], 0, "byte 3 is reserved");
    assert_eq!(r[4], 5, "byte 4 is the page");
    assert_eq!(r[5], 3, "byte 5 is the payload length");
    assert_eq!(r[6], 0xAA, "payload starts at byte 6");
    assert_eq!(r[8], 0xCC, "payload is contiguous");
    assert_eq!(r[9], 0, "tail is zero filled");

    assert!(is_reply_to(&r, Command::QueryDeviceInfo));
    assert!(!is_reply_to(&r, Command::SetLighting));
}

#[test]
fn oversized_payload_is_clamped_not_overflowed() {
    let big = vec![0x7Fu8; 200];
    let r = make_request(Command::SetPerKeyColors, 0, 0, &big);
    assert_eq!(r[5], 58, "oversized payload is clamped to 58");
    assert_eq!(r[63], 0x7F, "clamped payload fills to the last byte");
}

#[test]
fn reply_text_stops_at_the_nul() {
    let mut r = make_request(Command::QueryDeviceInfo, 0, 0, b"ABC\0DEF");
    assert_eq!(reply_text(&r), "ABC");
    assert_eq!(reply_page(&r), 0);
    // A length field beyond the window must not panic.
    r[5] = 200;
    assert_eq!(reply_len(&r), 58);
}

// ---- set lighting (0x07) -------------------------------------------------

fn sample_state() -> LightingState {
    LightingState {
        effect: Effect::Breath,
        brightness: 3,
        speed: 1,
        color: Rgb::new(0x11, 0x22, 0x33),
        background: Rgb::new(0x44, 0x55, 0x66),
        direction: 1,
        random_color: false,
        enabled: true,
    }
}

#[test]
fn set_lighting_payload_is_byte_exact() {
    let r = encode_set_lighting(&sample_state());

    assert_eq!(r[1], 0x07, "command is 0x07");
    assert_eq!(r[5], 0x0E, "length field is always 14");
    assert_eq!(r[6], 0x02, "effect id in payload byte 0");
    assert_eq!(r[7], 3, "brightness in payload byte 1");
    assert_eq!(r[8], 1, "speed in payload byte 2");
    assert_eq!(r[9], 0x11, "primary R");
    assert_eq!(r[10], 0x22, "primary G");
    assert_eq!(r[11], 0x33, "primary B");
    assert_eq!(r[12], 0x44, "background R");
    assert_eq!(r[13], 0x55, "background G");
    assert_eq!(r[14], 0x66, "background B");
    assert_eq!(r[15], 1, "direction");
    assert_eq!(r[16], 0, "ledModeType clear when not cycling");
    // Inverted on the wire, verified on hardware: 0 lights up, 1 blanks.
    assert_eq!(r[17], 0, "ledOpenOrClose is 0 when enabled");
    assert_eq!(r[18], 0, "payload bytes 12-13 stay zero");
    assert_eq!(r[19], 0);
}

#[test]
fn random_color_replaces_the_colour_block_with_the_marker() {
    let mut s = sample_state();
    s.random_color = true;
    let r = encode_set_lighting(&s);
    assert_eq!(r[16], 1, "ledModeType set when cycling");
    assert_eq!(r[9], 0xFF, "cycling writes the 0xFF marker");
    assert_eq!(r[10], 0, "cycling clears the rest of the colour block");
    assert_eq!(r[14], 0, "cycling clears the background too");
}

#[test]
fn static_never_cycles_colours() {
    let mut s = sample_state();
    s.effect = Effect::Static;
    s.random_color = true;
    s.color = Rgb::new(1, 2, 3);
    let r = encode_set_lighting(&s);
    assert_eq!(r[16], 0, "Static is forced out of colour-cycling mode");
    assert_eq!(r[9], 1, "Static keeps its explicit colour");
}

/// The bug that blanked the board for a whole test session. Both directions are
/// pinned so a refactor cannot quietly flip it back.
#[test]
fn led_open_or_close_is_inverted() {
    let mut s = sample_state();
    s.enabled = false;
    assert_eq!(
        encode_set_lighting(&s)[17],
        1,
        "ledOpenOrClose is 1 when disabled"
    );

    s.enabled = true;
    assert_eq!(encode_set_lighting(&s)[17], 0);
}

#[test]
fn defaults_and_panic_state_always_light_the_board() {
    assert!(LightingState::default().enabled);
    assert_eq!(encode_set_lighting(&LightingState::default())[17], 0);

    let p = LightingState::panic();
    assert!(p.enabled);
    assert_eq!(p.effect, Effect::Static);
    assert_eq!(p.brightness, 4);
    assert_eq!(p.color, Rgb::WHITE);
    assert_eq!(encode_set_lighting(&p)[17], 0, "panic must never blank");
}

/// `enabled` is the one field whose absence is dangerous, so serde must treat a
/// preset that omits it as "lit", never as "blank".
#[test]
fn deserialising_a_preset_without_enabled_defaults_to_lit() {
    let json = r#"{
        "effect": "Wave", "brightness": 4, "speed": 2,
        "color": {"r":0,"g":170,"b":255},
        "background": {"r":0,"g":0,"b":0},
        "direction": 0, "random_color": false
    }"#;
    let s: LightingState = serde_json::from_str(json).unwrap();
    assert!(s.enabled);
    assert_eq!(encode_set_lighting(&s)[17], 0);
}

// ---- the hardware guard --------------------------------------------------

/// 0x0B enters the bootloader and 0x0C flashes a firmware block. Getting either
/// wrong bricks the keyboard, so no encoder may ever emit one. `Command` has no
/// variant for them, which makes this structurally true -- this test proves it
/// across every encoder's full input space.
#[test]
fn no_encoder_can_emit_a_flash_command() {
    const FORBIDDEN: [u8; 2] = [0x0B, 0x0C];

    assert!(Command::try_from(0x0Bu8).is_err());
    assert!(Command::try_from(0x0Cu8).is_err());

    // Every effect x every on/off x every random-colour setting.
    for e in Effect::all() {
        for enabled in [true, false] {
            for random_color in [true, false] {
                let s = LightingState {
                    effect: e,
                    enabled,
                    random_color,
                    ..Default::default()
                };
                assert!(!FORBIDDEN.contains(&encode_set_lighting(&s)[1]));
            }
        }
    }

    // Every colour-stream packet on both sinks.
    let mut f = LedFrame::new();
    f.set_all_keys(Rgb::WHITE);
    for cmd in [StreamCmd::Live, StreamCmd::Slot] {
        for r in encode_color_stream(cmd, 0, &f) {
            assert!(!FORBIDDEN.contains(&r[1]));
        }
    }

    // And every command we are willing to name.
    for c in [
        Command::Handshake,
        Command::SetLighting,
        Command::WriteCustomLedProfile,
        Command::QueryLightModes,
        Command::QueryDeviceInfo,
        Command::SetPerKeyColors,
        Command::QueryDeviceSerial,
        Command::EnterLightSync,
    ] {
        assert!(!FORBIDDEN.contains(&make_request(c, 0, 0, &[])[1]));
    }
}

// ---- queries -------------------------------------------------------------

#[test]
fn capability_parsing_matches_the_real_reply() {
    // Exactly what the keyboard replied on the wire.
    let payload = [
        0x02, 0x01, 0x03, 0x09, 0x04, 0x00, 0x0E, 0x0F, 0x06, 0x07, 0x0B, 0x0C, 0x10, 0x08, 0xA5,
        0x5A, 0x04, 0x04,
    ];
    let r = make_request(Command::QueryLightModes, 0, 0, &payload);
    let caps = parse_light_capabilities(&r);

    assert_eq!(caps.effects.len(), 14, "14 effects reported");
    assert_eq!(caps.speed_max, 4);
    assert_eq!(caps.brightness_max, 4);
    assert_eq!(caps.effects.first(), Some(&(Effect::Breath as u8)));
    assert_eq!(caps.effects.last(), Some(&(Effect::Ripple as u8)));

    // This board does not advertise Custom -- which turned out not to matter,
    // since 0x0F applies per-key colour live without selecting it.
    assert!(!caps.supports(Effect::Custom));
    assert!(!caps.supports(Effect::Snake));
    assert!(!caps.supports(Effect::MusicalRhythm));
    assert!(caps.supports(Effect::Static));
    assert_eq!(caps.known_effects().len(), 14);
}

#[test]
fn device_info_parsing() {
    let info = parse_device_info("M252,01,KB,TY,XS61BRGB,V1.03.07");
    assert_eq!(info.model, "XS61BRGB");
    assert_eq!(info.firmware, "V1.03.07");
}

// ---- effects -------------------------------------------------------------

#[test]
fn effect_names_round_trip() {
    assert_eq!(Effect::from_name("breath"), Some(Effect::Breath));
    assert_eq!(Effect::from_name("MusicalRhythm"), Some(Effect::MusicalRhythm));
    assert_eq!(Effect::MusicalRhythm as u8, 0x0A);
    assert_eq!(Effect::from_name("nope"), None);
    assert_eq!(Effect::Fireworks.name(), "Fireworks");
    assert_eq!(Effect::all().count(), 17);

    for e in Effect::all() {
        assert_eq!(Effect::from_name(e.name()), Some(e), "{} round-trips", e.name());
        assert_eq!(Effect::try_from(e as u8), Ok(e));
    }
}

// ---- key map -------------------------------------------------------------

#[test]
fn key_map_shape() {
    assert_eq!(keys().len(), 61, "61 physical keys");
    assert_eq!(MATRIX_CELLS, 132);

    // Q sits at indexX=2, indexY=1 in the vendor's custom-light XML.
    let q = find("Q").expect("Q exists");
    assert_eq!(q.row, 2);
    assert_eq!(q.col, 1);
    assert_eq!(q.led, 45, "2*22+1");
    assert_eq!(q.hid, 0x14);

    assert_eq!(find("esc").unwrap().led, 22, "Esc is the first cell of row 1");
    assert!(find("ESCAPE").is_some(), "alias 'escape' resolves");
    assert_eq!(
        find("ctrl").unwrap().name,
        find("LeftCtrl").unwrap().name,
        "alias 'ctrl' means LeftCtrl"
    );
    assert_eq!(find("'").unwrap().name, "Quote", "punctuation alias resolves");
    assert_eq!(find("-").unwrap().name, "Minus", "'-' survives normalisation");
    assert!(find("nosuchkey").is_none());

    let space = find("space").unwrap();
    assert_eq!((space.row, space.col), (5, 6));

    assert_eq!(layout().at(2, 1).unwrap().name, "Q");
    assert!(layout().at(0, 0).is_none(), "row 0 has no keys on a 61-key board");
}

#[test]
fn every_key_is_in_range_and_unique() {
    let mut seen = vec![false; MATRIX_CELLS];
    for k in keys() {
        assert!(k.led < MATRIX_CELLS, "{} LED out of range", k.name);
        assert_eq!(k.led, k.row as usize * MATRIX_COLS + k.col as usize);
        assert!(!seen[k.led], "duplicate LED index for {}", k.name);
        seen[k.led] = true;
        // Row 0 is the absent function row.
        assert!((1..MATRIX_ROWS).contains(&(k.row as usize)));
    }
    assert_eq!(seen.iter().filter(|s| **s).count(), 61);
}

#[test]
fn key_geometry_is_sane() {
    let b = layout().bounds;
    assert!(b.w > 0.0 && b.h > 0.0);

    for k in keys() {
        assert!(k.w > 0.0 && k.h > 0.0, "{} has no area", k.name);
        assert!(k.x >= b.x && k.y >= b.y, "{} starts outside the board", k.name);
        assert!(
            k.x + k.w <= b.x + b.w + 0.01 && k.y + k.h <= b.y + b.h + 0.01,
            "{} extends past the board",
            k.name
        );
        assert!((0.0..=1.0).contains(&k.nx), "{} nx out of range", k.name);
        assert!((0.0..=1.0).contains(&k.ny), "{} ny out of range", k.name);
    }

    // No two keycaps may overlap, or painting and hit-testing both go wrong.
    for (i, a) in keys().iter().enumerate() {
        for b in &keys()[i + 1..] {
            let disjoint = a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
            assert!(disjoint, "{} overlaps {}", a.name, b.name);
        }
    }

    // Spot-check against the reference layout: Space is the widest key, and the
    // bottom row sits lowest.
    let space = find("space").unwrap();
    assert!(keys().iter().all(|k| k.w <= space.w), "Space is the widest key");
}

// ---- colour frames -------------------------------------------------------

#[test]
fn led_frame_painting() {
    let mut frame = LedFrame::new();
    assert!(frame.cells.iter().all(|c| c.is_black()), "a fresh frame is black");
    assert_eq!(frame.to_bytes().len(), 396, "frame flattens to 396 bytes");

    assert!(frame.set_key("W", Rgb::new(1, 2, 3)));
    assert!(!frame.set_key("bogus", Rgb::new(1, 2, 3)));

    let w = find("W").unwrap();
    let bytes = frame.to_bytes();
    let off = w.led * 3;
    assert_eq!((bytes[off], bytes[off + 1], bytes[off + 2]), (1, 2, 3));

    frame.set_all_keys(Rgb::new(9, 9, 9));
    let all = frame.to_bytes();
    // Row 0 has no keys, so it must stay black even after set_all_keys.
    assert_eq!(all[0], 0, "unused matrix cells stay black");
    assert_eq!(all[find("Esc").unwrap().led * 3], 9, "Esc is painted");
}

#[test]
fn colour_stream_chunking() {
    let mut frame = LedFrame::new();
    frame.set_all_keys(Rgb::WHITE);
    let packets = encode_color_stream(StreamCmd::Live, 0, &frame);

    assert_eq!(packets.len(), 8, "7 full packets plus a final short one");
    assert_eq!(FRAME_BYTES, 396);
    assert_eq!(COLOR_BYTES_PER_PACKET, 54);

    let mut total = 0usize;
    for (page, p) in packets.iter().enumerate() {
        assert_eq!(p[0], REPORT_ID);
        assert_eq!(p[1], Command::SetPerKeyColors as u8);
        assert_eq!(p[2], 0, "zone 0 is the main key matrix");
        assert_eq!(p[4] as usize, page, "page index increments");
        let want = if page < 7 { 54 } else { 18 };
        assert_eq!(p[5], want, "packet {page} length");
        total += p[5] as usize;
    }
    assert_eq!(total, 396, "the 8 packets carry exactly one frame");

    // The slot sink differs only in the command byte.
    let slot = encode_color_stream(StreamCmd::Slot, 3, &frame);
    assert_eq!(slot[0][1], Command::WriteCustomLedProfile as u8);
    assert_eq!(slot[0][2], 3, "zone carries the slot number");
}

#[test]
fn colour_stream_round_trips_the_cells() {
    let mut frame = LedFrame::new();
    for (i, c) in frame.cells.iter_mut().enumerate() {
        *c = Rgb::new(i as u8, (i * 2) as u8, (i * 3) as u8);
    }
    let packets = encode_color_stream(StreamCmd::Live, 0, &frame);

    let mut flat = Vec::with_capacity(FRAME_BYTES);
    for p in &packets {
        let n = p[5] as usize;
        flat.extend_from_slice(&p[PAYLOAD_OFFSET..PAYLOAD_OFFSET + n]);
    }
    assert_eq!(flat, frame.to_bytes().to_vec(), "packets reassemble to the frame");
}

// ---- colour --------------------------------------------------------------

#[test]
fn colour_parsing() {
    assert_eq!(Rgb::parse("#00AAFF"), Some(Rgb::new(0, 0xAA, 0xFF)));
    assert_eq!(Rgb::parse("00AAFF"), Some(Rgb::new(0, 0xAA, 0xFF)));
    assert_eq!(Rgb::parse("#f0a"), Some(Rgb::new(0xFF, 0x00, 0xAA)));
    assert_eq!(Rgb::parse("255,0,128"), Some(Rgb::new(255, 0, 128)));
    assert_eq!(Rgb::parse("  red "), Some(Rgb::new(255, 0, 0)));
    assert_eq!(Rgb::parse("CYAN"), Some(Rgb::new(0, 255, 255)));
    assert_eq!(Rgb::parse("off"), Some(Rgb::BLACK));

    assert_eq!(Rgb::parse("nope"), None);
    assert_eq!(Rgb::parse("#12"), None);
    assert_eq!(Rgb::parse("300,0,0"), None);
    assert_eq!(Rgb::parse(""), None);

    assert_eq!(Rgb::new(0, 0xAA, 0xFF).to_hex(), "#00AAFF");
}

#[test]
fn colour_maths() {
    assert_eq!(Rgb::lerp(Rgb::BLACK, Rgb::WHITE, 0.0), Rgb::BLACK);
    assert_eq!(Rgb::lerp(Rgb::BLACK, Rgb::WHITE, 1.0), Rgb::WHITE);
    assert_eq!(Rgb::lerp(Rgb::BLACK, Rgb::WHITE, 0.5), Rgb::new(128, 128, 128));
    // Out-of-range t is clamped rather than wrapping or overflowing.
    assert_eq!(Rgb::lerp(Rgb::BLACK, Rgb::WHITE, -1.0), Rgb::BLACK);
    assert_eq!(Rgb::lerp(Rgb::BLACK, Rgb::WHITE, 2.0), Rgb::WHITE);

    assert_eq!(Rgb::WHITE.scale(0.0), Rgb::BLACK);
    assert_eq!(Rgb::WHITE.scale(1.0), Rgb::WHITE);

    assert_eq!(Rgb::from_hsv(0.0, 1.0, 1.0), Rgb::new(255, 0, 0));
    assert_eq!(Rgb::from_hsv(1.0 / 3.0, 1.0, 1.0), Rgb::new(0, 255, 0));
    assert_eq!(Rgb::from_hsv(2.0 / 3.0, 1.0, 1.0), Rgb::new(0, 0, 255));
    assert_eq!(Rgb::from_hsv(0.0, 0.0, 1.0), Rgb::WHITE);
    // Hue wraps rather than clamping.
    assert_eq!(Rgb::from_hsv(1.0, 1.0, 1.0), Rgb::from_hsv(0.0, 1.0, 1.0));
    assert_eq!(Rgb::from_hsv(-0.5, 1.0, 1.0), Rgb::from_hsv(0.5, 1.0, 1.0));
}
