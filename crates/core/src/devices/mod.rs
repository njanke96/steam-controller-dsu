//! Contains adapters from raw HID data to DSU frames for supported devices.
//!
//! All devices implement the [`Device`](crate::devices::Device) trait.

pub mod legacy;
pub mod triton;
pub(crate) mod util;

mod device;

use std::fmt;
use std::str::FromStr;

use crate::errors::DeviceError;

pub use device::Device;
pub use device::DeviceButton;
pub use device::DeviceConfig;
pub use device::FrameDevice;
pub use device::GyroActivationMode;

/// A specific Steam Controller family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceFamily {
	Triton,
	Legacy,
}

impl Default for DeviceFamily {
	fn default() -> Self {
		Self::Triton
	}
}


impl fmt::Display for DeviceFamily {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Triton => f.write_str("triton"),
			Self::Legacy => f.write_str("legacy"),
		}
	}
}


impl FromStr for DeviceFamily {
	type Err = DeviceError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"triton" => Ok(Self::Triton),
			"legacy" => Ok(Self::Legacy),
			_ => Err(DeviceError::InvalidDeviceFamily(s.to_string())),
		}
	}
}
