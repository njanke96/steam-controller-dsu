use crate::dsu::DSUFrame;
use crate::errors::DeviceError;

/// A trait defining shared behavior between compatible devices.
pub trait Device {
    /// Run any initialization logic the device requires.
    fn initialize(&self) -> Result<(), DeviceError>;

    /// Read a DSU frame from the device.
    fn read_frame(&self) -> Result<DSUFrame, DeviceError>;
}

/// Common device configurations
#[derive(Default, Debug, Clone)]
pub struct DeviceConfig {
    /// Don't enable lizard mode when the device is dropped.
    pub no_enable_lizard_mode_on_close: bool,
}
