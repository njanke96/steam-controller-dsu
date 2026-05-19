use crate::errors::DeviceError;

pub trait Device {
    /// Initialize the device
    fn initialize(&self) -> Result<(), DeviceError>;

    /// Read a gyro frame from the device
    fn read_frame(&self) -> Result<GyroFrame, DeviceError>;
}

/// Represents a gyro frame with axes orientation and sign same as Triton (Steam Controller 2026)
/// Accel values in G, Gyro in degrees per second
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GyroFrame {
    pub accel_x: f32,
    pub accel_y: f32,
    pub accel_z: f32,
    pub gyro_x: f32,
    pub gyro_y: f32,
    pub gyro_z: f32,
}
