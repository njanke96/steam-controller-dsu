use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

pub mod device;
pub mod frame;
pub mod protocol;
pub mod reader;
pub mod report;
pub mod server;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Address or host to bind to
    pub bind_addr: String,
    // Port to listen on
    pub port: u16,
    /// Invert the yaxis values on the gyro and accelerometer
    pub invert_y: bool,
}

pub fn run_server(config: ServerConfig) {
    let addr = SocketAddr::from_str(&format!("{}:{}", config.bind_addr, config.port))
        .unwrap_or_else(|e| {
            log::error!(
                "Invalid bind address '{}:{}': {}",
                config.bind_addr,
                config.port,
                e
            );
            std::process::exit(1);
        });

    let mut api = match hidapi::HidApi::new() {
        Ok(api) => api,
        Err(e) => {
            log::error!("Failed to initialize HID API: {e}");
            std::process::exit(1);
        }
    };

    loop {
        if let Err(e) = api.refresh_devices() {
            log::warn!("Failed to refresh HID device list: {e}");
        }
        let device = open_controller_with_retry(&api);

        log::info!("Controller opened. Enabling IMU...");
        if let Err(e) = device::enable_imu(&device.raw_file().lock().unwrap()) {
            log::error!("Failed to enable IMU: {e}");
            std::thread::sleep(Duration::from_secs(3));
            continue;
        }
        log::info!("IMU enabled. Starting CemuHook server on {} ...", addr);

        let (reader, rx) = reader::Reader::start(device.hid);

        if let Err(e) = server::Server::run(rx, &config) {
            log::error!("Server error: {e}");
        }

        reader.join();
        log::info!("Server shut down. Waiting 3 seconds before reconnect...");
        std::thread::sleep(Duration::from_secs(3));
    }
}

pub fn run_debug_dump() {
    let api = match hidapi::HidApi::new() {
        Ok(api) => api,
        Err(e) => {
            log::error!("Failed to initialize HID API: {e}");
            return;
        }
    };

    let device = open_controller_with_retry(&api);

    log::info!("Controller opened. Enabling IMU...");
    if let Err(e) = device::enable_imu(&device.raw_file().lock().unwrap()) {
        log::error!("Failed to enable IMU: {e}");
        return;
    }
    log::info!("IMU enabled. Dumping frames (Ctrl-C to stop)...");

    let (reader, rx) = reader::Reader::start(device.hid);

    loop {
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
}

fn open_controller_with_retry(api: &hidapi::HidApi) -> device::Device {
    loop {
        match device::open_controller(api) {
            Ok(d) => return d,
            Err(e) => {
                log::warn!("Failed to open controller: {e}. Retrying in 5 seconds...");
                std::thread::sleep(Duration::from_secs(5));
            }
        }
    }
}
