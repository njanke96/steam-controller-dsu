use std::sync::{Arc, atomic, mpsc};
use std::thread;

use crate::READ_ATOMIC_BOOL_ORDERING;
use crate::devices::device;
use crate::errors::DeviceError;
use crate::frame::TritonFrame;

/// Number of frozen frames before giving up and exiting (~1 second at 100Hz).
/// This allows recovery when Steam takes the controller and changes IMU mode.
const FROZEN_EXIT_THRESHOLD: usize = 100;
/// Number of consecutive failed reads before assuming disconnect.
/// At 100Hz this is ~1 second of no data.
const DISCONNECT_THRESHOLD: usize = 10;

/// Background reader that continuously parses Triton frames from a device
pub struct Reader {
    handle: thread::JoinHandle<()>,
}

impl Reader {
    /// Spawn a thread that reads from `device` and sends parsed frames over the returned channel.
    /// Returns immediately, use `Reader::join` to join the reader thread.
    /// Returns `Self` and a mpsc Receiver for the UDP server
    pub fn start(
        running: Arc<atomic::AtomicBool>,
        device: impl device::Device + std::marker::Send + 'static,
    ) -> (Self, mpsc::Receiver<TritonFrame>) {
        let (tx, rx) = mpsc::channel::<TritonFrame>();

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

        (Self { handle }, rx)
    }

    /// Join the reader's thread
    pub fn join(self) -> Result<(), Box<dyn std::any::Any + Send>> {
        self.handle.join()
    }
}

struct FrameState {
    pub frozen_count: usize,
    pub total_frames: usize,
    pub prev_frame: Option<TritonFrame>,
    pub fail_count: usize,
}

impl FrameState {
    pub fn new() -> Self {
        Self {
            frozen_count: 0,
            total_frames: 0,
            prev_frame: None,
            fail_count: 0,
        }
    }
}

/// Read a frame, returning true if another should be read.
fn read_frame<D>(device: &D, frame_state: &mut FrameState, tx: &mpsc::Sender<TritonFrame>) -> bool
where
    D: device::Device + std::marker::Send + 'static,
{
    match device.read_triton_frame() {
        Ok(frame) => {
            frame_state.fail_count = 0;
            frame_state.total_frames += 1;

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

            // Check for frozen/stale IMU data
            // This is observed behavior when Steam disables the Gyro
            let is_frozen = frame_state
                .prev_frame
                .as_ref()
                .map(|prev| frame.imu_eq(prev))
                .unwrap_or(false);

            if is_frozen {
                frame_state.frozen_count += 1;
                if frame_state.frozen_count >= FROZEN_EXIT_THRESHOLD {
                    log::warn!(
                        "IMU data frozen ({} identical frames). Steam likely disabled the gyro..",
                        frame_state.frozen_count
                    );
                    return false;
                }

                frame_state.prev_frame = Some(frame);
                return true;
            }

            frame_state.frozen_count = 0;

            if frame_state.total_frames.is_multiple_of(100) {
                log::debug!(
                    "Reader: frame {} sent, seq={}, gyro=({},{},{})",
                    frame_state.total_frames,
                    frame.seq_num,
                    frame.gyro_x,
                    frame.gyro_y,
                    frame.gyro_z
                );
            }

            frame_state.prev_frame = Some(frame);

            if tx.send(frame).is_err() {
                log::debug!("Receiver has hung up, reader thread exiting");
                return false;
            }
        }
        Err(DeviceError::ShortRead(n, expected)) => {
            log::trace!("Short read: {} bytes (expected {})", n, expected);
            frame_state.fail_count += 1;
        }
        Err(DeviceError::NonTritonReport(id)) => {
            log::trace!("Ignoring non-Triton report (first byte: 0x{:02x})", id);
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
