use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use crate::errors::{DeviceError, ServerError};

pub mod errors;

pub(crate) mod device;
pub(crate) mod frame;
pub(crate) mod protocol;
pub(crate) mod reader;
pub(crate) mod report;
pub(crate) mod server;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Address or host to bind to
    pub bind_addr: String,
    // Port to listen on
    pub port: u16,
    /// Invert the yaxis values on the gyro and accelerometer
    pub invert_y: bool,
}

/// Run the server loop until receiving a signal
pub fn run_server(config: ServerConfig) -> Result<(), ServerError> {
    let address = format!("{}:{}", config.bind_addr, config.port);
    let addr = SocketAddr::from_str(&address).map_err(|_| ServerError::InvalidAddress(address))?;

    let mut api = hidapi::HidApi::new()?;

    loop {
        if let Err(e) = api.refresh_devices() {
            log::warn!("Failed to refresh HID device list: {e}");
        }
        let device = open_controller_with_retry(&api);

        log::info!("Controller opened. Enabling IMU...");
        if let Err(e) = device::enable_imu(&device.raw_file().lock().unwrap()) {
            log::error!("Failed to enable IMU: {e}");
            std::thread::sleep(Duration::from_secs(3));
            continue;
        }
        log::info!("IMU enabled. Starting CemuHook server on {} ...", addr);

        let (reader, rx) = reader::Reader::start(device.hid);

        if let Err(e) = server::Server::run(rx, &config) {
            log::error!("Server error: {e}");
        }

        reader.join();
        log::info!("Server shut down. Waiting 3 seconds before reconnect...");
        std::thread::sleep(Duration::from_secs(3));
    }
}

/// Run the debug loop.
/// Attempts to open the controller and dump frames.
pub fn run_debug_dump() -> Result<(), DeviceError> {
    let api = hidapi::HidApi::new()?;

    let device = device::open_controller(&api)?;

    log::info!("Controller opened. Enabling IMU...");
    device::enable_imu(&device.raw_file().lock().unwrap())?;
    log::info!("IMU enabled. Dumping frames (Ctrl-C to stop)...");

    let (reader, rx) = reader::Reader::start(device.hid);

    loop {
        match rx.recv() {
            Ok(frame) => {
                let (ax, ay, az) = frame.accel_g();
                let (gx, gy, gz) = frame.gyro_dps();
                println!(
                    "seq={:3} | accel=({:7.3},{:7.3},{:7.3}) g | gyro=({:8.1},{:8.1},{:8.1}) dps",
                    frame.seq_num, ax, ay, az, gx, gy, gz
                );
            }
            Err(e) => {
                log::error!("Recv error: {e}");
                break;
            }
        }
    }

    drop(rx);
    reader.join();
    log::info!("Debug dump finished.");
    Ok(())
}

fn open_controller_with_retry(api: &hidapi::HidApi) -> device::Device {
    loop {
        match device::open_controller(api) {
            Ok(d) => return d,
            Err(e) => {
                log::warn!("Failed to open controller: {e}. Retrying in 5 seconds...");
                std::thread::sleep(Duration::from_secs(5));
            }
        }
    }
}
