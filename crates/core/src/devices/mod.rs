//! This module contains adapters from raw HID data to DSU frames for supported devices.
//!
//! All devices implement the [`Device`](crate::devices::device::Device) trait, and their frame
//! data implement `From<SomeDeviceSpecifcFrameData> for DSUFrame`.

pub mod device;
pub mod triton;
pub(crate) mod util;
