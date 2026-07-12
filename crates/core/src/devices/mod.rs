//! Contains adapters from raw HID data to DSU frames for supported devices.
//!
//! All devices implement the [`Device`](crate::devices::Device) trait.

pub mod legacy;
pub mod triton;
pub(crate) mod util;

mod device;

pub use device::DEFAULT_ACCEL_SMOOTHING_ALPHA;
pub use device::DEFAULT_GYRO_SMOOTHING_ALPHA;
pub use device::Device;
pub use device::DeviceButton;
pub use device::DeviceConfig;
pub use device::DeviceFamily;
pub use device::DeviceSmoothingAlphas;
pub use device::FrameDevice;
pub use device::GyroActivationMode;
