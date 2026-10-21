//! Hardware access for the Fantech MAXFIT61.
//!
//! Everything that touches USB lives here; `maxfit-core` stays pure so the wire
//! format can be tested without a keyboard.

pub mod device;
pub mod timer;
pub mod transport;
pub mod worker;

pub use device::Device;
pub use timer::Pacer;
pub use worker::{DeviceCmd, DeviceEvt, DeviceLink, EngineStats, Preview};
pub use transport::{HidTransport, InterfaceInfo, MockTransport, Transport, enumerate, find_control_interface};

#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("no MAXFIT61 control interface found (is the keyboard plugged in?)")]
    NotFound,

    #[error("device path is not valid")]
    BadPath,

    #[error("HID error: {0}")]
    Hid(#[from] hidapi::HidError),

    #[error("short write: {0} of 64 bytes")]
    ShortWrite(usize),

    #[error("timed out waiting for a reply to {0}")]
    Timeout(&'static str),
}

impl DeviceError {
    /// True when the error most likely means the keyboard was unplugged, as
    /// opposed to a protocol or permission problem. hidapi flattens OS errors
    /// into a message string, so this is a heuristic — confirm by re-enumerating
    /// before declaring the device gone.
    pub fn is_disconnect(&self) -> bool {
        match self {
            DeviceError::NotFound => true,
            DeviceError::Hid(e) => {
                let m = e.to_string().to_ascii_lowercase();
                m.contains("not connected")
                    || m.contains("device not found")
                    || m.contains("invalid handle")
                    || m.contains("the device is not")
            }
            _ => false,
        }
    }
}
