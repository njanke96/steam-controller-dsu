use hidapi::{HidApi, HidDevice};
use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::errors::DeviceError;
use crate::report;

/// Combined handle: `hidapi` for reads, raw file for feature-report ioctls.
pub struct Device {
    pub hid: HidDevice,
    raw: Arc<Mutex<File>>,
}

impl Device {
    /// Cloneable handle to the raw file for use in other threads.
    pub fn raw_file(&self) -> Arc<Mutex<File>> {
        Arc::clone(&self.raw)
    }
}

/// x86_64 Linux HIDIOCSFEATURE for a 64-byte buffer.
const HIDIOCSFEATURE_64: libc::c_ulong = 0xC040_4806;

fn send_feature_ioctl(file: &File, data: &[u8; 64]) -> Result<(), std::io::Error> {
    let ret = unsafe { libc::ioctl(file.as_raw_fd(), HIDIOCSFEATURE_64, data.as_ptr()) };
    if ret < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Enumerate all vendor interfaces and return the first one that accepts feature reports.
pub fn open_controller(api: &HidApi) -> Result<Device, DeviceError> {
    let candidates: Vec<_> = api
        .device_list()
        .filter(|d| {
            d.vendor_id() == report::VID
                && d.product_id() == report::PID
                && d.usage_page() == report::USAGE_PAGE_VENDOR
        })
        .collect();

    log::debug!("Found {} candidate vendor interfaces", candidates.len());

    for info in candidates {
        let Ok(path) = info.path().to_str() else {
            log::debug!("Skipping device, could not get a path: {info:?}");
            continue;
        };

        log::debug!("Trying interface at {}", path);

        let raw = OpenOptions::new().read(true).write(true).open(path)?;
        let hid = info.open_device(api)?;

        // Try to clear digital mappings (Report ID 1)
        // If this succeeds we have reason enough to believe the device is valid
        let mut probe = [0u8; 64];
        probe[0] = 0x01;
        probe[1] = report::commands::CLEAR_DIGITAL_MAPPINGS;
        if send_feature_ioctl(&raw, &probe).is_ok() {
            log::info!("Opened controller on {}", path);
            return Ok(Device {
                hid,
                raw: Arc::new(Mutex::new(raw)),
            });
        }
        log::debug!("Interface at {} rejected feature report probe", path);
    }

    Err(DeviceError::NoDeviceFound)
}

/// Enable raw accelerometer + gyroscope output.
pub fn enable_imu(file: &File) -> Result<(), DeviceError> {
    log::debug!("Sending IMU enable sequence...");

    // 1. Clear digital button mappings (disable "lizard mode").
    let mut cmd = [0u8; 64];
    cmd[0] = 0x01;
    cmd[1] = report::commands::CLEAR_DIGITAL_MAPPINGS;
    send_feature_ioctl(file, &cmd)?;
    log::trace!("Sent CLEAR_DIGITAL_MAPPINGS");

    std::thread::sleep(Duration::from_millis(5));

    // 2. Reset to factory defaults.
    let mut cmd = [0u8; 64];
    cmd[0] = 0x01;
    cmd[1] = report::commands::LOAD_DEFAULT_SETTINGS;
    cmd[2] = 0;
    send_feature_ioctl(file, &cmd)?;
    log::trace!("Sent LOAD_DEFAULT_SETTINGS");

    std::thread::sleep(Duration::from_millis(5));

    // 3. Disable trackpad mouse emulation and enable IMU raw data.
    let mut cmd = [0u8; 64];
    cmd[0] = 0x01;
    cmd[1] = report::commands::SET_SETTINGS_VALUES;
    cmd[2] = 9; // 3 settings × 3 bytes each

    cmd[3] = report::settings::LEFT_TRACKPAD_MODE;
    cmd[4] = (report::trackpad_modes::NONE & 0xFF) as u8;
    cmd[5] = (report::trackpad_modes::NONE >> 8) as u8;

    cmd[6] = report::settings::RIGHT_TRACKPAD_MODE;
    cmd[7] = (report::trackpad_modes::NONE & 0xFF) as u8;
    cmd[8] = (report::trackpad_modes::NONE >> 8) as u8;

    let imu_mode = report::imu_modes::SEND_RAW_ACCEL | report::imu_modes::SEND_RAW_GYRO;
    cmd[9] = report::settings::IMU_MODE;
    cmd[10] = (imu_mode & 0xFF) as u8;
    cmd[11] = (imu_mode >> 8) as u8;

    send_feature_ioctl(file, &cmd)?;
    log::debug!("IMU enable sequence complete");

    Ok(())
}
