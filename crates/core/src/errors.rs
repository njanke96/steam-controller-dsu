//! Error types for the library.

use thiserror::Error;

/// Errors that can occur when running the UDP server.
#[derive(Error, Debug)]
pub enum ServerError {
    /// The HID Api failed to initialize when starting the server.
    #[error("could not initialize HID API: {0}")]
    HidApi(#[from] hidapi::HidError),
    /// The `UdpSocket` could not be cloned.
    #[error("failed to clone UdpSocket: {0}")]
    UdpSocketCloneFailed(std::io::Error),
    /// There was an error performing a `UdpSocket` operation.
    #[error("UdpSocket operation error: {0}")]
    UdpSocketOperationError(std::io::Error),
}

/// Errors that can occur when opening, initializing, or reading from devices.
#[derive(Error, Debug)]
pub enum DeviceError {
    /// The Device failed to open
    #[error("Failed to open Device")]
    NoDeviceFound,
    /// The device at path failed to open
    #[error("Failed to open specefied device: {0}")]
    NoDeviceFoundAtPath(String),
    /// A device operation resulted in a [`std::io::Error`]
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
    /// A device operation resulted in a [`hidapi::HidError`]
    #[error("HID Error: {0}")]
    Hid(#[from] hidapi::HidError),
    /// A device report was not the required length
    #[error("Short read: got {0} bytes, expected at least {1}")]
    ShortRead(usize, usize),
    /// A device report was invalid
    #[error("Invalid report (first byte: 0x{0:02x})")]
    InvalidReport(u8),
    /// Invalid device button string
    #[error("Invalid device button string: {0}")]
    InvalidDeviceButton(String),
    /// Invalid gyro activation mode string
    #[error("Invalid gyro activation mode string: {0}")]
    InvalidGyroActivationMode(String),
    /// Invalid device family string
    #[error("Invalid device family string: {0}")]
    InvalidDeviceFamily(String),
}
