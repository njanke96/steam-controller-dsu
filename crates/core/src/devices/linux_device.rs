use hidapi::{HidApi, HidDevice};
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::time::Duration;

use crate::devices::device::Device;
use crate::errors::DeviceError;
use crate::frame::TritonFrame;

/// Steam Controller vendor/product IDs.
pub const VID: u16 = 0x28de;
pub const PID: u16 = 0x1304;

/// HID usage page for the vendor-defined gamepad interface.
pub const USAGE_PAGE_VENDOR: u16 = 0xFF00;

/// Feature report command IDs (shared with Steam Deck / old controller).
pub const CMD_CLEAR_DIGITAL_MAPPINGS: u8 = 0x81;
pub const CMD_LOAD_DEFAULT_SETTINGS: u8 = 0x8E;
pub const CMD_SET_SETTINGS_VALUES: u8 = 0x87;

/// Setting register IDs.
pub const SETTING_LEFT_TRACKPAD_MODE: u8 = 0x07;
pub const SETTING_RIGHT_TRACKPAD_MODE: u8 = 0x08;
pub const SETTING_IMU_MODE: u8 = 0x30;

/// Trackpad mode values.
pub const MODE_NONE: u16 = 0x07;

/// IMU mode bitflags.
pub const IMU_MODE_SEND_RAW_ACCEL: u16 = 0x08;
pub const IMU_MODE_SEND_RAW_GYRO: u16 = 0x10;

const FEATURE_REPORT_SLEEP_MILLIS: u64 = 50;
const READ_TIMOUT_MILLIS: i32 = 100;

pub struct LinuxDevice {
    hid: HidDevice,
    path: String,
}

impl LinuxDevice {
    /// Enable raw accelerometer + gyroscope output.
    pub fn enable_imu(&self) -> Result<(), DeviceError> {
        let raw = OpenOptions::new().read(true).write(true).open(&self.path)?;
        enable_imu_on_file(&raw)?;
        Ok(())
    }
}

impl Device for LinuxDevice {
    /// Read a single Triton frame from the controller.
    fn read_triton_frame(&self) -> Result<TritonFrame, DeviceError> {
        let mut buf = [0u8; 64];
        let n = self.hid.read_timeout(&mut buf, READ_TIMOUT_MILLIS)?;
        if n < TritonFrame::REPORT_SIZE {
            return Err(DeviceError::ShortRead(n, TritonFrame::REPORT_SIZE));
        }
        TritonFrame::parse(&buf[..n]).ok_or(DeviceError::NonTritonReport(buf[0]))
    }
}

impl Drop for LinuxDevice {
    fn drop(&mut self) {
        // Best-effort cleanup: attempt to return controller to factory defaults
        let Ok(raw) = OpenOptions::new().read(true).write(true).open(&self.path) else {
            return;
        };
        let mut cmd = [0u8; 64];
        cmd[0] = 0x01;
        cmd[1] = CMD_LOAD_DEFAULT_SETTINGS;
        cmd[2] = 0;
        if send_feature_report_via_ioctl(&raw, &cmd).is_ok() {
            log::debug!("IMU disable sequence complete");
        }
    }
}

/// Compute the HIDIOCSFEATURE ioctl number for a buffer length `len`.
///
/// `_IOC(_IOC_WRITE | _IOC_READ, 'H', 0x06, len)`
fn hidiocsfeature(len: usize) -> libc::c_ulong {
    let dir = 3u32; // _IOC_WRITE | _IOC_READ
    ((dir << 30) | ((len as u32) << 16) | ((b'H' as u32) << 8) | 6u32) as libc::c_ulong
}

fn send_feature_report_via_ioctl(file: &std::fs::File, data: &[u8]) -> Result<(), std::io::Error> {
    // TODO: re-evaluate if we can do this with `hidapi` instead of raw
    let ret = unsafe { libc::ioctl(file.as_raw_fd(), hidiocsfeature(data.len()), data.as_ptr()) };
    if ret < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Enumerate all vendor interfaces and return the first one that opens.
pub fn open_controller(api: &HidApi) -> Result<LinuxDevice, DeviceError> {
    let candidates: Vec<_> = api
        .device_list()
        .filter(|d| {
            d.vendor_id() == VID && d.product_id() == PID && d.usage_page() == USAGE_PAGE_VENDOR
        })
        .collect();

    log::debug!("Found {} candidate vendor interfaces", candidates.len());

    for info in candidates {
        let Ok(path) = info.path().to_str() else {
            log::debug!("Skipping device, could not get a path: {info:?}");
            continue;
        };

        log::debug!("Trying interface at {}", path);

        let hid = info.open_device(api)?;

        // Probe with a feature report to verify the controller is actually
        // connected and responsive. The dongle keeps the USB endpoint alive
        // even when the controller is off.
        let Ok(raw) = OpenOptions::new().read(true).write(true).open(path) else {
            log::debug!("Could not open raw hidraw at {}", path);
            continue;
        };
        let mut probe = [0u8; 64];
        probe[0] = 0x01;
        probe[1] = CMD_CLEAR_DIGITAL_MAPPINGS;
        if send_feature_report_via_ioctl(&raw, &probe).is_ok() {
            log::info!("Opened controller on {}", path);
            return Ok(LinuxDevice {
                hid,
                path: path.to_string(),
            });
        }
        log::debug!("Interface at {} rejected feature report probe", path);
    }

    Err(DeviceError::NoDeviceFound)
}

fn enable_imu_on_file(file: &std::fs::File) -> Result<(), DeviceError> {
    log::debug!("Sending IMU enable sequence...");

    // Disable lizard mode
    let mut cmd = [0u8; 64];
    cmd[0] = 0x01;
    cmd[1] = CMD_CLEAR_DIGITAL_MAPPINGS;
    send_feature_report_via_ioctl(file, &cmd)?;
    log::trace!("Sent CLEAR_DIGITAL_MAPPINGS");

    std::thread::sleep(Duration::from_millis(FEATURE_REPORT_SLEEP_MILLIS));

    // Reset to factory defaults
    let mut cmd = [0u8; 64];
    cmd[0] = 0x01;
    cmd[1] = CMD_LOAD_DEFAULT_SETTINGS;
    cmd[2] = 0;
    send_feature_report_via_ioctl(file, &cmd)?;
    log::trace!("Sent LOAD_DEFAULT_SETTINGS");

    std::thread::sleep(Duration::from_millis(FEATURE_REPORT_SLEEP_MILLIS));

    // Disable trackpad mouse emulation and enable IMU raw data
    // This does not seem to interfere with steam input configurations somehow..
    let mut cmd = [0u8; 64];
    cmd[0] = 0x01;
    cmd[1] = CMD_SET_SETTINGS_VALUES;
    cmd[2] = 9; // 3 settings x 3 bytes each

    cmd[3] = SETTING_LEFT_TRACKPAD_MODE;
    cmd[4] = (MODE_NONE & 0xFF) as u8;
    cmd[5] = (MODE_NONE >> 8) as u8;

    cmd[6] = SETTING_RIGHT_TRACKPAD_MODE;
    cmd[7] = (MODE_NONE & 0xFF) as u8;
    cmd[8] = (MODE_NONE >> 8) as u8;

    let imu_mode = IMU_MODE_SEND_RAW_ACCEL | IMU_MODE_SEND_RAW_GYRO;
    cmd[9] = SETTING_IMU_MODE;
    cmd[10] = (imu_mode & 0xFF) as u8;
    cmd[11] = (imu_mode >> 8) as u8;

    send_feature_report_via_ioctl(file, &cmd)?;
    log::debug!("IMU enable sequence complete");

    Ok(())
}
