use crate::{errors::DeviceError, frame::TritonFrame};

pub trait Device {
    /// Read a triton frame from the device
    fn read_triton_frame(&self) -> Result<TritonFrame, DeviceError>;
}
