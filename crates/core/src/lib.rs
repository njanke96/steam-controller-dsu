#[cfg(target_os = "windows")]
compile_error!("This crate does not support Windows.");

pub mod errors;

pub(crate) mod devices;
pub(crate) mod dsu;
pub(crate) mod reader;
pub(crate) mod server;

pub use server::ServerConfig;

use std::sync::Arc;
use std::sync::atomic;
use std::time::Duration;

use crate::devices::device::Device;
use crate::errors::{DeviceError, ServerError};
use crate::reader::Reader;

pub const READ_ATOMIC_BOOL_ORDERING: atomic::Ordering = atomic::Ordering::Relaxed;
const CONTROLLER_OPEN_RETRY_DELAY_SEC: u64 = 5;

/// Sleep in 100 ms increments while `running`.
pub(crate) fn sleep_interruptible(running: &atomic::AtomicBool, total: Duration) {
    let start = std::time::Instant::now();
    while start.elapsed() < total {
        if !running.load(READ_ATOMIC_BOOL_ORDERING) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100).min(total - start.elapsed()));
    }
}

/// Run the server loop until receiving a signal
pub fn run_server(
    running: Arc<atomic::AtomicBool>,
    config: server::ServerConfig,
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

        log::info!("Controller opened. Initializing...");
        if let Err(e) = device.initialize() {
            log::error!("Failed to initialize device: {e}");
            sleep_interruptible(&running, Duration::from_secs(3));
            continue;
        }
        log::info!(
            "Device initialized. Starting CemuHook server on {}:{} ...",
            config.bind_addr,
            config.port
        );

        // Start the device reader and cemuhook udp server
        //

        let (reader, rx) = Reader::start(running.clone(), device);

        let udp_server = server::Server::new(running.clone(), config.clone())?;

        match udp_server.run(rx) {
            Ok((recv_result, send_result)) => {
                if let Err(e) = recv_result {
                    log::error!("UDP receive loop error: {e}");
                }
                if let Err(err) = send_result {
                    log::error!("UDP send thread panicked: {err:?}");
                }
            }
            Err(e) => {
                log::error!("Failed to clone UDP socket for send thread: {e}");
            }
        }

        if let Err(err) = reader.join() {
            log::error!("Reader thread panicked: {err:?}");
        }

        if !running.load(READ_ATOMIC_BOOL_ORDERING) {
            return Ok(());
        }

        log::info!("Server shut down. Waiting 3 seconds before reconnect...");
        sleep_interruptible(&running, Duration::from_secs(3));
    }
}

/// Run the debug loop.
/// Attempts to open the controller and dump frames.
pub fn run_debug_dump(running: Arc<atomic::AtomicBool>) -> Result<(), DeviceError> {
    let api = hidapi::HidApi::new()?;

    // If more devices are ever supported, add selection logic
    let device = devices::triton::linux_find_and_open(&api)?;

    log::info!("Controller opened. Running initialization...");
    device.initialize()?;
    log::info!("Initialized. Dumping frames...");

    let (reader, rx) = Reader::start(running.clone(), device);

    while running.load(READ_ATOMIC_BOOL_ORDERING) {
        match rx.recv() {
            Ok(frame) => {
                println!(
                    "accel=({:7.3},{:7.3},{:7.3}) g | gyro=({:8.1},{:8.1},{:8.1}) dps",
                    frame.accel_x,
                    frame.accel_y,
                    frame.accel_z,
                    frame.gyro_x,
                    frame.gyro_y,
                    frame.gyro_z
                );
            }
            Err(e) => {
                log::error!("Recv error: {e}");
                break;
            }
        }
    }

    drop(rx);
    if let Err(err) = reader.join() {
        log::error!("Reader thread panicked: {err:?}");
    }

    log::info!("Debug dump finished.");
    Ok(())
}

/// Open a controller with unlimited retries
/// Returns `None` if interrupted.
fn open_controller_with_retry(
    running: Arc<atomic::AtomicBool>,
    api: &hidapi::HidApi,
) -> Option<impl devices::device::Device + use<>> {
    loop {
        if !running.load(READ_ATOMIC_BOOL_ORDERING) {
            return None;
        }

        // If more devices are ever supported, add selection logic
        match devices::triton::linux_find_and_open(api) {
            Ok(d) => return Some(d),
            Err(e) => {
                log::warn!(
                    "Failed to open controller: {e}. Retrying in {} seconds...",
                    CONTROLLER_OPEN_RETRY_DELAY_SEC
                );
                sleep_interruptible(
                    &running,
                    Duration::from_secs(CONTROLLER_OPEN_RETRY_DELAY_SEC),
                );
            }
        }
    }
}
