use hidapi::{HidApi, HidDevice};
use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::time::Duration;

use crate::report;

/// Combined handle: `hidapi` for reads, raw file for feature-report ioctls.
pub struct Device {
    pub hid: HidDevice,
    raw: File,
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
pub fn open_controller(api: &HidApi) -> Result<Device, Box<dyn std::error::Error>> {
    let candidates: Vec<_> = api
        .device_list()
        .filter(|d| {
            d.vendor_id() == report::VID
                && d.product_id() == report::PID
                && d.usage_page() == report::USAGE_PAGE_VENDOR
        })
        .collect();

    for info in candidates {
        let path = info.path().to_str()?;
        let raw = OpenOptions::new().read(true).write(true).open(path)?;
        let hid = info.open_device(api)?;

        // Probe: try to clear digital mappings (Report ID 1).
        let mut probe = [0u8; 64];
        probe[0] = 0x01;
        probe[1] = report::commands::CLEAR_DIGITAL_MAPPINGS;
        if send_feature_ioctl(&raw, &probe).is_ok() {
            return Ok(Device { hid, raw });
        }
    }

    Err("No controller interface accepted feature reports".into())
}

/// Enable raw accelerometer + gyroscope output.
pub fn enable_imu(device: &Device) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Clear digital button mappings (disable "lizard mode").
    let mut cmd = [0u8; 64];
    cmd[0] = 0x01;
    cmd[1] = report::commands::CLEAR_DIGITAL_MAPPINGS;
    send_feature_ioctl(&device.raw, &cmd)?;

    std::thread::sleep(Duration::from_millis(5));

    // 2. Reset to factory defaults.
    let mut cmd = [0u8; 64];
    cmd[0] = 0x01;
    cmd[1] = report::commands::LOAD_DEFAULT_SETTINGS;
    cmd[2] = 0;
    send_feature_ioctl(&device.raw, &cmd)?;

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

    send_feature_ioctl(&device.raw, &cmd)?;

    Ok(())
}
