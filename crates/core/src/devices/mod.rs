//! Contains adapters from raw HID data to DSU frames for supported devices.
//!
//! All devices implement the [`Device`](crate::devices::Device) trait, and their frame
//! data implement `From<SomeDeviceSpecifcFrameData> for DSUFrame`.

pub mod triton;
pub(crate) mod util;

mod device;

pub use device::Device;
