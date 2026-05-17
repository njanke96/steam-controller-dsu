use clap::Parser;
use std::net::SocketAddr;
use std::str::FromStr;

#[derive(Parser)]
pub struct Args {
    /// Run in debug mode: open the controller and dump raw IMU frames.
    #[arg(long)]
    pub debug: bool,

    /// Say hello to NAME instead of running the DSU server.
    #[arg(long)]
    pub name: Option<String>,

    /// UDP bind address for the CemuHook server.
    #[arg(long, default_value = "0.0.0.0")]
    pub bind_addr: String,

    /// UDP port for the CemuHook server.
    #[arg(long, default_value_t = 26760)]
    pub port: u16,
}

pub fn main() {
    env_logger::init();

    let args = Args::parse();

    if let Some(name) = args.name {
        println!("hello {name}");
        return;
    }

    if args.debug {
        run_debug_dump();
        return;
    }

    run_server(args.bind_addr, args.port);
}

fn run_server(bind_addr: String, port: u16) {
    let addr = SocketAddr::from_str(&format!("{}:{}", bind_addr, port))
        .unwrap_or_else(|e| {
            log::error!("Invalid bind address '{}:{}': {}", bind_addr, port, e);
            std::process::exit(1);
        });

    let api = match scdsu_core::hidapi::HidApi::new() {
        Ok(api) => api,
        Err(e) => {
            log::error!("Failed to initialize HID API: {e}");
            std::process::exit(1);
        }
    };

    let device = match scdsu_core::device::open_controller(&api) {
        Ok(d) => d,
        Err(e) => {
            log::error!("Failed to open controller: {e}");
            std::process::exit(1);
        }
    };

    log::info!("Controller opened. Enabling IMU...");
    if let Err(e) = scdsu_core::device::enable_imu(&*device.raw_file().lock().unwrap()) {
        log::error!("Failed to enable IMU: {e}");
        std::process::exit(1);
    }
    log::info!("IMU enabled. Starting CemuHook server on {} ...", addr);

    let raw = device.raw_file();
    let (reader, rx) = scdsu_core::reader::Reader::start(device.hid, raw);

    if let Err(e) = scdsu_core::server::Server::run(addr, rx) {
        log::error!("Server error: {e}");
    }

    reader.join();
    log::info!("Server shut down.");
}

fn run_debug_dump() {
    let api = match scdsu_core::hidapi::HidApi::new() {
        Ok(api) => api,
        Err(e) => {
            log::error!("Failed to initialize HID API: {e}");
            return;
        }
    };

    let device = match scdsu_core::device::open_controller(&api) {
        Ok(d) => d,
        Err(e) => {
            log::error!("Failed to open controller: {e}");
            return;
        }
    };

    log::info!("Controller opened. Enabling IMU...");
    if let Err(e) = scdsu_core::device::enable_imu(&*device.raw_file().lock().unwrap()) {
        log::error!("Failed to enable IMU: {e}");
        return;
    }
    log::info!("IMU enabled. Dumping frames (Ctrl-C to stop)...");

    let raw = device.raw_file();
    let (reader, rx) = scdsu_core::reader::Reader::start(device.hid, raw);

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
