//! Core library for `steam-controller-dsu`.
//!
//! This crate provides functions to run a CemuHook (DSU) server which supplies controller input
//! state over a UDP connection to emulators.

#[cfg(target_os = "windows")]
compile_error!("This crate does not support Windows.");

pub mod devices;
pub mod dsu;
pub mod errors;
pub mod reader;
pub mod server;

pub use server::ServerConfig;

use std::sync::Arc;
use std::sync::atomic;
use std::time::Duration;

use crate::devices::device::Device;
use crate::errors::{DeviceError, ServerError};

pub(crate) const READ_ATOMIC_BOOL_ORDERING: atomic::Ordering = atomic::Ordering::Relaxed;
const CONTROLLER_OPEN_RETRY_DELAY_SEC: u64 = 5;

/// Run the server loop until receiving a signal.
///
/// Accepts an [`AtomicBool`](std::sync::atomic::AtomicBool) within an `Arc<>` for signaling when
/// the server should shut down (set to `false`).
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

        let (reader_handle, rx) = reader::spawn_reader(running.clone(), device);

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

        if let Err(err) = reader_handle.join() {
            log::error!("Reader thread panicked: {err:?}");
        }

        if !running.load(READ_ATOMIC_BOOL_ORDERING) {
            return Ok(());
        }

        log::info!("Server shut down. Waiting 3 seconds before reconnect...");
        sleep_interruptible(&running, Duration::from_secs(3));
    }
}

/// Runs a debug loop, dumping DSU-compatible frames to stdout for debugging purposes.
///
/// Accepts an [`AtomicBool`](std::sync::atomic::AtomicBool) within an `Arc<>` for signaling when
/// the server should shut down.
pub fn run_debug_dump(running: Arc<atomic::AtomicBool>) -> Result<(), DeviceError> {
    let api = hidapi::HidApi::new()?;

    // If more devices are ever supported, add selection logic
    let device = devices::triton::linux_find_and_open(&api)?;

    log::info!("Controller opened. Running initialization...");
    device.initialize()?;
    log::info!("Initialized. Dumping frames...");

    let (reader_handle, rx) = reader::spawn_reader(running.clone(), device);

    while running.load(READ_ATOMIC_BOOL_ORDERING) {
        match rx.recv() {
            Ok(frame) => {
                let buttons_pressed: Vec<&str> = [
                    ("A", frame.a),
                    ("B", frame.b),
                    ("X", frame.x),
                    ("Y", frame.y),
                    ("L1", frame.l1),
                    ("R1", frame.r1),
                    ("L2", frame.l2),
                    ("R2", frame.r2),
                    ("L3", frame.l3),
                    ("R3", frame.r3),
                    ("Options", frame.options),
                    ("Share", frame.share),
                    ("Home", frame.home),
                    ("QAM", frame.touch),
                ]
                .iter()
                .filter(|(_, p)| *p)
                .map(|(n, _)| *n)
                .collect();

                let dpad_pressed: Vec<&str> = [
                    ("Up", frame.dpad_up),
                    ("Down", frame.dpad_down),
                    ("Left", frame.dpad_left),
                    ("Right", frame.dpad_right),
                ]
                .iter()
                .filter(|(_, p)| *p)
                .map(|(n, _)| *n)
                .collect();

                let buttons_str = if buttons_pressed.is_empty() {
                    "none".to_string()
                } else {
                    buttons_pressed.join(" ")
                };
                let dpad_str = if dpad_pressed.is_empty() {
                    "none".to_string()
                } else {
                    dpad_pressed.join(" ")
                };

                println!(
                    "Buttons: {buttons_str}\n\
                     DPad:    {dpad_str}\n\
                     Sticks:  L({:4},{:4})  R({:4},{:4})\n\
                     Triggers: L2={:3}  R2={:3}\n\
                     Accel:   ({:7.3},{:7.3},{:7.3}) g\n\
                     Gyro:    ({:8.1},{:8.1},{:8.1}) dps",
                    frame.left_stick_x,
                    frame.left_stick_y,
                    frame.right_stick_x,
                    frame.right_stick_y,
                    frame.analog_l2,
                    frame.analog_r2,
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
    if let Err(err) = reader_handle.join() {
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
