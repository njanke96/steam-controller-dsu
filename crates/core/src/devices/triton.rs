//! Adapter for the 2026 Steam Controller (Triton)

use hidapi::{HidApi, HidDevice};
use std::time::Duration;

use crate::devices::device::{Device, FrameDevice};
use crate::devices::util::{
    is_u32_masked_button_pressed, scale_stick_to_byte, scale_trigger_to_byte,
};
use crate::devices::{DeviceButton, DeviceConfig, GyroActivationMode};
use crate::dsu::DSUFrame;
use crate::errors::DeviceError;

/// Steam Controller vendor/product IDs.
const VID: u16 = 0x28de;
const PID_WIRED: u16 = 0x1302;
const PID_BT: u16 = 0x1303;
const PID_PUCK: u16 = 0x1304;

/// HID usage page for the vendor-defined gamepad interface.
const USAGE_PAGE_VENDOR_MIN: u16 = 0xFF00;

/// Feature report constants
const FEATURE_REPORT_ID: u8 = 0x01;
const FEATURE_REPORT_SIZE: usize = 64;
const SEND_FEATURE_REPORT_SLEEP_DURATION: Duration = Duration::from_millis(50);
const CMD_SET_SETTINGS_VALUES: u8 = 0x87;

/// Setting register IDs
const SETTING_LIZARD_MODE: u8 = 9;
const SETTING_IMU_MODE: u8 = 48;

/// Setting values
const LIZARD_MODE_OFF: u16 = 0;
const LIZARD_MODE_ON: u16 = 1;
const IMU_MODE_SEND_RAW_ACCEL: u16 = 0x08;
const IMU_MODE_SEND_RAW_GYRO: u16 = 0x10;
const IMU_MODE_GYRO_ACCEL: u16 = IMU_MODE_SEND_RAW_ACCEL | IMU_MODE_SEND_RAW_GYRO;

/// Input report IDs
const REPORT_ID_STATE_USB: u8 = 0x42;
const REPORT_ID_STATE_BLE: u8 = 0x45;

/// IMU data offset in input report (after report ID)
const IMU_OFFSET_START: usize = 29;
const IMU_OFFSET_ACCEL_X: usize = IMU_OFFSET_START + 4;
const IMU_OFFSET_ACCEL_Y: usize = IMU_OFFSET_START + 6;
const IMU_OFFSET_ACCEL_Z: usize = IMU_OFFSET_START + 8;
const IMU_OFFSET_GYRO_X: usize = IMU_OFFSET_START + 10;
const IMU_OFFSET_GYRO_Y: usize = IMU_OFFSET_START + 12;
const IMU_OFFSET_GYRO_Z: usize = IMU_OFFSET_START + 14;

/// Sensor scale factors
const ACCEL_PER_G: f32 = 16384.0;
const GYRO_PER_DPS: f32 = 16.384;

const ANALOG_TRIGGER_TO_DIGITAL_THRESHOLD: u8 = 228;

// Button masks
const MASK_A: u32 = 0x0000_0001;
const MASK_B: u32 = 0x0000_0002;
const MASK_X: u32 = 0x0000_0004;
const MASK_Y: u32 = 0x0000_0008;
const MASK_QAM: u32 = 0x0000_0010;
const MASK_R3: u32 = 0x0000_0020;
const MASK_VIEW: u32 = 0x0000_0040;
const MASK_R: u32 = 0x0000_0200;
const MASK_DPAD_DOWN: u32 = 0x0000_0400;
const MASK_DPAD_RIGHT: u32 = 0x0000_0800;
const MASK_DPAD_LEFT: u32 = 0x0000_1000;
const MASK_DPAD_UP: u32 = 0x0000_2000;
const MASK_MENU: u32 = 0x0000_4000;
const MASK_L3: u32 = 0x0000_8000;
const MASK_STEAM: u32 = 0x0001_0000;
const MASK_L: u32 = 0x0008_0000;
const MASK_RIGHT_STICK_TOUCH: u32 = 0x0010_0000;
const MASK_RIGHT_PAD_TOUCH: u32 = 0x0020_0000;
const MASK_LEFT_STICK_TOUCH: u32 = 0x0100_0000;
const MASK_LEFT_PAD_TOUCH: u32 = 0x0200_0000;
const MASK_RIGHT_GRIP: u32 = 0x1000_0000;
const MASK_LEFT_GRIP: u32 = 0x2000_0000;
const MASK_L4: u32 = 0x0002_0000;
const MASK_L5: u32 = 0x0004_0000;
const MASK_R4: u32 = 0x0000_0080;
const MASK_R5: u32 = 0x0000_0100;

const READ_TIMEOUT_MILLIS: i32 = 100;

/// Parsed Triton frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TritonFrame {
    pub buttons: u32,
    pub trigger_left: u16,
    pub trigger_right: u16,
    pub left_stick_x: i16,
    pub left_stick_y: i16,
    pub right_stick_x: i16,
    pub right_stick_y: i16,
    pub imu_timestamp: u32,
    pub accel_x: i16,
    pub accel_y: i16,
    pub accel_z: i16,
    pub gyro_x: i16,
    pub gyro_y: i16,
    pub gyro_z: i16,

    // the remaining are not for DSU, only for gyro toggle support.
    pub left_stick_touch: bool,
    pub right_stick_touch: bool,
    pub left_pad_touch: bool,
    pub right_pad_touch: bool,
    pub left_grip: bool,
    pub right_grip: bool,
}

impl TritonFrame {
    /// Parse a raw HID report. Works for both USB (0x42) and BLE (0x45) report IDs.
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        let report_id = data[0];
        if report_id != REPORT_ID_STATE_USB && report_id != REPORT_ID_STATE_BLE {
            return None;
        }

        // Need at least: 1 report ID + 29 bytes to IMU + 16 bytes IMU data
        if data.len() < 1 + IMU_OFFSET_START + 16 {
            return None;
        }

        let p = &data[1..];
        let buttons = u32::from_le_bytes([p[1], p[2], p[3], p[4]]);

        Some(Self {
            buttons,
            trigger_left: u16::from_le_bytes([p[5], p[6]]),
            trigger_right: u16::from_le_bytes([p[7], p[8]]),
            left_stick_x: i16::from_le_bytes([p[9], p[10]]),
            left_stick_y: i16::from_le_bytes([p[11], p[12]]),
            right_stick_x: i16::from_le_bytes([p[13], p[14]]),
            right_stick_y: i16::from_le_bytes([p[15], p[16]]),
            imu_timestamp: u32::from_le_bytes([
                p[IMU_OFFSET_START],
                p[IMU_OFFSET_START + 1],
                p[IMU_OFFSET_START + 2],
                p[IMU_OFFSET_START + 3],
            ]),
            accel_x: i16::from_le_bytes([p[IMU_OFFSET_ACCEL_X], p[IMU_OFFSET_ACCEL_X + 1]]),
            accel_y: i16::from_le_bytes([p[IMU_OFFSET_ACCEL_Y], p[IMU_OFFSET_ACCEL_Y + 1]]),
            accel_z: i16::from_le_bytes([p[IMU_OFFSET_ACCEL_Z], p[IMU_OFFSET_ACCEL_Z + 1]]),
            gyro_x: i16::from_le_bytes([p[IMU_OFFSET_GYRO_X], p[IMU_OFFSET_GYRO_X + 1]]),
            gyro_y: i16::from_le_bytes([p[IMU_OFFSET_GYRO_Y], p[IMU_OFFSET_GYRO_Y + 1]]),
            gyro_z: i16::from_le_bytes([p[IMU_OFFSET_GYRO_Z], p[IMU_OFFSET_GYRO_Z + 1]]),
            left_stick_touch: (buttons & MASK_LEFT_STICK_TOUCH) != 0,
            right_stick_touch: (buttons & MASK_RIGHT_STICK_TOUCH) != 0,
            left_pad_touch: (buttons & MASK_LEFT_PAD_TOUCH) != 0,
            right_pad_touch: (buttons & MASK_RIGHT_PAD_TOUCH) != 0,
            left_grip: (buttons & MASK_LEFT_GRIP) != 0,
            right_grip: (buttons & MASK_RIGHT_GRIP) != 0,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionMode {
    Usb,
    UsbPuck,
    Bluetooth,
}

/// Triton (Steam Controller 2026) device
pub struct Triton {
    config: DeviceConfig,
    hid: HidDevice,
}

impl Triton {
    /// Enumerate all vendor interfaces and return the first Triton found.
    ///
    /// The created Triton will use `config` as its ['DeviceConfiguration'](crate::devices::DeviceConfiguration).
    /// Requires passing an `api` ([`HidApi`](hidapi::HidApi)) and optionally a specific `device_path`
    pub fn find(
        config: DeviceConfig,
        api: &HidApi,
        device_path: Option<&str>,
    ) -> Result<Self, DeviceError> {
        let candidates: Vec<_> = api
            .device_list()
            .filter(|d| {
                log::debug!(
                    "Considering VID {:04x}, PID {:04x}, Usage page {:04x}",
                    d.vendor_id(),
                    d.product_id(),
                    d.usage_page()
                );
                d.vendor_id() == VID
                    && d.usage_page() >= USAGE_PAGE_VENDOR_MIN
                    && (d.product_id() == PID_PUCK
                        || d.product_id() == PID_WIRED
                        || d.product_id() == PID_BT)
            })
            .collect();

        log::debug!("Found {} candidate vendor interfaces", candidates.len());

        if let Some(target) = device_path {
            let info = candidates
                .into_iter()
                .find(|d| d.path().to_str().ok() == Some(target));

            let Some(info) = info else {
                return Err(DeviceError::NoDeviceFoundAtPath(target.to_string()));
            };

            let pid = info.product_id();
            let hid = info.open_device(api)?;
            let mode = connection_mode_from_pid(pid);

            probe_device(&hid)?;

            log::info!("Opened controller on {} ({:?})", target, mode);
            return Ok(Triton { config, hid });
        }

        for info in candidates {
            let Ok(path) = info.path().to_str() else {
                log::debug!("Skipping device, could not get a path: {info:?}");
                continue;
            };

            log::debug!("Trying interface at {}", path);

            let pid = info.product_id();
            let hid = match info.open_device(api) {
                Ok(hid) => hid,
                Err(err) => {
                    log::debug!("Failed to obtain handle to device at {path}: {err:?}");
                    continue;
                }
            };

            let mode = connection_mode_from_pid(pid);

            if let Err(e) = probe_device(&hid) {
                log::debug!("Probe failed for device at {path}: {e}");
                continue;
            }

            log::info!("Opened controller on {} ({:?})", path, mode);
            return Ok(Triton { config, hid });
        }

        Err(DeviceError::NoDeviceFound)
    }
}

impl Device for Triton {
    fn initialize(&self) -> Result<(), DeviceError> {
        log::debug!("Sending IMU enable sequence... ");
        send_setting(&self.hid, SETTING_LIZARD_MODE, LIZARD_MODE_OFF)?;

        std::thread::sleep(SEND_FEATURE_REPORT_SLEEP_DURATION);

        send_setting(&self.hid, SETTING_IMU_MODE, IMU_MODE_GYRO_ACCEL)?;

        log::debug!("IMU enable sequence complete");

        Ok(())
    }

    fn read_frame(&self) -> Result<DSUFrame, DeviceError> {
        let mut buf = [0u8; 64];
        let n = self.hid.read_timeout(&mut buf, READ_TIMEOUT_MILLIS)?;

        if n == 0 {
            return Err(DeviceError::ShortRead(0, 1));
        }

        let frame = TritonFrame::parse(&buf[..n]).ok_or(DeviceError::InvalidReport(buf[0]))?;

        let inputs = &self.config.gyro_activation_inputs;
        let mut enable_gyro = true;

        // gyro toggling
        if !inputs.is_empty() {
            enable_gyro = match self.config.gyro_activation_mode {
                GyroActivationMode::Any => inputs
                    .iter()
                    .any(|button| self.is_device_button_pressed(button, &frame)),

                GyroActivationMode::All => inputs
                    .iter()
                    .all(|button| self.is_device_button_pressed(button, &frame)),
            };
        }

        log::trace!("Parsed TritonFrame: {:?}", frame);

        Ok(self.to_dsu_frame(&frame, !enable_gyro))
    }
}

impl FrameDevice<TritonFrame> for Triton {
    fn to_dsu_frame(&self, frame: &TritonFrame, gyro_disabled: bool) -> DSUFrame {
        let l2 = scale_trigger_to_byte(frame.trigger_left as i16);
        let r2 = scale_trigger_to_byte(frame.trigger_right as i16);

        let gyro_x_dps = frame.gyro_x as f32 / GYRO_PER_DPS;
        let gyro_y_dps = -(frame.gyro_z as f32 / GYRO_PER_DPS);
        let gyro_z_dps = frame.gyro_y as f32 / GYRO_PER_DPS;

        let apply_deadzone = |v: f32| {
            if v.abs() < self.config.gyro_deadzone {
                0.0
            } else {
                v
            }
        };

        let zero_on_gyro_disabled = |v: f32| {
            if gyro_disabled { 0.0 } else { v }
        };

        DSUFrame {
            dpad_left: is_u32_masked_button_pressed(frame.buttons, MASK_DPAD_LEFT),
            dpad_down: is_u32_masked_button_pressed(frame.buttons, MASK_DPAD_DOWN),
            dpad_right: is_u32_masked_button_pressed(frame.buttons, MASK_DPAD_RIGHT),
            dpad_up: is_u32_masked_button_pressed(frame.buttons, MASK_DPAD_UP),
            options: is_u32_masked_button_pressed(frame.buttons, MASK_VIEW),
            r3: is_u32_masked_button_pressed(frame.buttons, MASK_R3),
            l3: is_u32_masked_button_pressed(frame.buttons, MASK_L3),
            share: is_u32_masked_button_pressed(frame.buttons, MASK_MENU),
            y: is_u32_masked_button_pressed(frame.buttons, MASK_Y),
            b: is_u32_masked_button_pressed(frame.buttons, MASK_B),
            a: is_u32_masked_button_pressed(frame.buttons, MASK_A),
            x: is_u32_masked_button_pressed(frame.buttons, MASK_X),
            r1: is_u32_masked_button_pressed(frame.buttons, MASK_R),
            l1: is_u32_masked_button_pressed(frame.buttons, MASK_L),
            r2: r2 >= ANALOG_TRIGGER_TO_DIGITAL_THRESHOLD,
            l2: l2 >= ANALOG_TRIGGER_TO_DIGITAL_THRESHOLD,
            home: is_u32_masked_button_pressed(frame.buttons, MASK_STEAM),
            touch: is_u32_masked_button_pressed(frame.buttons, MASK_QAM),
            left_stick_x: scale_stick_to_byte(frame.left_stick_x),
            left_stick_y: scale_stick_to_byte(frame.left_stick_y),
            right_stick_x: scale_stick_to_byte(frame.right_stick_x),
            right_stick_y: scale_stick_to_byte(frame.right_stick_y),
            analog_r2: r2,
            analog_l2: l2,
            raw_accel_x: frame.accel_x as f32,
            raw_accel_y: frame.accel_y as f32,
            raw_accel_z: frame.accel_z as f32,
            raw_gyro_x: frame.gyro_x as f32,
            raw_gyro_y: frame.gyro_y as f32,
            raw_gyro_z: frame.gyro_z as f32,
            accel_x: zero_on_gyro_disabled(-(frame.accel_x as f32 / ACCEL_PER_G)),
            accel_y: zero_on_gyro_disabled(-(frame.accel_z as f32 / ACCEL_PER_G)),
            accel_z: zero_on_gyro_disabled(frame.accel_y as f32 / ACCEL_PER_G),
            gyro_x: zero_on_gyro_disabled(
                apply_deadzone(gyro_x_dps) * self.config.gyro_pitch_scale,
            ),
            gyro_y: zero_on_gyro_disabled(apply_deadzone(gyro_y_dps) * self.config.gyro_yaw_scale),
            gyro_z: zero_on_gyro_disabled(apply_deadzone(gyro_z_dps) * self.config.gyro_roll_scale),
        }
    }

    fn is_device_button_pressed(&self, button: &DeviceButton, frame: &TritonFrame) -> bool {
        match button {
            DeviceButton::DpadLeft => is_u32_masked_button_pressed(frame.buttons, MASK_DPAD_LEFT),
            DeviceButton::DpadDown => is_u32_masked_button_pressed(frame.buttons, MASK_DPAD_DOWN),
            DeviceButton::DpadRight => is_u32_masked_button_pressed(frame.buttons, MASK_DPAD_RIGHT),
            DeviceButton::DpadUp => is_u32_masked_button_pressed(frame.buttons, MASK_DPAD_UP),
            DeviceButton::Start => is_u32_masked_button_pressed(frame.buttons, MASK_VIEW),
            DeviceButton::Select => is_u32_masked_button_pressed(frame.buttons, MASK_MENU),
            DeviceButton::Guide => is_u32_masked_button_pressed(frame.buttons, MASK_STEAM),
            DeviceButton::Quaternary => is_u32_masked_button_pressed(frame.buttons, MASK_QAM),
            DeviceButton::A => is_u32_masked_button_pressed(frame.buttons, MASK_A),
            DeviceButton::B => is_u32_masked_button_pressed(frame.buttons, MASK_B),
            DeviceButton::X => is_u32_masked_button_pressed(frame.buttons, MASK_X),
            DeviceButton::Y => is_u32_masked_button_pressed(frame.buttons, MASK_Y),
            DeviceButton::L1 => is_u32_masked_button_pressed(frame.buttons, MASK_L),
            DeviceButton::R1 => is_u32_masked_button_pressed(frame.buttons, MASK_R),
            DeviceButton::L2 => {
                scale_trigger_to_byte(frame.trigger_left as i16)
                    >= ANALOG_TRIGGER_TO_DIGITAL_THRESHOLD
            }
            DeviceButton::R2 => {
                scale_trigger_to_byte(frame.trigger_right as i16)
                    >= ANALOG_TRIGGER_TO_DIGITAL_THRESHOLD
            }
            DeviceButton::L3 => is_u32_masked_button_pressed(frame.buttons, MASK_L3),
            DeviceButton::R3 => is_u32_masked_button_pressed(frame.buttons, MASK_R3),
            DeviceButton::L4 => is_u32_masked_button_pressed(frame.buttons, MASK_L4),
            DeviceButton::L5 => is_u32_masked_button_pressed(frame.buttons, MASK_L5),
            DeviceButton::R4 => is_u32_masked_button_pressed(frame.buttons, MASK_R4),
            DeviceButton::R5 => is_u32_masked_button_pressed(frame.buttons, MASK_R5),
            DeviceButton::LeftStickTouch => frame.left_stick_touch,
            DeviceButton::RightStickTouch => frame.right_stick_touch,
            DeviceButton::LeftPadTouch => frame.left_pad_touch,
            DeviceButton::RightPadTouch => frame.right_pad_touch,
            DeviceButton::LeftGrip => frame.left_grip,
            DeviceButton::RightGrip => frame.right_grip,
            DeviceButton::Unknown => false,
        }
    }
}

impl Drop for Triton {
    fn drop(&mut self) {
        if !self.config.no_enable_lizard_mode_on_close
            && send_setting(&self.hid, SETTING_LIZARD_MODE, LIZARD_MODE_ON).is_ok()
        {
            log::debug!("Re-enabled lizard mode");
        }
    }
}

/// Send a single setting value using hidapi.
fn send_setting(hid: &HidDevice, setting: u8, value: u16) -> Result<(), DeviceError> {
    let mut buf = [0u8; FEATURE_REPORT_SIZE];
    buf[0] = FEATURE_REPORT_ID;
    buf[1] = CMD_SET_SETTINGS_VALUES;
    buf[2] = 3;
    buf[3] = setting;
    buf[4] = (value & 0xFF) as u8;
    buf[5] = ((value >> 8) & 0xFF) as u8;

    hid.send_feature_report(&buf)?;
    Ok(())
}

fn connection_mode_from_pid(pid: u16) -> ConnectionMode {
    match pid {
        PID_BT => ConnectionMode::Bluetooth,
        PID_WIRED => ConnectionMode::Usb,
        PID_PUCK => ConnectionMode::UsbPuck,

        // todo: this is stupid
        _ => ConnectionMode::Usb,
    }
}

/// Probe a device to verify it's responsive.
fn probe_device(hid: &HidDevice) -> Result<(), DeviceError> {
    let mut probe = [0u8; FEATURE_REPORT_SIZE];
    probe[0] = FEATURE_REPORT_ID;
    probe[1] = CMD_SET_SETTINGS_VALUES;
    probe[2] = 3;
    probe[3] = SETTING_LIZARD_MODE;
    probe[4] = 0;
    probe[5] = 0;
    hid.send_feature_report(&probe)?;
    Ok(())
}
