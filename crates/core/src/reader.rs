use hidapi::HidDevice;
use std::fs::File;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use crate::device::enable_imu;
use crate::frame::TritonFrame;

/// Number of consecutive identical IMU frames before triggering a re-enable.
/// Normal sensor noise rarely produces more than a few identical frames.
const FROZEN_THRESHOLD: usize = 100;
/// Number of frames to wait after a re-enable attempt before trying again.
const REENABLE_COOLDOWN_FRAMES: usize = 1000;

/// Background reader that continuously parses Triton frames from the controller.
pub struct Reader {
    handle: Option<thread::JoinHandle<()>>,
}

impl Reader {
    /// Spawn a thread that reads from `device` and sends parsed frames over the returned channel.
    pub fn start(
        hid: HidDevice,
        raw: Arc<Mutex<File>>,
    ) -> (Self, mpsc::Receiver<TritonFrame>) {
        let (tx, rx) = mpsc::channel::<TritonFrame>();

        let handle = thread::spawn(move || {
            let mut buf = [0u8; 64];
            let mut frozen_count = 0usize;
            let mut cooldown = 0usize;
            let mut total_frames = 0usize;
            let mut prev_frame: Option<TritonFrame> = None;

            log::debug!("Reader thread started");

            loop {
                match hid.read_timeout(&mut buf, 100) {
                    Ok(n) if n >= TritonFrame::REPORT_SIZE => {
                        if let Some(frame) = TritonFrame::parse(&buf[..n]) {
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
                                if frozen_count >= 50 {
                                    log::debug!(
                                        "Frozen IMU frame detected ({}/{})",
                                        frozen_count,
                                        FROZEN_THRESHOLD
                                    );
                                }
                                if frozen_count >= FROZEN_THRESHOLD && cooldown == 0 {
                                    log::warn!(
                                        "IMU appears frozen ({} consecutive identical frames). Re-enabling...",
                                        frozen_count
                                    );
                                    if let Ok(file) = raw.lock() {
                                        match enable_imu(&*file) {
                                            Ok(_) => {
                                                log::info!("IMU re-enabled successfully");
                                                cooldown = REENABLE_COOLDOWN_FRAMES;
                                                frozen_count = 0;
                                            }
                                            Err(e) => {
                                                log::error!("Failed to re-enable IMU: {}", e);
                                                cooldown = REENABLE_COOLDOWN_FRAMES;
                                            }
                                        }
                                    }
                                }
                                prev_frame = Some(frame);
                                continue;
                            }

                            if frozen_count > 0 && frozen_count >= 50 {
                                log::debug!("IMU data resumed after {} frozen frames", frozen_count);
                            }
                            frozen_count = 0;

                            if cooldown > 0 {
                                cooldown -= 1;
                            }

                            if total_frames % 100 == 0 {
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
                        } else {
                            log::trace!("Ignoring non-Triton report (first byte: 0x{:02x})", buf[0]);
                        }
                    }
                    Ok(n) => {
                        log::trace!("Short read: {} bytes", n);
                    }
                    Err(e) => {
                        log::trace!("HID read error: {}", e);
                    }
                }
            }

            log::debug!("Reader thread finished after {} frames", total_frames);
        });

        (Self { handle: Some(handle) }, rx)
    }

    /// Block until the background thread finishes.
    pub fn join(mut self) {
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
