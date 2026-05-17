use std::sync::{Arc, atomic, mpsc};
use std::thread;

use crate::READ_ATOMIC_BOOL_ORDERING;
use crate::devices::device;
use crate::errors::DeviceError;
use crate::frame::TritonFrame;

/// Number of consecutive identical IMU frames before logging a warning.
const FROZEN_THRESHOLD: usize = 100;
/// Number of frozen frames before giving up and exiting (~5 seconds at 100Hz).
/// This allows recovery when Steam takes the controller and changes IMU mode.
const FROZEN_EXIT_THRESHOLD: usize = 500;
/// Number of consecutive failed reads before assuming disconnect.
/// At 100Hz this is ~1 second of no data.
const DISCONNECT_THRESHOLD: usize = 10;

/// Background reader that continuously parses Triton frames from the controller.
pub struct Reader {
    handle: Option<thread::JoinHandle<()>>,
}

impl Reader {
    /// Spawn a thread that reads from `device` and sends parsed frames over the returned channel.
    pub fn start(
        running: Arc<atomic::AtomicBool>,
        device: impl device::Device + std::marker::Send + 'static,
    ) -> (Self, mpsc::Receiver<TritonFrame>) {
        let (tx, rx) = mpsc::channel::<TritonFrame>();

        let handle = thread::spawn(move || {
            let mut frozen_count = 0usize;
            let mut total_frames = 0usize;
            let mut prev_frame: Option<TritonFrame> = None;
            let mut fail_count = 0usize;

            log::debug!("Reader thread started");

            while running.load(READ_ATOMIC_BOOL_ORDERING) {
                match device.read_triton_frame() {
                    Ok(frame) => {
                        fail_count = 0;
                        total_frames += 1;
                        log::trace!(
                            "seq={} accel=({},{},{}) gyro=({},{},{})",
                            frame.seq_num,
                            frame.accel_x,
                            frame.accel_y,
                            frame.accel_z,
                            frame.gyro_x,
                            frame.gyro_y,
                            frame.gyro_z
                        );

                        // Check for frozen/stale IMU data.
                        let is_frozen = prev_frame
                            .as_ref()
                            .map(|prev| frame.imu_eq(prev))
                            .unwrap_or(false);

                        if is_frozen {
                            frozen_count += 1;
                            if frozen_count == FROZEN_THRESHOLD {
                                log::warn!(
                                    "IMU data frozen ({} identical frames). Steam likely disabled the gyro..",
                                    frozen_count
                                );
                            }
                            if frozen_count >= FROZEN_EXIT_THRESHOLD {
                                log::warn!(
                                    "IMU frozen for {} frames. Exiting to trigger recovery.",
                                    frozen_count
                                );
                                break;
                            }
                            prev_frame = Some(frame);
                            continue;
                        }

                        if frozen_count >= FROZEN_THRESHOLD {
                            log::info!("IMU data resumed after {} frozen frames", frozen_count);
                        }
                        frozen_count = 0;

                        if total_frames.is_multiple_of(100) {
                            log::debug!(
                                "Reader: frame {} sent, seq={}, gyro=({},{},{})",
                                total_frames,
                                frame.seq_num,
                                frame.gyro_x,
                                frame.gyro_y,
                                frame.gyro_z
                            );
                        }

                        prev_frame = Some(frame);

                        if tx.send(frame).is_err() {
                            log::debug!("Receiver dropped, reader thread exiting");
                            break;
                        }
                    }
                    Err(DeviceError::ShortRead(n, expected)) => {
                        log::trace!("Short read: {} bytes (expected {})", n, expected);
                        fail_count += 1;
                    }
                    Err(DeviceError::NonTritonReport(id)) => {
                        log::trace!("Ignoring non-Triton report (first byte: 0x{:02x})", id);
                        fail_count = 0;
                    }
                    Err(e) => {
                        log::trace!("HID read error: {}", e);
                        fail_count += 1;
                    }
                }

                if fail_count >= DISCONNECT_THRESHOLD {
                    log::warn!(
                        "Controller appears disconnected ({} consecutive read failures). Exiting reader.",
                        fail_count
                    );
                    break;
                }
            }

            log::debug!("Reader thread finished after {} frames", total_frames);
        });

        (
            Self {
                handle: Some(handle),
            },
            rx,
        )
    }

    /// Block until the background thread finishes.
    pub fn join(mut self) {
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
