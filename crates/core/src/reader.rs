use hidapi::HidDevice;
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};

use crate::frame::TritonFrame;

/// Background reader that continuously parses Triton frames from the controller.
pub struct Reader {
    handle: Option<JoinHandle<()>>,
}

impl Reader {
    /// Spawn a thread that reads from `device` and sends parsed frames over the returned channel.
    pub fn start(device: HidDevice) -> (Self, Receiver<TritonFrame>) {
        let (tx, rx) = mpsc::channel::<TritonFrame>();

        let handle = thread::spawn(move || {
            let mut buf = [0u8; 64];
            loop {
                match device.read_timeout(&mut buf, 100) {
                    Ok(n) if n >= TritonFrame::REPORT_SIZE => {
                        if let Some(frame) = TritonFrame::parse(&buf[..n]) {
                            if tx.send(frame).is_err() {
                                break; // receiver dropped
                            }
                        }
                    }
                    Ok(_) => continue,
                    Err(_) => continue,
                }
            }
        });

        (Self { handle: Some(handle) }, rx)
    }

    /// Block until the background thread finishes (e.g. after the channel receiver is dropped).
    pub fn join(mut self) {
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
