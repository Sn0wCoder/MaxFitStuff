//! HID transport for the MAXFIT61's vendor control interface.
//!
//! The device is selected by usage page / usage rather than by path substring:
//! the vendor app filters paths for `mi_02` / `col06`, which breaks the moment a
//! second identical keyboard or a different enumeration order shows up.

use std::ffi::CString;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};
use maxfit_core::protocol::{
    Command, PRODUCT_ID, REPORT_ID, REPORT_SIZE, Report, USAGE, USAGE_PAGE, VENDOR_ID, is_reply_to,
};

use crate::DeviceError;

/// One HID interface exposed by a USB device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceInfo {
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub product: String,
    pub manufacturer: String,
    pub interface_number: i32,
}

impl InterfaceInfo {
    /// True for the vendor control channel: usage page 0xFF1B, usage 0x91.
    pub fn is_control(&self) -> bool {
        self.usage_page == USAGE_PAGE && self.usage == USAGE
    }
}

/// Everything the driver needs from a 64-byte report pipe.
///
/// Behind a trait so the protocol layer can be exercised against a mock, and so
/// the hidapi backend can be swapped for a raw Win32 one if the firmware ever
/// needs the `HidD_SetOutputReport` fallback the C++ driver carries.
pub trait Transport: Send {
    fn write(&mut self, r: &Report) -> Result<(), DeviceError>;

    /// Returns false on timeout rather than erroring: several queries answer
    /// with more reports than asked for, and some produce none.
    fn read_timeout(&mut self, out: &mut Report, timeout: Duration) -> Result<bool, DeviceError>;

    /// Drops reports the device has already queued, so a following exchange
    /// cannot pick up a stale answer.
    fn drain(&mut self) -> Result<(), DeviceError> {
        let mut scratch = [0u8; REPORT_SIZE];
        // Bounded: a wedged device must not spin here forever.
        for _ in 0..64 {
            if !self.read_timeout(&mut scratch, Duration::from_millis(8))? {
                break;
            }
        }
        Ok(())
    }

    /// Writes `req`, then reads until a reply carrying `cmd` arrives.
    fn request(
        &mut self,
        req: &Report,
        cmd: Command,
        timeout: Duration,
    ) -> Result<Option<Report>, DeviceError> {
        self.drain()?;
        self.write(req)?;

        let deadline = Instant::now() + timeout;
        let mut reply = [0u8; REPORT_SIZE];
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            if !self.read_timeout(&mut reply, left)? {
                break;
            }
            if is_reply_to(&reply, cmd) {
                return Ok(Some(reply));
            }
            // Not ours; the firmware emits extra pages for some queries.
        }
        Ok(None)
    }

    fn path(&self) -> &str;
}

/// Lists every HID interface matching `vid`/`pid` (0 = wildcard).
pub fn enumerate(vid: u16, pid: u16) -> Result<Vec<InterfaceInfo>, DeviceError> {
    let api = HidApi::new()?;
    Ok(api
        .device_list()
        .filter(|d| (vid == 0 || d.vendor_id() == vid) && (pid == 0 || d.product_id() == pid))
        .map(|d| InterfaceInfo {
            path: d.path().to_string_lossy().into_owned(),
            vendor_id: d.vendor_id(),
            product_id: d.product_id(),
            usage_page: d.usage_page(),
            usage: d.usage(),
            product: d.product_string().unwrap_or_default().to_string(),
            manufacturer: d.manufacturer_string().unwrap_or_default().to_string(),
            interface_number: d.interface_number(),
        })
        .collect())
}

/// The vendor control interface, or `None` when the keyboard is not plugged in.
pub fn find_control_interface() -> Result<Option<InterfaceInfo>, DeviceError> {
    Ok(enumerate(VENDOR_ID, PRODUCT_ID)?
        .into_iter()
        .find(InterfaceInfo::is_control))
}

/// hidapi-backed transport.
///
/// `HidApi` and `HidDevice` are not `Sync` and should be treated as
/// thread-affine, so this is always constructed on the thread that will use it
/// — in practice the device worker.
pub struct HidTransport {
    device: HidDevice,
    path: String,
}

impl HidTransport {
    /// Opens the first attached MAXFIT61 control interface.
    pub fn open() -> Result<Self, DeviceError> {
        let iface = find_control_interface()?.ok_or(DeviceError::NotFound)?;
        Self::open_path(&iface.path)
    }

    pub fn open_path(path: &str) -> Result<Self, DeviceError> {
        let api = HidApi::new()?;
        let cpath = CString::new(path).map_err(|_| DeviceError::BadPath)?;
        let device = api.open_path(&cpath)?;
        // Let the driver buffer a few reports so a reply is not dropped while
        // we are still finishing the write.
        let _ = device.set_blocking_mode(true);
        Ok(Self {
            device,
            path: path.to_string(),
        })
    }
}

impl Transport for HidTransport {
    fn write(&mut self, r: &Report) -> Result<(), DeviceError> {
        debug_assert_eq!(r[0], REPORT_ID, "report id must lead the buffer");
        let n = self.device.write(r)?;
        if n != REPORT_SIZE {
            return Err(DeviceError::ShortWrite(n));
        }
        Ok(())
    }

    fn read_timeout(&mut self, out: &mut Report, timeout: Duration) -> Result<bool, DeviceError> {
        out.fill(0);
        let ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        match self.device.read_timeout(out, ms)? {
            0 => Ok(false),
            _ => Ok(true),
        }
    }

    fn path(&self) -> &str {
        &self.path
    }
}

/// Records what was sent and replays canned replies. Lets the protocol layer be
/// tested without hardware.
#[derive(Default)]
pub struct MockTransport {
    pub sent: Vec<Report>,
    pub replies: std::collections::VecDeque<Report>,
}

impl Transport for MockTransport {
    fn write(&mut self, r: &Report) -> Result<(), DeviceError> {
        self.sent.push(*r);
        Ok(())
    }

    fn read_timeout(&mut self, out: &mut Report, _timeout: Duration) -> Result<bool, DeviceError> {
        match self.replies.pop_front() {
            Some(r) => {
                *out = r;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    fn drain(&mut self) -> Result<(), DeviceError> {
        Ok(()) // Keep queued replies; the mock's queue IS the script.
    }

    fn path(&self) -> &str {
        "mock"
    }
}
