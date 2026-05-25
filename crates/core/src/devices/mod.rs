//! Contains adapters from raw HID data to DSU frames for supported devices.
//!
//! All devices implement the [`Device`](crate::devices::Device) trait and can be selected
//! through [`SupportedDevice`].

pub mod legacy;
pub mod triton;
pub(crate) mod util;

mod device;

use hidapi::HidApi;

use crate::errors::DeviceError;

pub use device::Device;
pub use device::DeviceButton;
pub use device::DeviceConfig;
pub use device::FrameDevice;
pub use device::GyroActivationMode;

/// Supported controller families.
pub enum SupportedDevice {
	Triton(triton::Triton),
	Legacy(legacy::LegacySteamController),
}

/// A specific Steam Controller family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceFamily {
	Triton,
	Legacy,
}

impl SupportedDevice {
	/// Find the first supported controller.
	pub fn find(
		config: DeviceConfig,
		api: &HidApi,
		device_path: Option<&str>,
	) -> Result<Self, DeviceError> {
		Self::find_family(DeviceFamily::Triton, config.clone(), api, device_path).or_else(|_| {
			Self::find_family(DeviceFamily::Legacy, config, api, device_path)
		})
	}

	/// Find a controller from the requested family.
	pub fn find_family(
		family: DeviceFamily,
		config: DeviceConfig,
		api: &HidApi,
		device_path: Option<&str>,
	) -> Result<Self, DeviceError> {
		match family {
			DeviceFamily::Triton => triton::Triton::find(config, api, device_path).map(Self::Triton),
			DeviceFamily::Legacy => {
				legacy::LegacySteamController::find(config, api, device_path).map(Self::Legacy)
			}
		}
	}
}

impl Device for SupportedDevice {
	fn initialize(&self) -> Result<(), DeviceError> {
		match self {
			Self::Triton(device) => device.initialize(),
			Self::Legacy(device) => device.initialize(),
		}
	}

	fn read_frame(&self) -> Result<crate::dsu::DSUFrame, DeviceError> {
		match self {
			Self::Triton(device) => device.read_frame(),
			Self::Legacy(device) => device.read_frame(),
		}
	}
}
