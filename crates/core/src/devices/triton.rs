//! Adapter for the 2026 Steam Controller (Triton)

use hidapi::{HidApi, HidDevice};
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::time::Duration;

use crate::devices::device::Device;
use crate::devices::util::{
    is_u32_masked_button_pressed, scale_stick_to_byte, scale_trigger_to_byte,
};
use crate::dsu::DSUFrame;
use crate::errors::DeviceError;

/// Steam Controller vendor/product IDs.
const VID: u16 = 0x28de;
const PID: u16 = 0x1304;

/// HID usage page for the vendor-defined gamepad interface.
const USAGE_PAGE_VENDOR: u16 = 0xFF00;

/// Feature report command IDs (shared with Steam Deck / old controller).
const CMD_CLEAR_DIGITAL_MAPPINGS: u8 = 0x81;
const CMD_LOAD_DEFAULT_SETTINGS: u8 = 0x8E;
const CMD_SET_SETTINGS_VALUES: u8 = 0x87;

/// Setting register IDs.
const SETTING_LEFT_TRACKPAD_MODE: u8 = 0x07;
const SETTING_RIGHT_TRACKPAD_MODE: u8 = 0x08;
const SETTING_IMU_MODE: u8 = 0x30;

/// Trackpad mode values.
const MODE_NONE: u16 = 0x07;

/// IMU mode bitflags.
const IMU_MODE_SEND_RAW_ACCEL: u16 = 0x08;
const IMU_MODE_SEND_RAW_GYRO: u16 = 0x10;

const FEATURE_REPORT_SLEEP_MILLIS: u64 = 50;
const READ_TIMOUT_MILLIS: i32 = 100;

/// Input report ID for the Triton full-state packet
const REPORT_ID_TRITON_FULL: u8 = 0x42;

/// Total HID report length (including Report ID)
const REPORT_SIZE: usize = 54;

/// Sensor scale factors
const ACCEL_PER_G: f32 = 16384.0;
const GYRO_PER_DPS: f32 = 16.384;

const ANALOG_TRIGGER_TO_DIGITAL_THRESHOLD: u8 = 228; // ~90%

const MASK_A: u32 = 0x0000_0001;
const MASK_B: u32 = 0x0000_0002;
const MASK_X: u32 = 0x0000_0004;
const MASK_Y: u32 = 0x0000_0008;
const MASK_QAM: u32 = 0x0000_0010;
const MASK_R3: u32 = 0x0000_0020;
const MASK_VIEW: u32 = 0x0000_0040;
// const MASK_R4: u32 = 0x0000_0080;
// const MASK_R5: u32 = 0x0000_0100;
const MASK_R: u32 = 0x0000_0200;
const MASK_DPAD_DOWN: u32 = 0x0000_0400;
const MASK_DPAD_RIGHT: u32 = 0x0000_0800;
const MASK_DPAD_LEFT: u32 = 0x0000_1000;
const MASK_DPAD_UP: u32 = 0x0000_2000;
const MASK_MENU: u32 = 0x0000_4000;
const MASK_L3: u32 = 0x0000_8000;
const MASK_STEAM: u32 = 0x0001_0000;
// const MASK_L4: u32 = 0x0002_0000;
// const MASK_L5: u32 = 0x0004_0000;
const MASK_L: u32 = 0x0008_0000;

/// Parsed Triton frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TritonFrame {
    pub seq_num: u8,
    pub buttons: u32,
    pub trigger_left: i16,
    pub trigger_right: i16,
    pub left_stick_x: i16,
    pub left_stick_y: i16,
    pub right_stick_x: i16,
    pub right_stick_y: i16,
    pub left_pad_x: i16,
    pub left_pad_y: i16,
    pub pressure_left: u16,
    pub right_pad_x: i16,
    pub right_pad_y: i16,
    pub pressure_right: u16,
    pub imu_timestamp: u32,
    pub accel_x: i16,
    pub accel_y: i16,
    pub accel_z: i16,
    pub gyro_x: i16,
    pub gyro_y: i16,
    pub gyro_z: i16,
    pub quat_w: i16,
    pub quat_x: i16,
    pub quat_y: i16,
    pub quat_z: i16,
}

impl TritonFrame {
    /// Parse a raw HID report. `data` must include the Report ID byte.
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < REPORT_SIZE || data[0] != REPORT_ID_TRITON_FULL {
            return None;
        }
        let p = &data[1..];
        Some(Self {
            seq_num: p[0],
            buttons: u32::from_le_bytes([p[1], p[2], p[3], p[4]]),
            trigger_left: i16::from_le_bytes([p[5], p[6]]),
            trigger_right: i16::from_le_bytes([p[7], p[8]]),
            left_stick_x: i16::from_le_bytes([p[9], p[10]]),
            left_stick_y: i16::from_le_bytes([p[11], p[12]]),
            right_stick_x: i16::from_le_bytes([p[13], p[14]]),
            right_stick_y: i16::from_le_bytes([p[15], p[16]]),
            left_pad_x: i16::from_le_bytes([p[17], p[18]]),
            left_pad_y: i16::from_le_bytes([p[19], p[20]]),
            pressure_left: u16::from_le_bytes([p[21], p[22]]),
            right_pad_x: i16::from_le_bytes([p[23], p[24]]),
            right_pad_y: i16::from_le_bytes([p[25], p[26]]),
            pressure_right: u16::from_le_bytes([p[27], p[28]]),
            imu_timestamp: u32::from_le_bytes([p[29], p[30], p[31], p[32]]),
            accel_x: i16::from_le_bytes([p[33], p[34]]),
            accel_y: i16::from_le_bytes([p[35], p[36]]),
            accel_z: i16::from_le_bytes([p[37], p[38]]),
            gyro_x: i16::from_le_bytes([p[39], p[40]]),
            gyro_y: i16::from_le_bytes([p[41], p[42]]),
            gyro_z: i16::from_le_bytes([p[43], p[44]]),
            quat_w: i16::from_le_bytes([p[45], p[46]]),
            quat_x: i16::from_le_bytes([p[47], p[48]]),
            quat_y: i16::from_le_bytes([p[49], p[50]]),
            quat_z: i16::from_le_bytes([p[51], p[52]]),
        })
    }
}

impl From<TritonFrame> for DSUFrame {
    fn from(value: TritonFrame) -> Self {
        let l2 = scale_trigger_to_byte(value.trigger_left);
        let r2 = scale_trigger_to_byte(value.trigger_right);

        DSUFrame {
            dpad_left: is_u32_masked_button_pressed(value.buttons, MASK_DPAD_LEFT),
            dpad_down: is_u32_masked_button_pressed(value.buttons, MASK_DPAD_DOWN),
            dpad_right: is_u32_masked_button_pressed(value.buttons, MASK_DPAD_RIGHT),
            dpad_up: is_u32_masked_button_pressed(value.buttons, MASK_DPAD_UP),
            options: is_u32_masked_button_pressed(value.buttons, MASK_VIEW),
            r3: is_u32_masked_button_pressed(value.buttons, MASK_R3),
            l3: is_u32_masked_button_pressed(value.buttons, MASK_L3),
            share: is_u32_masked_button_pressed(value.buttons, MASK_MENU),
            y: is_u32_masked_button_pressed(value.buttons, MASK_Y),
            b: is_u32_masked_button_pressed(value.buttons, MASK_B),
            a: is_u32_masked_button_pressed(value.buttons, MASK_A),
            x: is_u32_masked_button_pressed(value.buttons, MASK_X),
            r1: is_u32_masked_button_pressed(value.buttons, MASK_R),
            l1: is_u32_masked_button_pressed(value.buttons, MASK_L),
            r2: r2 >= ANALOG_TRIGGER_TO_DIGITAL_THRESHOLD,
            l2: l2 >= ANALOG_TRIGGER_TO_DIGITAL_THRESHOLD,
            home: is_u32_masked_button_pressed(value.buttons, MASK_STEAM),
            touch: is_u32_masked_button_pressed(value.buttons, MASK_QAM),
            left_stick_x: scale_stick_to_byte(value.left_stick_x),
            left_stick_y: scale_stick_to_byte(value.left_stick_y),
            right_stick_x: scale_stick_to_byte(value.right_stick_x),
            right_stick_y: scale_stick_to_byte(value.right_stick_y),
            analog_r2: r2,
            analog_l2: l2,
            accel_x: -(value.accel_x as f32 / ACCEL_PER_G),
            accel_y: -(value.accel_z as f32 / ACCEL_PER_G),
            accel_z: (value.accel_y as f32 / ACCEL_PER_G),
            gyro_x: (value.gyro_x as f32 / GYRO_PER_DPS),
            gyro_y: -(value.gyro_z as f32 / GYRO_PER_DPS),
            gyro_z: (value.gyro_y as f32 / GYRO_PER_DPS),
        }
    }
}

/// Triton (Steam Controller 2026) Linux device
pub struct LinuxTriton {
    hid: HidDevice,
    path: String,
}

impl Device for LinuxTriton {
    /// Initialize by enabling IMU on the raw file.
    fn initialize(&self) -> Result<(), DeviceError> {
        let raw = OpenOptions::new().read(true).write(true).open(&self.path)?;
        enable_imu_on_file(&raw)?;
        Ok(())
    }

    /// Read a single DSU frame from the controller.
    fn read_frame(&self) -> Result<DSUFrame, DeviceError> {
        let mut buf = [0u8; 64];
        let n = self.hid.read_timeout(&mut buf, READ_TIMOUT_MILLIS)?;
        if n < REPORT_SIZE {
            return Err(DeviceError::ShortRead(n, REPORT_SIZE));
        }

        let frame = TritonFrame::parse(&buf[..n]).ok_or(DeviceError::InvalidReport(buf[0]))?;

        Ok(frame.into())
    }
}

impl Drop for LinuxTriton {
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

/// Enumerate all vendor interfaces and return the first Triton that responds to a
/// `CMD_CLEAR_DIGITAL_MAPPINGS` probe without error. Requires an [`HidApi`](hidapi::HidApi)
pub fn linux_find_and_open(api: &HidApi) -> Result<LinuxTriton, DeviceError> {
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
            return Ok(LinuxTriton {
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
