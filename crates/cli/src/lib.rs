use clap::Parser;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

#[derive(Parser)]
pub struct Args {
    /// Run in debug mode: open the controller and dump raw IMU frames.
    #[arg(long)]
    pub debug: bool,

    /// UDP bind address for the CemuHook server.
    #[arg(long, default_value = "0.0.0.0")]
    pub bind_addr: String,

    /// UDP port for the CemuHook server.
    #[arg(long, default_value_t = 26760)]
    pub port: u16,

    /// Invert the pitch axis (opposite of Nintendo Switch behavior).
    #[arg(long)]
    pub invert_y: bool,
}

pub fn main() {
    env_logger::init();

    let args = Args::parse();

    if args.debug {
        run_debug_dump();
        return;
    }

    run_server(args.bind_addr, args.port, args.invert_y);
}

fn open_controller_with_retry(api: &scdsu_core::hidapi::HidApi) -> scdsu_core::device::Device {
    loop {
        match scdsu_core::device::open_controller(api) {
            Ok(d) => return d,
            Err(e) => {
                log::warn!("Failed to open controller: {e}. Retrying in 5 seconds...");
                std::thread::sleep(Duration::from_secs(5));
            }
        }
    }
}

fn run_server(bind_addr: String, port: u16, invert_y: bool) {
    let addr = SocketAddr::from_str(&format!("{}:{}", bind_addr, port)).unwrap_or_else(|e| {
        log::error!("Invalid bind address '{}:{}': {}", bind_addr, port, e);
        std::process::exit(1);
    });

    let mut api = match scdsu_core::hidapi::HidApi::new() {
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
        if let Err(e) = scdsu_core::device::enable_imu(&*device.raw_file().lock().unwrap()) {
            log::error!("Failed to enable IMU: {e}");
            std::thread::sleep(Duration::from_secs(3));
            continue;
        }
        log::info!("IMU enabled. Starting CemuHook server on {} ...", addr);

        let (reader, rx) = scdsu_core::reader::Reader::start(device.hid);

        if let Err(e) = scdsu_core::server::Server::run(addr, rx, invert_y) {
            log::error!("Server error: {e}");
        }

        reader.join();
        log::info!("Server shut down. Waiting 3 seconds before reconnect...");
        std::thread::sleep(Duration::from_secs(3));
    }
}

fn run_debug_dump() {
    let api = match scdsu_core::hidapi::HidApi::new() {
        Ok(api) => api,
        Err(e) => {
            log::error!("Failed to initialize HID API: {e}");
            return;
        }
    };

    let device = open_controller_with_retry(&api);

    log::info!("Controller opened. Enabling IMU...");
    if let Err(e) = scdsu_core::device::enable_imu(&*device.raw_file().lock().unwrap()) {
        log::error!("Failed to enable IMU: {e}");
        return;
    }
    log::info!("IMU enabled. Dumping frames (Ctrl-C to stop)...");

    let (reader, rx) = scdsu_core::reader::Reader::start(device.hid);

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
