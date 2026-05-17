use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc::Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crate::frame::TritonFrame;
use crate::{ServerConfig, protocol};

const CLIENT_TIMEOUT: Duration = Duration::from_secs(5);
const VERSION_TYPE: u32 = 0x100000;
const INFO_TYPE: u32 = 0x100001;
const DATA_TYPE: u32 = 0x100002;

#[derive(Debug)]
struct Client {
    addr: SocketAddr,
    id: u32,
    slot: u8,
    last_seen: Instant,
    packet_counter: u32,
}

pub struct Server;

impl Server {
    /// Start the CemuHook UDP server on `bind_addr` and broadcast frames
    /// received on `rx` to all subscribed CemuHook clients.
    pub fn run(rx: Receiver<TritonFrame>, config: &ServerConfig) -> io::Result<()> {
        let socket = UdpSocket::bind(config.bind_addr.clone())?;
        socket.set_read_timeout(Some(Duration::from_secs(1)))?;
        log::info!("CemuHook server listening on {}", &config.bind_addr);

        let clients: Arc<Mutex<Vec<Client>>> = Arc::new(Mutex::new(Vec::new()));
        let shutdown = Arc::new(AtomicBool::new(false));

        // Spawn send thread.
        let send_socket = socket.try_clone()?;
        let send_clients = Arc::clone(&clients);
        let send_shutdown = Arc::clone(&shutdown);
        let send_config = config.clone();

        let send_handle = thread::spawn(move || {
            send_loop(send_socket, rx, send_clients, send_shutdown, send_config);
        });

        // Run recv loop on this thread.
        let recv_result = recv_loop(socket, clients, shutdown);

        // Wait for send thread to finish.
        if let Err(e) = send_handle.join() {
            log::error!("Send thread panicked: {:?}", e);
        }

        recv_result
    }
}

fn recv_loop(
    socket: UdpSocket,
    clients: Arc<Mutex<Vec<Client>>>,
    shutdown: Arc<AtomicBool>,
) -> io::Result<()> {
    let mut buf = [0u8; 256];
    let mut version_buf = [0u8; 22];
    let mut info_buf = [0u8; 32];

    while !shutdown.load(Ordering::Relaxed) {
        match socket.recv_from(&mut buf) {
            Ok((n, addr)) => {
                if n < 20 {
                    log::trace!("Short packet ({} bytes) from {}", n, addr);
                    continue;
                }

                let magic = &buf[0..4];
                if magic != b"DSUC" {
                    log::trace!("Ignoring non-DSUC packet from {}", addr);
                    continue;
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
                    VERSION_TYPE => {
                        protocol::write_version_response(&mut version_buf, client_id);
                        if let Err(e) = socket.send_to(&version_buf, addr) {
                            log::warn!("Failed to send version response to {}: {}", addr, e);
                        }
                    }
                    INFO_TYPE => {
                        // Parse requested slots.
                        let port_cnt =
                            i32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]) as usize;
                        let requested = port_cnt.min(4);
                        for i in 0..requested {
                            let slot = buf[24 + i];
                            // Report our single controller as connected on every
                            // requested slot so clients don't have to be
                            // configured for slot 0 specifically.
                            protocol::write_info_response(&mut info_buf, slot, client_id, true);
                            if let Err(e) = socket.send_to(&info_buf, addr) {
                                log::warn!("Failed to send info response to {}: {}", addr, e);
                            }
                        }
                    }
                    DATA_TYPE => {
                        // Parse requested slot from payload.
                        // CemuHook DATA request: byte 20 = flags, byte 21 = slot.
                        let requested_slot = if n > 21 { buf[21] } else { 0 };

                        let mut list = clients.lock().unwrap();
                        match list.iter_mut().find(|c| c.addr == addr) {
                            Some(client) => {
                                client.last_seen = Instant::now();
                                client.id = client_id;
                                client.slot = requested_slot;
                                log::trace!(
                                    "Updated existing client {} (slot={})",
                                    addr,
                                    requested_slot
                                );
                            }
                            None => {
                                log::info!(
                                    "New client subscribed: {} (slot={})",
                                    addr,
                                    requested_slot
                                );
                                list.push(Client {
                                    addr,
                                    id: client_id,
                                    slot: requested_slot,
                                    last_seen: Instant::now(),
                                    packet_counter: 0,
                                });
                            }
                        }
                    }
                    _ => {
                        log::trace!("Unhandled event type 0x{:06x} from {}", event_type, addr);
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // Read timeout — prune stale clients.
                prune_clients(&clients);
            }
            Err(e) => {
                log::error!("UDP recv error: {}", e);
            }
        }
    }

    Ok(())
}

fn send_loop(
    socket: UdpSocket,
    rx: Receiver<TritonFrame>,
    clients: Arc<Mutex<Vec<Client>>>,
    shutdown: Arc<AtomicBool>,
    config: ServerConfig,
) {
    let mut packet_buf = [0u8; 100];
    let mut timestamp_us: u64 = 0;

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        let frame = match rx.recv() {
            Ok(f) => f,
            Err(_) => {
                log::debug!("Frame channel closed, send loop exiting");
                shutdown.store(true, Ordering::Relaxed);
                break;
            }
        };

        let mut list = clients.lock().unwrap();
        if list.is_empty() {
            continue;
        }

        for client in list.iter_mut() {
            client.packet_counter = client.packet_counter.wrapping_add(1);

            protocol::write_data_event(
                &mut packet_buf,
                &frame,
                client.packet_counter,
                client.id,
                client.slot,
                timestamp_us,
                config.invert_y,
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

            if let Err(e) = socket.send_to(&packet_buf, client.addr) {
                log::trace!("Send error to {}: {}", client.addr, e);
            }
        }

        timestamp_us = timestamp_us.wrapping_add(4000);
    }
}

fn prune_clients(clients: &Arc<Mutex<Vec<Client>>>) {
    let mut list = clients.lock().unwrap();
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

fn event_type_str(t: u32) -> &'static str {
    match t {
        VERSION_TYPE => "VERSION",
        INFO_TYPE => "INFO",
        DATA_TYPE => "DATA",
        _ => "UNKNOWN",
    }
}
