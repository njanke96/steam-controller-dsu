use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("could not initialize HID API: {0}")]
    HidApi(#[from] hidapi::HidError),
    #[error("failed to clone UdpSocket: {0}")]
    UdpSocketCloneFailed(std::io::Error),
    #[error("UdpSocket operation error: {0}")]
    UdpSocketOperationError(std::io::Error),
}

#[derive(Error, Debug)]
pub enum DeviceError {
    #[error("Failed to open Device")]
    NoDeviceFound,
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
    #[error("HID Error: {0}")]
    Hid(#[from] hidapi::HidError),
    #[error("Short read: got {0} bytes, expected at least {1}")]
    ShortRead(usize, usize),
    #[error("Non-Triton report (first byte: 0x{0:02x})")]
    NonTritonReport(u8),
}
