/// Input report ID for the Triton full-state packet.
pub const REPORT_ID_TRITON_FULL: u8 = 0x42;

/// Total HID report length (including Report ID).
pub const REPORT_SIZE: usize = 54;

/// Sensor scale factors (same as Steam Deck).
pub const ACCEL_PER_G: f32 = 16384.0;
pub const GYRO_PER_DPS: f32 = 16.0;

/// Parsed Triton full-state frame (Report ID 0x42).
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
    pub const REPORT_ID: u8 = REPORT_ID_TRITON_FULL;
    pub const REPORT_SIZE: usize = REPORT_SIZE;

    /// Parse a raw HID report.  `data` must include the Report ID byte.
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < Self::REPORT_SIZE || data[0] != Self::REPORT_ID {
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

    /// Accelerometer in g (same scale as Steam Deck).
    pub fn accel_g(&self) -> (f32, f32, f32) {
        (
            self.accel_x as f32 / ACCEL_PER_G,
            self.accel_y as f32 / ACCEL_PER_G,
            self.accel_z as f32 / ACCEL_PER_G,
        )
    }

    /// Gyroscope in degrees per second (same scale as Steam Deck).
    pub fn gyro_dps(&self) -> (f32, f32, f32) {
        (
            self.gyro_x as f32 / GYRO_PER_DPS,
            self.gyro_y as f32 / GYRO_PER_DPS,
            self.gyro_z as f32 / GYRO_PER_DPS,
        )
    }

    /// Returns true if IMU data (accel + gyro) matches another frame exactly.
    /// Used to detect frozen/stale sensor data.
    pub fn imu_eq(&self, other: &Self) -> bool {
        self.accel_x == other.accel_x
            && self.accel_y == other.accel_y
            && self.accel_z == other.accel_z
            && self.gyro_x == other.gyro_x
            && self.gyro_y == other.gyro_y
            && self.gyro_z == other.gyro_z
    }
}
