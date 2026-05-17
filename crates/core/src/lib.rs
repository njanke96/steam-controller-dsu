use std::sync::Arc;
use std::sync::atomic;
use std::time::Duration;

use crate::errors::{DeviceError, ServerError};

pub mod errors;

pub(crate) mod device;
pub(crate) mod frame;
pub(crate) mod protocol;
pub(crate) mod reader;
pub(crate) mod report;
pub(crate) mod server;

pub const READ_ATOMIC_BOOL_ORDERING: atomic::Ordering = atomic::Ordering::Relaxed;

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
pub fn run_server(
    running: Arc<atomic::AtomicBool>,
    config: ServerConfig,
) -> Result<(), ServerError> {
    let mut api = hidapi::HidApi::new()?;

    loop {
        if !running.load(READ_ATOMIC_BOOL_ORDERING) {
            return Ok(());
        }

        if let Err(e) = api.refresh_devices() {
            log::warn!("Failed to refresh HID device list: {e}");
        }

        let Some(device) = open_controller_with_retry(running.clone(), &api) else {
            // Interrupted by signal
            return Ok(());
        };

        log::info!("Controller opened. Enabling IMU...");
        if let Err(e) = device::enable_imu(&device.raw_file().lock().unwrap()) {
            log::error!("Failed to enable IMU: {e}");
            std::thread::sleep(Duration::from_secs(3));
            continue;
        }
        log::info!(
            "IMU enabled. Starting CemuHook server on {}:{} ...",
            config.bind_addr,
            config.port
        );

        let (reader, rx) = reader::Reader::start(running.clone(), device.hid);

        if let Err(e) = server::Server::run(rx, running.clone(), &config) {
            log::error!("Server error: {e}");
        }

        reader.join();

        if !running.load(READ_ATOMIC_BOOL_ORDERING) {
            return Ok(());
        }

        log::info!("Server shut down. Waiting 3 seconds before reconnect...");
        std::thread::sleep(Duration::from_secs(3));
    }
}

/// Run the debug loop.
/// Attempts to open the controller and dump frames.
pub fn run_debug_dump(running: Arc<atomic::AtomicBool>) -> Result<(), DeviceError> {
    let api = hidapi::HidApi::new()?;

    let device = device::open_controller(&api)?;

    log::info!("Controller opened. Enabling IMU...");
    device::enable_imu(&device.raw_file().lock().unwrap())?;
    log::info!("IMU enabled. Dumping frames (Ctrl-C to stop)...");

    let (reader, rx) = reader::Reader::start(running.clone(), device.hid);

    while running.load(READ_ATOMIC_BOOL_ORDERING) {
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

/// Open a controller with unlimited retries
/// Returns `None` if interrupted
fn open_controller_with_retry(
    running: Arc<atomic::AtomicBool>,
    api: &hidapi::HidApi,
) -> Option<device::Device> {
    loop {
        if !running.load(READ_ATOMIC_BOOL_ORDERING) {
            return None;
        }

        match device::open_controller(api) {
            Ok(d) => return Some(d),
            Err(e) => {
                log::warn!("Failed to open controller: {e}. Retrying in 5 seconds...");
                for _ in 0..50 {
                    if !running.load(READ_ATOMIC_BOOL_ORDERING) {
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }
}
