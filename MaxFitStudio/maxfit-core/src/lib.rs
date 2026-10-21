//! Protocol, key layout and colour maths for the Fantech MAXFIT61 (MK857).
//!
//! This crate is deliberately pure: no HID, no OS calls, no GUI. That keeps the
//! wire format unit-testable with no keyboard attached, which is what pins it to
//! the behaviour recovered from the vendor software.
//!
//! Hardware access lives in `maxfit-device`; the app lives in `maxfit-gui`.

pub mod color;
pub mod effects;
pub mod frame;
pub mod keys;
pub mod protocol;

pub use color::Rgb;
pub use effects::{AnimParams, AnimationKind, Animator};
pub use frame::{
    COLOR_BYTES_PER_PACKET, COLOR_CELLS_PER_PACKET, COLOR_PACKETS, FRAME_BYTES, LedFrame,
    StreamCmd, encode_color_stream,
};
pub use keys::{
    Bounds, FN_USAGE, KEY_COUNT, Key, Layout, MATRIX_CELLS, MATRIX_COLS, MATRIX_ROWS, find, keys,
    layout,
};
pub use protocol::{
    Command, DeviceInfo, Effect, LightCapabilities, LightingState, MAX_PAYLOAD, PAYLOAD_OFFSET,
    PRODUCT_ID, REPORT_ID, REPORT_SIZE, Report, USAGE, USAGE_PAGE, VENDOR_ID, encode_set_lighting,
    is_reply_to, make_request, parse_device_info, parse_light_capabilities, reply_len, reply_page,
    reply_text,
};
