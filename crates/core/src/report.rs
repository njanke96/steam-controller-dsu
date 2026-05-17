/// Steam Controller vendor/product IDs.
pub const VID: u16 = 0x28de;
pub const PID: u16 = 0x1304;

/// HID usage page for the vendor-defined gamepad interface.
pub const USAGE_PAGE_VENDOR: u16 = 0xFF00;

/// Input report ID for the Triton full-state packet.
pub const REPORT_ID_TRITON_FULL: u8 = 0x42;

/// Total HID report length (including Report ID).
pub const REPORT_SIZE: usize = 54;

/// Feature report command IDs (shared with Steam Deck / old controller).
pub mod commands {
    pub const CLEAR_DIGITAL_MAPPINGS: u8 = 0x81;
    pub const LOAD_DEFAULT_SETTINGS: u8 = 0x8E;
    pub const SET_SETTINGS_VALUES: u8 = 0x87;
}

/// Setting register IDs.
pub mod settings {
    pub const LEFT_TRACKPAD_MODE: u8 = 0x07;
    pub const RIGHT_TRACKPAD_MODE: u8 = 0x08;
    pub const IMU_MODE: u8 = 0x30;
}

/// Trackpad mode values.
pub mod trackpad_modes {
    pub const NONE: u16 = 0x07;
}

/// IMU mode bitflags.
pub mod imu_modes {
    pub const SEND_RAW_ACCEL: u16 = 0x08;
    pub const SEND_RAW_GYRO: u16 = 0x10;
}

/// Sensor scale factors (same as Steam Deck).
pub const ACCEL_PER_G: f32 = 16384.0;
pub const GYRO_PER_DPS: f32 = 16.0;
