//! A full per-key colour frame and its on-the-wire packetisation.

use crate::color::Rgb;
use crate::keys::{self, MATRIX_CELLS, MATRIX_COLS};
use crate::protocol::{Command, PAYLOAD_OFFSET, REPORT_SIZE, Report, make_request};

/// 54 bytes is 18 cells: the largest multiple of 3 that fits the 58-byte payload
/// window. The vendor software uses exactly this chunking.
pub const COLOR_CELLS_PER_PACKET: usize = 18;
pub const COLOR_BYTES_PER_PACKET: usize = COLOR_CELLS_PER_PACKET * 3; // 54
pub const FRAME_BYTES: usize = MATRIX_CELLS * 3; // 396
/// 7 packets of 54 bytes plus a final 18-byte packet.
pub const COLOR_PACKETS: usize = FRAME_BYTES.div_ceil(COLOR_BYTES_PER_PACKET); // 8

/// Which hardware path a colour frame is being written to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StreamCmd {
    /// `0x0F` -- applied immediately. Verified working on this unit with no
    /// effect change beforehand.
    Live,
    /// `0x09` -- stored into an onboard custom slot instead of applied.
    Slot,
}

impl StreamCmd {
    fn command(self) -> Command {
        match self {
            StreamCmd::Live => Command::SetPerKeyColors,
            StreamCmd::Slot => Command::WriteCustomLedProfile,
        }
    }
}

/// One `Rgb` per 6x22 matrix cell.
///
/// Named `LedFrame` rather than `Frame` because `eframe::Frame` is in scope all
/// over the GUI crate.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LedFrame {
    pub cells: [Rgb; MATRIX_CELLS],
}

impl Default for LedFrame {
    fn default() -> Self {
        Self::BLACK
    }
}

impl LedFrame {
    pub const BLACK: LedFrame = LedFrame {
        cells: [Rgb::BLACK; MATRIX_CELLS],
    };

    pub fn new() -> Self {
        Self::BLACK
    }

    /// Paints every matrix cell, including the 71 with no key over them.
    pub fn fill(&mut self, c: Rgb) {
        self.cells.fill(c);
    }

    pub fn clear(&mut self) {
        self.fill(Rgb::BLACK);
    }

    /// Paints only the 61 real keys, leaving keyless cells black.
    pub fn set_all_keys(&mut self, c: Rgb) {
        self.clear();
        for k in keys::keys() {
            self.cells[k.led] = c;
        }
    }

    /// Returns false when `name` is not a key on this board.
    pub fn set_key(&mut self, name: &str, c: Rgb) -> bool {
        match keys::find(name) {
            Some(k) => {
                self.cells[k.led] = c;
                true
            }
            None => false,
        }
    }

    pub fn at(&self, row: usize, col: usize) -> Rgb {
        self.cells[row * MATRIX_COLS + col]
    }

    pub fn set_at(&mut self, row: usize, col: usize, c: Rgb) {
        self.cells[row * MATRIX_COLS + col] = c;
    }

    /// Flattened R,G,B stream in matrix order.
    pub fn to_bytes(&self) -> [u8; FRAME_BYTES] {
        let mut out = [0u8; FRAME_BYTES];
        for (i, c) in self.cells.iter().enumerate() {
            out[i * 3] = c.r;
            out[i * 3 + 1] = c.g;
            out[i * 3 + 2] = c.b;
        }
        out
    }
}

impl std::ops::Index<usize> for LedFrame {
    type Output = Rgb;

    fn index(&self, i: usize) -> &Rgb {
        &self.cells[i]
    }
}

impl std::ops::IndexMut<usize> for LedFrame {
    fn index_mut(&mut self, i: usize) -> &mut Rgb {
        &mut self.cells[i]
    }
}

/// Encodes a frame as the 8 reports the firmware expects.
///
/// Entirely stack-resident (512 bytes out, 396 in) so the effect engine can call
/// this every frame without allocating.
pub fn encode_color_stream(cmd: StreamCmd, zone: u8, f: &LedFrame) -> [Report; COLOR_PACKETS] {
    let bytes = f.to_bytes();
    let mut out = [[0u8; REPORT_SIZE]; COLOR_PACKETS];
    for (page, chunk) in bytes.chunks(COLOR_BYTES_PER_PACKET).enumerate() {
        debug_assert!(chunk.len() + PAYLOAD_OFFSET <= REPORT_SIZE);
        out[page] = make_request(cmd.command(), zone, page as u8, chunk);
    }
    out
}
