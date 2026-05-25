//! Adapter for the original Steam Controller family.

use hidapi::{HidApi, HidDevice};
use std::time::Duration;

use crate::devices::device::Device;
use crate::devices::util::{is_u32_masked_button_pressed, scale_stick_to_byte, scale_trigger_to_byte};
use crate::devices::{DeviceButton, DeviceConfig, GyroActivationMode};
use crate::dsu::DSUFrame;
use crate::errors::DeviceError;

const VID: u16 = 0x28de;
const PID_WIRED: u16 = 0x1102;
const PID_WIRELESS: u16 = 0x1142;

const USAGE_PAGE_VENDOR_MIN: u16 = 0xFF00;

const FEATURE_REPORT_ID: u8 = 0x01;
const FEATURE_REPORT_SIZE: usize = 64;
const SEND_FEATURE_REPORT_SLEEP_DURATION: Duration = Duration::from_millis(50);
const CMD_SET_SETTINGS_VALUES: u8 = 0x87;

const SETTING_LIZARD_MODE: u8 = 9;
const SETTING_IMU_MODE: u8 = 48;

const LIZARD_MODE_OFF: u16 = 0;
const LIZARD_MODE_ON: u16 = 1;
const IMU_MODE_SEND_RAW_ACCEL: u16 = 0x08;
const IMU_MODE_SEND_RAW_GYRO: u16 = 0x10;
const IMU_MODE_GYRO_ACCEL: u16 = IMU_MODE_SEND_RAW_ACCEL | IMU_MODE_SEND_RAW_GYRO;

const REPORT_ID_STATE: u8 = 0x01;
const READ_TIMEOUT_MILLIS: i32 = 100;

const ACCEL_PER_G: f32 = 16384.0;
const GYRO_PER_DPS: f32 = 16.384;
const ANALOG_TRIGGER_TO_DIGITAL_THRESHOLD: u8 = 228;

const MASK_A: u64 = 0x0000_0000_0000_0080;
const MASK_B: u64 = 0x0000_0000_0000_0020;
const MASK_X: u64 = 0x0000_0000_0000_0040;
const MASK_Y: u64 = 0x0000_0000_0000_0010;
const MASK_RIGHT_BUMPER: u64 = 0x0000_0000_0000_0004;
const MASK_LEFT_BUMPER: u64 = 0x0000_0000_0000_0008;
const MASK_DPAD_UP: u64 = 0x0000_0000_0000_0100;
const MASK_DPAD_RIGHT: u64 = 0x0000_0000_0000_0200;
const MASK_DPAD_LEFT: u64 = 0x0000_0000_0000_0400;
const MASK_DPAD_DOWN: u64 = 0x0000_0000_0000_0800;
const MASK_SELECT: u64 = 0x0000_0000_0000_1000;
const MASK_GUIDE: u64 = 0x0000_0000_0000_2000;
const MASK_START: u64 = 0x0000_0000_0000_4000;
const MASK_LEFT_GRIP: u64 = 0x0000_0000_0000_8000;
const MASK_RIGHT_GRIP: u64 = 0x0000_0000_0001_0000;
const MASK_RIGHT_PAD_CLICKED: u64 = 0x0000_0000_0004_0000;
const MASK_LEFT_PAD_TOUCH: u64 = 0x0000_0000_0008_0000;
const MASK_RIGHT_PAD_TOUCH: u64 = 0x0000_0000_0010_0000;
const MASK_LEFT_STICK_CLICK: u64 = 0x0000_0000_0040_0000;
const MASK_LEFT_PAD_AND_JOYSTICK: u64 = 0x0000_0000_0080_0000;

#[derive(Debug, Clone, Copy, PartialEq)]
struct LegacyFrame {
    buttons: u64,
    trigger_left: u8,
    trigger_right: u8,
    left_stick_x: i16,
    left_stick_y: i16,
    right_pad_x: i16,
    right_pad_y: i16,
    imu_timestamp: u32,
    accel_x: i16,
    accel_y: i16,
    accel_z: i16,
    gyro_x: i16,
    gyro_y: i16,
    gyro_z: i16,
    left_pad_touch: bool,
    right_pad_touch: bool,
    left_stick_click: bool,
    right_pad_click: bool,
    left_grip: bool,
    right_grip: bool,
}

impl LegacyFrame {
    fn parse(data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        let report_id = data[0];
        if report_id != REPORT_ID_STATE {
            return None;
        }

        if data.len() < 64 {
            return None;
        }

        let buttons = u64::from_le_bytes([
            data[8], data[9], data[10], 0, 0, 0, 0, 0,
        ]);

        Some(Self {
            buttons,
            trigger_left: data[11],
            trigger_right: data[12],
            left_stick_x: i16::from_le_bytes([data[54], data[55]]),
            left_stick_y: i16::from_le_bytes([data[56], data[57]]),
            right_pad_x: i16::from_le_bytes([data[20], data[21]]),
            right_pad_y: i16::from_le_bytes([data[22], data[23]]),
            imu_timestamp: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
            accel_x: i16::from_le_bytes([data[28], data[29]]),
            accel_y: i16::from_le_bytes([data[30], data[31]]),
            accel_z: i16::from_le_bytes([data[32], data[33]]),
            gyro_x: i16::from_le_bytes([data[34], data[35]]),
            gyro_y: i16::from_le_bytes([data[36], data[37]]),
            gyro_z: i16::from_le_bytes([data[38], data[39]]),
            left_pad_touch: is_u32_masked_button_pressed(buttons as u32, MASK_LEFT_PAD_TOUCH as u32)
                || is_u32_masked_button_pressed(buttons as u32, MASK_LEFT_PAD_AND_JOYSTICK as u32),
            right_pad_touch: is_u32_masked_button_pressed(buttons as u32, MASK_RIGHT_PAD_TOUCH as u32),
            left_stick_click: is_u32_masked_button_pressed(buttons as u32, MASK_LEFT_STICK_CLICK as u32),
            right_pad_click: is_u32_masked_button_pressed(buttons as u32, MASK_RIGHT_PAD_CLICKED as u32),
            left_grip: is_u32_masked_button_pressed(buttons as u32, MASK_LEFT_GRIP as u32),
            right_grip: is_u32_masked_button_pressed(buttons as u32, MASK_RIGHT_GRIP as u32),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionMode {
    Usb,
    Wireless,
}

/// Original Steam Controller family.
pub struct LegacySteamController {
    config: DeviceConfig,
    hid: HidDevice,
}

impl LegacySteamController {
    /// Enumerate legacy Steam Controller interfaces and return the first supported device.
    pub fn find(
        config: DeviceConfig,
        api: &HidApi,
        device_path: Option<&str>,
    ) -> Result<Self, DeviceError> {
        let candidates: Vec<_> = api
            .device_list()
            .filter(|d| {
                d.vendor_id() == VID
                    && d.usage_page() >= USAGE_PAGE_VENDOR_MIN
                    && (d.product_id() == PID_WIRED || d.product_id() == PID_WIRELESS)
                    && Self::supports_interface(d.interface_number())
            })
            .collect();

        if let Some(target) = device_path {
            let info = candidates
                .into_iter()
                .find(|d| d.path().to_str().ok() == Some(target));

            let Some(info) = info else {
                return Err(DeviceError::NoDeviceFoundAtPath(target.to_string()));
            };

            let hid = info.open_device(api)?;
            probe_device(&hid)?;
            log::info!("Opened legacy controller on {} ({:?})", target, connection_mode_from_pid(info.product_id()));
            return Ok(Self { config, hid });
        }

        for info in candidates {
            let Ok(path) = info.path().to_str() else {
                continue;
            };

            let hid = match info.open_device(api) {
                Ok(hid) => hid,
                Err(err) => {
                    log::debug!("Failed to obtain handle to legacy device at {path}: {err:?}");
                    continue;
                }
            };

            if let Err(e) = probe_device(&hid) {
                log::debug!("Probe failed for legacy device at {path}: {e}");
                continue;
            }

            log::info!("Opened legacy controller on {} ({:?})", path, connection_mode_from_pid(info.product_id()));
            return Ok(Self { config, hid });
        }

        Err(DeviceError::NoDeviceFound)
    }

    fn supports_interface(interface_number: i32) -> bool {
        interface_number == 2 || (1..=4).contains(&interface_number)
    }

    fn initialize_impl(&self) -> Result<(), DeviceError> {
        send_setting(&self.hid, SETTING_LIZARD_MODE, LIZARD_MODE_OFF)?;
        std::thread::sleep(SEND_FEATURE_REPORT_SLEEP_DURATION);
        send_setting(&self.hid, SETTING_IMU_MODE, IMU_MODE_GYRO_ACCEL)?;
        Ok(())
    }

    fn to_dsu_frame(&self, frame: &LegacyFrame, gyro_disabled: bool) -> DSUFrame {
        let l2 = scale_trigger_to_byte(((frame.trigger_left as u16) << 7 | frame.trigger_left as u16) as i16);
        let r2 = scale_trigger_to_byte(((frame.trigger_right as u16) << 7 | frame.trigger_right as u16) as i16);

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
            dpad_left: is_u32_masked_button_pressed(frame.buttons as u32, MASK_DPAD_LEFT as u32),
            dpad_down: is_u32_masked_button_pressed(frame.buttons as u32, MASK_DPAD_DOWN as u32),
            dpad_right: is_u32_masked_button_pressed(frame.buttons as u32, MASK_DPAD_RIGHT as u32),
            dpad_up: is_u32_masked_button_pressed(frame.buttons as u32, MASK_DPAD_UP as u32),
            options: is_u32_masked_button_pressed(frame.buttons as u32, MASK_START as u32),
            r3: frame.right_pad_click,
            l3: frame.left_stick_click,
            share: is_u32_masked_button_pressed(frame.buttons as u32, MASK_SELECT as u32),
            y: is_u32_masked_button_pressed(frame.buttons as u32, MASK_Y as u32),
            b: is_u32_masked_button_pressed(frame.buttons as u32, MASK_B as u32),
            a: is_u32_masked_button_pressed(frame.buttons as u32, MASK_A as u32),
            x: is_u32_masked_button_pressed(frame.buttons as u32, MASK_X as u32),
            r1: is_u32_masked_button_pressed(frame.buttons as u32, MASK_RIGHT_BUMPER as u32),
            l1: is_u32_masked_button_pressed(frame.buttons as u32, MASK_LEFT_BUMPER as u32),
            r2: r2 >= ANALOG_TRIGGER_TO_DIGITAL_THRESHOLD,
            l2: l2 >= ANALOG_TRIGGER_TO_DIGITAL_THRESHOLD,
            home: is_u32_masked_button_pressed(frame.buttons as u32, MASK_GUIDE as u32),
            touch: frame.left_pad_touch || frame.right_pad_touch,
            left_stick_x: scale_stick_to_byte(frame.left_stick_x),
            left_stick_y: scale_stick_to_byte(frame.left_stick_y),
            right_stick_x: scale_stick_to_byte(frame.right_pad_x),
            right_stick_y: scale_stick_to_byte(frame.right_pad_y),
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
            gyro_x: zero_on_gyro_disabled(apply_deadzone(gyro_x_dps) * self.config.gyro_pitch_scale),
            gyro_y: zero_on_gyro_disabled(apply_deadzone(gyro_y_dps) * self.config.gyro_yaw_scale),
            gyro_z: zero_on_gyro_disabled(apply_deadzone(gyro_z_dps) * self.config.gyro_roll_scale),
        }
    }

    fn is_device_button_pressed(&self, button: &DeviceButton, frame: &LegacyFrame) -> bool {
        match button {
            DeviceButton::DpadLeft => is_u32_masked_button_pressed(frame.buttons as u32, MASK_DPAD_LEFT as u32),
            DeviceButton::DpadDown => is_u32_masked_button_pressed(frame.buttons as u32, MASK_DPAD_DOWN as u32),
            DeviceButton::DpadRight => is_u32_masked_button_pressed(frame.buttons as u32, MASK_DPAD_RIGHT as u32),
            DeviceButton::DpadUp => is_u32_masked_button_pressed(frame.buttons as u32, MASK_DPAD_UP as u32),
            DeviceButton::Start => is_u32_masked_button_pressed(frame.buttons as u32, MASK_START as u32),
            DeviceButton::Select => is_u32_masked_button_pressed(frame.buttons as u32, MASK_SELECT as u32),
            DeviceButton::Guide => is_u32_masked_button_pressed(frame.buttons as u32, MASK_GUIDE as u32),
            DeviceButton::Quaternary => false,
            DeviceButton::A => is_u32_masked_button_pressed(frame.buttons as u32, MASK_A as u32),
            DeviceButton::B => is_u32_masked_button_pressed(frame.buttons as u32, MASK_B as u32),
            DeviceButton::X => is_u32_masked_button_pressed(frame.buttons as u32, MASK_X as u32),
            DeviceButton::Y => is_u32_masked_button_pressed(frame.buttons as u32, MASK_Y as u32),
            DeviceButton::L1 => is_u32_masked_button_pressed(frame.buttons as u32, MASK_LEFT_BUMPER as u32),
            DeviceButton::R1 => is_u32_masked_button_pressed(frame.buttons as u32, MASK_RIGHT_BUMPER as u32),
            DeviceButton::L2 => scale_trigger_to_byte(((frame.trigger_left as u16) << 7 | frame.trigger_left as u16) as i16)
                >= ANALOG_TRIGGER_TO_DIGITAL_THRESHOLD,
            DeviceButton::R2 => scale_trigger_to_byte(((frame.trigger_right as u16) << 7 | frame.trigger_right as u16) as i16)
                >= ANALOG_TRIGGER_TO_DIGITAL_THRESHOLD,
            DeviceButton::L3 => frame.left_stick_click,
            DeviceButton::R3 => frame.right_pad_click,
            DeviceButton::L4 => false,
            DeviceButton::L5 => false,
            DeviceButton::R4 => false,
            DeviceButton::R5 => false,
            DeviceButton::LeftStickTouch => false,
            DeviceButton::RightStickTouch => false,
            DeviceButton::LeftPadTouch => frame.left_pad_touch,
            DeviceButton::RightPadTouch => frame.right_pad_touch,
            DeviceButton::LeftGrip => frame.left_grip,
            DeviceButton::RightGrip => frame.right_grip,
            DeviceButton::Unknown => false,
        }
    }
}

impl Device for LegacySteamController {
    fn initialize(&self) -> Result<(), DeviceError> {
        self.initialize_impl()
    }

    fn read_frame(&self) -> Result<DSUFrame, DeviceError> {
        let mut buf = [0u8; 64];
        let n = self.hid.read_timeout(&mut buf, READ_TIMEOUT_MILLIS)?;

        if n == 0 {
            return Err(DeviceError::ShortRead(0, 1));
        }

        let frame = LegacyFrame::parse(&buf[..n]).ok_or(DeviceError::InvalidReport(buf[0]))?;

        let inputs = &self.config.gyro_activation_inputs;
        let mut enable_gyro = true;

        if !inputs.is_empty() {
            enable_gyro = match self.config.gyro_activation_mode {
                GyroActivationMode::Any => inputs.iter().any(|button| self.is_device_button_pressed(button, &frame)),
                GyroActivationMode::All => inputs.iter().all(|button| self.is_device_button_pressed(button, &frame)),
            };
        }

        Ok(self.to_dsu_frame(&frame, !enable_gyro))
    }
}

impl Drop for LegacySteamController {
    fn drop(&mut self) {
        if !self.config.no_enable_lizard_mode_on_close
            && send_setting(&self.hid, SETTING_LIZARD_MODE, LIZARD_MODE_ON).is_ok()
        {
            log::debug!("Re-enabled lizard mode on legacy controller");
        }
    }
}

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
        PID_WIRELESS => ConnectionMode::Wireless,
        _ => ConnectionMode::Usb,
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_legacy_state_packet() {
        let mut data = [0u8; 64];
        data[0] = REPORT_ID_STATE;
        data[8] = 0x80;
        data[9] = 0x13;
        data[10] = 0x08;
        data[11] = 0x40;
        data[12] = 0x20;
        data[20] = 0x34;
        data[21] = 0x12;
        data[22] = 0x78;
        data[23] = 0x56;
        data[28] = 0x11;
        data[29] = 0x22;
        data[30] = 0x33;
        data[31] = 0x44;
        data[32] = 0x55;
        data[33] = 0x66;
        data[34] = 0x77;
        data[35] = 0x88;
        data[36] = 0x99;
        data[37] = 0xAA;
        data[38] = 0xBB;
        data[39] = 0xCC;
        data[54] = 0x01;
        data[55] = 0x02;
        data[56] = 0x03;
        data[57] = 0x04;

        let frame = LegacyFrame::parse(&data).expect("frame");
        assert_eq!(frame.trigger_left, 0x40);
        assert_eq!(frame.trigger_right, 0x20);
        assert_eq!(frame.left_stick_x, i16::from_le_bytes([0x01, 0x02]));
        assert_eq!(frame.right_pad_x, i16::from_le_bytes([0x34, 0x12]));
        assert!(frame.left_pad_touch);
    }
}