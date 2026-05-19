use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use crate::errors::ServerError;
use crate::frame::TritonFrame;
use crate::{READ_ATOMIC_BOOL_ORDERING, dsu};

const CLIENT_TIMEOUT: Duration = Duration::from_secs(5);
const VERSION_TYPE: u32 = 0x100000;
const INFO_TYPE: u32 = 0x100001;
const DATA_TYPE: u32 = 0x100002;

// TODO: Move to server.rs
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Address or host to bind to
    pub bind_addr: String,
    // Port to listen on
    pub port: u16,
    /// Invert the yaxis values on the gyro and accelerometer
    pub invert_y: bool,
}
#[derive(Debug)]
struct Client {
    addr: SocketAddr,
    id: u32,
    slot: u8,
    last_seen: Instant,
    packet_counter: u32,
}

/// CemuHook UDP Server
pub struct Server {
    main_thread_running: Arc<atomic::AtomicBool>,
    server_thread_running: Arc<atomic::AtomicBool>,
    clients: Arc<Mutex<Vec<Client>>>,
    config: ServerConfig,
    socket: UdpSocket,
}

/// CemuHook UDP Server Send thread context
struct SendThreadContext {
    pub main_thread_running: Arc<atomic::AtomicBool>,
    pub server_thread_running: Arc<atomic::AtomicBool>,
    pub clients: Arc<Mutex<Vec<Client>>>,
    pub config: ServerConfig,
    pub socket: UdpSocket,
    pub rx: mpsc::Receiver<TritonFrame>,
}

type ThreadResults = (
    io::Result<()>,
    Result<(), Box<dyn std::any::Any + Send + 'static>>,
);

impl Server {
    /// Attempt to create a new `Server`
    pub fn new(
        main_thread_running: Arc<atomic::AtomicBool>,
        config: ServerConfig,
    ) -> Result<Self, ServerError> {
        let addr = format!("{}:{}", config.bind_addr, config.port);

        let socket = UdpSocket::bind(&addr).map_err(ServerError::UdpSocketOperationError)?;

        socket
            .set_read_timeout(Some(Duration::from_secs(1)))
            .map_err(ServerError::UdpSocketOperationError)?;

        log::info!("CemuHook server listening on {}", addr);

        let clients: Arc<Mutex<Vec<Client>>> = Arc::new(Mutex::new(Vec::new()));
        let server_thread_running = Arc::new(atomic::AtomicBool::new(true));

        Ok(Self {
            main_thread_running,
            server_thread_running,
            clients,
            config,
            socket,
        })
    }

    /// Start the CemuHook UDP server and broadcast frames received on `rx` to all subscribed CemuHook clients
    /// Blocks until both the Receving loop (this thread) and Send loop (background thread) complete
    /// Returns both results on Success, Err(ServerError) if the server failed to start
    pub fn run(
        &self,
        rx: mpsc::Receiver<TritonFrame>, // TODO: More agnostic struct for frame data, not device specific
    ) -> Result<ThreadResults, ServerError> {
        let send_context = SendThreadContext {
            main_thread_running: self.main_thread_running.clone(),
            server_thread_running: self.server_thread_running.clone(),
            clients: self.clients.clone(),
            config: self.config.clone(),
            socket: self
                .socket
                .try_clone()
                .map_err(ServerError::UdpSocketCloneFailed)?,
            rx,
        };

        // Spawn the send thread and store the handle
        let send_handle = thread::spawn(move || {
            Self::send_loop(send_context);
        });

        let recv_result = self.recv_loop();
        let send_result = send_handle.join();

        Ok((recv_result, send_result))
    }

    fn recv_loop(&self) -> io::Result<()> {
        let mut buf = [0u8; 256];

        while self.main_thread_running.load(READ_ATOMIC_BOOL_ORDERING)
            && self.server_thread_running.load(READ_ATOMIC_BOOL_ORDERING)
        {
            match self.socket.recv_from(&mut buf) {
                Ok((msg_len, addr)) => {
                    self.process_received_message(&buf, msg_len, &addr, &self.socket);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // Read timeout
                    self.prune_clients();
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                    // Probable SIGINT
                    log::debug!("UDP recv interrupted");
                }
                Err(e) => {
                    log::error!("UDP recv error: {:?}", e);
                }
            }
        }

        Ok(())
    }

    fn process_received_message(
        &self,
        buf: &[u8; 256],
        msg_len: usize,
        addr: &SocketAddr,
        socket: &UdpSocket,
    ) {
        if msg_len < 20 {
            log::trace!("Short packet ({} bytes) from {}", msg_len, addr);
            return;
        }

        let magic = &buf[0..4];
        if magic != b"DSUC" {
            log::trace!("Ignoring non-DSUC packet from {}", addr);
            return;
        }

        let client_id = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
        let event_type = u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);

        log::trace!(
            "Received {} request from {} (id={})",
            event_type_str(event_type),
            addr,
            client_id
        );

        match event_type {
            VERSION_TYPE => handle_version_request(client_id, addr, socket),
            INFO_TYPE => handle_info_request(buf, client_id, addr, socket),
            DATA_TYPE => handle_data_request(buf, msg_len, client_id, addr, &self.clients),
            _ => {
                log::trace!("Unhandled event type 0x{:06x} from {}", event_type, addr);
            }
        }
    }

    fn prune_clients(&self) {
        let Ok(mut list) = self.clients.lock() else {
            log::error!("Could not lock clients mutex... Skipping client prune.");
            return;
        };

        let before = list.len();
        list.retain(|c| c.last_seen.elapsed() < CLIENT_TIMEOUT);
        let after = list.len();
        if before != after {
            log::info!(
                "Pruned {} stale client(s), {} remaining",
                before - after,
                after
            );
        }
    }

    fn send_loop(context: SendThreadContext) {
        let mut packet_buf = [0u8; 100];
        let mut timestamp_us: u64 = 0;

        loop {
            if !context.main_thread_running.load(READ_ATOMIC_BOOL_ORDERING)
                || !context
                    .server_thread_running
                    .load(READ_ATOMIC_BOOL_ORDERING)
            {
                break;
            }

            let frame = match context.rx.recv() {
                Ok(f) => f,
                Err(_) => {
                    log::debug!("Frame channel closed, send loop exiting");
                    context
                        .server_thread_running
                        .store(false, atomic::Ordering::SeqCst);
                    break;
                }
            };

            let Ok(mut list) = context.clients.lock() else {
                log::error!("Not sending data this frame, could not lock clients mutex.");
                continue;
            };

            if list.is_empty() {
                continue;
            }

            for client in list.iter_mut() {
                client.packet_counter = client.packet_counter.wrapping_add(1);

                dsu::write_data_event(
                    &mut packet_buf,
                    &frame,
                    client.packet_counter,
                    client.id,
                    client.slot,
                    timestamp_us,
                    context.config.invert_y,
                );

                let (ax, ay, az) = frame.accel_g();
                let (gx, gy, gz) = frame.gyro_dps();
                log::trace!(
                    "Packet {} to {}: accel=({:.3}, {:.3}, {:.3}) gyro=({:.1}, {:.1}, {:.1})",
                    client.packet_counter,
                    client.addr,
                    ax,
                    ay,
                    az,
                    gx,
                    gy,
                    gz
                );

                if let Err(e) = context.socket.send_to(&packet_buf, client.addr) {
                    log::trace!("Send error to {}: {}", client.addr, e);
                }
            }

            timestamp_us = timestamp_us.wrapping_add(4000);
        }
    }
}

fn handle_version_request(client_id: u32, addr: &SocketAddr, socket: &UdpSocket) {
    let mut version_buf = [0u8; 22];
    dsu::write_version_response(&mut version_buf, client_id);
    if let Err(e) = socket.send_to(&version_buf, addr) {
        log::warn!("Failed to send version response to {}: {}", addr, e);
    }
}

fn handle_info_request(buf: &[u8; 256], client_id: u32, addr: &SocketAddr, socket: &UdpSocket) {
    let mut info_buf = [0u8; 32];
    // Parse requested slots.
    let port_cnt = i32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]) as usize;
    let requested = port_cnt.min(4);
    for i in 0..requested {
        let slot = buf[24 + i];
        // Report our single controller as connected on every
        // requested slot so clients don't have to be
        // configured for slot 0 specifically.
        dsu::write_info_response(&mut info_buf, slot, client_id, true);
        if let Err(e) = socket.send_to(&info_buf, addr) {
            log::warn!("Failed to send info response to {}: {}", addr, e);
        }
    }
}

fn handle_data_request(
    buf: &[u8; 256],
    msg_len: usize,
    client_id: u32,
    addr: &SocketAddr,
    clients: &Arc<Mutex<Vec<Client>>>,
) {
    // Parse requested slot from payload.
    // CemuHook DATA request: byte 20 = flags, byte 21 = slot.
    let requested_slot = if msg_len > 21 { buf[21] } else { 0 };

    let Ok(mut list) = clients.lock() else {
        log::error!("Not handling data request, could not lock clients mutex...");
        return;
    };

    match list.iter_mut().find(|c| c.addr == *addr) {
        Some(client) => {
            client.last_seen = Instant::now();
            client.id = client_id;
            client.slot = requested_slot;
            log::trace!("Updated existing client {} (slot={})", addr, requested_slot);
        }
        None => {
            log::info!("New client subscribed: {} (slot={})", addr, requested_slot);
            list.push(Client {
                addr: *addr,
                id: client_id,
                slot: requested_slot,
                last_seen: Instant::now(),
                packet_counter: 0,
            });
        }
    }
}

fn event_type_str(t: u32) -> &'static str {
    match t {
        VERSION_TYPE => "VERSION",
        INFO_TYPE => "INFO",
        DATA_TYPE => "DATA",
        _ => "UNKNOWN",
    }
}
