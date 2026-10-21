//! High-level keyboard operations, one layer above raw reports.

use std::time::Duration;

use maxfit_core::frame::{COLOR_PACKETS, LedFrame, StreamCmd, encode_color_stream};
use maxfit_core::protocol::{
    Command, DeviceInfo, LightCapabilities, LightingState, encode_set_lighting, make_request,
    parse_device_info, parse_light_capabilities, reply_page, reply_text,
};

use crate::transport::{HidTransport, Transport};
use crate::DeviceError;

const QUERY_TIMEOUT: Duration = Duration::from_millis(600);
const EXTRA_PAGE_TIMEOUT: Duration = Duration::from_millis(250);

pub struct Device {
    transport: Box<dyn Transport>,
    /// Delay between the 8 colour packets of a frame.
    ///
    /// Zero by default. The C++ driver sleeps 2ms here, which costs ~44% of a
    /// 30fps frame budget; it is unnecessary because the writes are already
    /// synchronous and the bus already enforces its own interval. Kept as a knob
    /// so a flaky link can be backed off instead of failing.
    pub inter_packet: Duration,
}

impl Device {
    /// Opens the first attached MAXFIT61 control interface.
    pub fn open() -> Result<Self, DeviceError> {
        Ok(Self::with_transport(Box::new(HidTransport::open()?)))
    }

    pub fn open_path(path: &str) -> Result<Self, DeviceError> {
        Ok(Self::with_transport(Box::new(HidTransport::open_path(path)?)))
    }

    pub fn with_transport(transport: Box<dyn Transport>) -> Self {
        Self {
            transport,
            inter_packet: Duration::ZERO,
        }
    }

    pub fn path(&self) -> &str {
        self.transport.path()
    }

    // ---- queries (read-only, safe) ----

    /// Model, firmware, build date and serial.
    pub fn read_device_info(&mut self) -> Result<DeviceInfo, DeviceError> {
        let mut out = DeviceInfo::default();

        // Page 0 carries "M252,01,KB,TY,XS61BRGB,V1.03.07"; the firmware then
        // volunteers page 1 with the build timestamp.
        let req = make_request(Command::QueryDeviceInfo, 0, 0, &[]);
        if let Some(reply) = self
            .transport
            .request(&req, Command::QueryDeviceInfo, QUERY_TIMEOUT)?
        {
            match reply_page(&reply) {
                0 => {
                    out = parse_device_info(&reply_text(&reply));
                    let mut extra = [0u8; maxfit_core::REPORT_SIZE];
                    if self.transport.read_timeout(&mut extra, EXTRA_PAGE_TIMEOUT)?
                        && maxfit_core::is_reply_to(&extra, Command::QueryDeviceInfo)
                        && reply_page(&extra) == 1
                    {
                        out.build_date = reply_text(&extra);
                    }
                }
                1 => out.build_date = reply_text(&reply),
                _ => {}
            }
        }

        let req = make_request(Command::QueryDeviceSerial, 0, 0, &[]);
        if let Some(reply) = self
            .transport
            .request(&req, Command::QueryDeviceSerial, QUERY_TIMEOUT)?
        {
            out.serial = reply_text(&reply);
        }
        Ok(out)
    }

    /// Supported effects plus the speed/brightness maxima.
    pub fn read_capabilities(&mut self) -> Result<Option<LightCapabilities>, DeviceError> {
        let req = make_request(Command::QueryLightModes, 0, 0, &[]);
        Ok(self
            .transport
            .request(&req, Command::QueryLightModes, QUERY_TIMEOUT)?
            .map(|r| parse_light_capabilities(&r)))
    }

    // ---- lighting ----

    /// Applies a built-in firmware effect.
    pub fn set_lighting(&mut self, s: &LightingState) -> Result<(), DeviceError> {
        self.transport.write(&encode_set_lighting(s))
    }

    /// Puts the lighting back to a known-good state.
    ///
    /// A blanked board looks exactly like dead hardware, and only a frame with
    /// the `ledOpenOrClose` byte clear brings it back — so this is wired to the
    /// panic button, the tray menu and the exit path.
    pub fn restore(&mut self) -> Result<(), DeviceError> {
        self.set_lighting(&LightingState::panic())
    }

    /// Streams a per-key colour frame, applied immediately (`0x0F`).
    ///
    /// Verified on hardware: this takes effect with no effect change beforehand,
    /// even though the board does not advertise the `Custom` effect.
    pub fn present_frame(&mut self, f: &LedFrame) -> Result<(), DeviceError> {
        self.stream(StreamCmd::Live, 0, f)
    }

    /// Writes a frame into onboard custom slot `slot` instead of applying it.
    pub fn store_frame(&mut self, slot: u8, f: &LedFrame) -> Result<(), DeviceError> {
        self.stream(StreamCmd::Slot, slot, f)
    }

    fn stream(&mut self, cmd: StreamCmd, zone: u8, f: &LedFrame) -> Result<(), DeviceError> {
        let packets = encode_color_stream(cmd, zone, f);
        for (i, p) in packets.iter().enumerate() {
            self.transport.write(p)?;
            if !self.inter_packet.is_zero() && i + 1 < COLOR_PACKETS {
                std::thread::sleep(self.inter_packet);
            }
        }
        Ok(())
    }

    pub fn transport_mut(&mut self) -> &mut dyn Transport {
        self.transport.as_mut()
    }
}
