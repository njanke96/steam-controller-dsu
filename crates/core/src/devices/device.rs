use crate::{errors::DeviceError, frame::TritonFrame};

pub trait Device {
    // TODO: Read an agnostic frame struct
    /// Read a triton frame from the device
    fn read_triton_frame(&self) -> Result<TritonFrame, DeviceError>;

    // TODO: Abstract "load" function in trait, avoid calling device specific functions from entrypoints
}
