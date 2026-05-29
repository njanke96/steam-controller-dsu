//! Contains adapters from raw HID data to DSU frames for supported devices.
//!
//! All devices implement the [`Device`](crate::devices::Device) trait.

// Triton Steam Controller (2026)
pub mod legacy;
/// Legacy Steam Controller (2015)
pub mod triton;
pub(crate) mod util;

mod device;

pub use device::Device;
pub use device::DeviceButton;
pub use device::DeviceConfig;
pub use device::DeviceFamily;
pub use device::FrameDevice;
pub use device::GyroActivationMode;
