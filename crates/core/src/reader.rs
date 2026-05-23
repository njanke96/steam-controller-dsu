//! Provides a background reader for reading [`DSUFrame`](crate::dsu::DSUFrame) data from devices.

use std::sync::{Arc, atomic, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use crate::READ_ATOMIC_BOOL_ORDERING;
use crate::devices::Device;
use crate::dsu::DSUFrame;
use crate::errors::DeviceError;

/// Number of identical IMU frames before we consider the IMU frozen.
/// At 100 Hz this is 1 second
const FROZEN_DETECT_THRESHOLD: usize = 100;
/// Retry interval for re-initializing the device when the IMU is frozen
const REINIT_RETRY_INTERVAL: Duration = Duration::from_secs(1);
/// Number of consecutive failed reads before assuming disconnect.
/// At 100Hz this is ~1 second of no data.
const DISCONNECT_THRESHOLD: usize = 100;

/// Spawn a thread that reads from `device` and sends parsed frames over the returned channel.
///
/// The reader thread will exit when `running` is set to false.
/// Returns a [`JoinHandle`](std::thread::JoinHandle) and a mpsc Receiver for receiving frame data.
pub fn spawn_reader(
    running: Arc<atomic::AtomicBool>,
    device: impl Device + std::marker::Send + 'static,
) -> (std::thread::JoinHandle<()>, mpsc::Receiver<DSUFrame>) {
    let (tx, rx) = mpsc::channel::<DSUFrame>();

    let handle = thread::spawn(move || {
        let mut frame_state = FrameState::new();

        log::debug!("Reader thread started");

        while running.load(READ_ATOMIC_BOOL_ORDERING) {
            if !read_frame(&device, &mut frame_state, &tx) {
                break;
            }
        }

        log::debug!(
            "Reader thread finished after {} frames",
            frame_state.total_frames
        );
    });

    (handle, rx)
}

struct FrameState {
    pub frozen_count: usize,
    pub total_frames: usize,
    pub prev_frame: Option<DSUFrame>,
    pub fail_count: usize,
    pub last_init_attempt: Option<Instant>,
}

impl FrameState {
    pub fn new() -> Self {
        Self {
            frozen_count: 0,
            total_frames: 0,
            prev_frame: None,
            fail_count: 0,
            last_init_attempt: None,
        }
    }
}

/// Read a frame, returning true if another should be read.
fn read_frame<D>(device: &D, frame_state: &mut FrameState, tx: &mpsc::Sender<DSUFrame>) -> bool
where
    D: Device + std::marker::Send + 'static,
{
    match device.read_frame() {
        Ok(frame) => {
            frame_state.fail_count = 0;
            frame_state.total_frames += 1;

            // Check for frozen/stale IMU data
            // This is observed behavior when Steam disables the IMU on Steam devices
            let is_imu_frozen = frame_state
                .prev_frame
                .map(|prev| {
                    frame.accel_x == prev.accel_x
                        && frame.accel_y == prev.accel_y
                        && frame.accel_z == prev.accel_z
                        && frame.gyro_x == prev.gyro_x
                        && frame.gyro_y == prev.gyro_y
                        && frame.gyro_z == prev.gyro_z
                })
                .unwrap_or(false);

            let mut frame_to_send = frame;

            if is_imu_frozen {
                frame_state.frozen_count += 1;

                if frame_state.frozen_count == FROZEN_DETECT_THRESHOLD {
                    log::warn!(
                        "IMU data frozen ({} identical frames). Steam likely disabled the IMU.",
                        frame_state.frozen_count
                    );
                }

                // Periodically attempt to re-enable the IMU
                if frame_state.frozen_count >= FROZEN_DETECT_THRESHOLD {
                    let should_try = frame_state
                        .last_init_attempt
                        .map(|t| t.elapsed() >= REINIT_RETRY_INTERVAL)
                        .unwrap_or(true);
                    if should_try {
                        frame_state.last_init_attempt = Some(Instant::now());
                        if let Err(e) = device.initialize() {
                            log::warn!("Failed to reinitialize device while IMU frozen: {e}");
                        } else {
                            log::info!("Reinitialized device while IMU was frozen.");
                        }
                    }
                }

                // Zero out motion data so clients don't drift on stale values
                // TODO: should accel actuall be zeroed here? What about gravity?
                frame_to_send.accel_x = 0.0;
                frame_to_send.accel_y = 0.0;
                frame_to_send.accel_z = 0.0;
                frame_to_send.gyro_x = 0.0;
                frame_to_send.gyro_y = 0.0;
                frame_to_send.gyro_z = 0.0;
            } else {
                frame_state.frozen_count = 0;
                frame_state.last_init_attempt = None;
            }

            frame_state.prev_frame = Some(frame);

            if tx.send(frame_to_send).is_err() {
                log::debug!("Receiver has hung up, reader thread exiting");
                return false;
            }
        }
        Err(DeviceError::ShortRead(n, expected)) => {
            log::trace!("Short read: {} bytes (expected {})", n, expected);
            frame_state.fail_count += 1;
        }
        Err(DeviceError::InvalidReport(id)) => {
            log::trace!("Ignoring invalid report (first byte: 0x{:02x})", id);
            frame_state.fail_count = 0;
        }
        Err(e) => {
            log::trace!("HID read error: {}", e);
            frame_state.fail_count += 1;
        }
    }

    if frame_state.fail_count >= DISCONNECT_THRESHOLD {
        log::warn!(
            "Controller appears disconnected ({} consecutive read failures). Exiting reader.",
            frame_state.fail_count,
        );
        return false;
    }

    true
}
