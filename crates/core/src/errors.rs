use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Address `{0}` is not valid")]
    InvalidAddress(String),
    #[error("could not initialize HID API: {0}")]
    HidApi(#[from] hidapi::HidError),
}

#[derive(Error, Debug)]
pub enum DeviceError {
    #[error("Failed to open Device")]
    NoDeviceFound,
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
    #[error("HID Error: {0}")]
    Hid(#[from] hidapi::HidError),
}
