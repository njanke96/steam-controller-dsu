use crate::dsu::DSUFrame;
use crate::errors::DeviceError;

pub trait Device {
    /// Initialize the device
    fn initialize(&self) -> Result<(), DeviceError>;

    /// Read a DSU frame from the device
    fn read_frame(&self) -> Result<DSUFrame, DeviceError>;
}
