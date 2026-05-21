use std::sync::{Arc, atomic};

use clap::Parser;

#[derive(Parser)]
#[command(version)]
pub struct Args {
    /// Run in debug mode: open the controller and dump raw IMU frames.
    #[arg(long, default_value_t = false)]
    pub debug: bool,

    /// UDP bind address for the CemuHook server.
    #[arg(long, default_value = "0.0.0.0")]
    pub bind_addr: String,

    /// UDP port for the CemuHook server.
    #[arg(long, default_value_t = 26760)]
    pub port: u16,

    /// Invert the pitch axis
    #[arg(long, default_value_t = false)]
    pub invert_pitch: bool,

    /// CemuHook controller slot to report on (0-3 for Controllers 1 through 4). Controller number is slot + 1.
    #[arg(long, default_value_t = 0)]
    pub slot: u8,

    /// Specific device path to open. Example: /dev/hidraw11
    #[arg(long)]
    pub device_path: Option<String>,
}

pub fn entrypoint() -> i32 {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Stdout)
        .init();

    // ctrlc signal handler, handles SIGINT on Unix as well
    let running = Arc::new(atomic::AtomicBool::new(true));
    let running_signal = running.clone();

    if let Err(err) = ctrlc::set_handler(move || {
        log::info!("Got a shutdown signal...");
        running_signal.store(false, atomic::Ordering::SeqCst);
    }) {
        log::error!("Failed to set ctrlc signal handler: {err}");
        return 1;
    };

    let args = Args::parse();

    if args.slot > 3 {
        log::error!("Invalid slot: {}. Slot must be between 0 and 3.", args.slot);
        return 1;
    }

    if args.debug {
        if let Err(err) = scdsu_core::run_debug_dump(running, args.device_path.as_deref()) {
            log::error!("Error from run_debug_dump: {err}");
        }
        return 1;
    }

    let config = scdsu_core::ServerConfig {
        bind_addr: args.bind_addr,
        port: args.port,
        invert_pitch: args.invert_pitch,
        slot: args.slot,
        device_path: args.device_path,
    };

    log::debug!("Server configuration from cli args: {config:?}");

    if let Err(err) = scdsu_core::run_server(running, config) {
        log::error!("Error from run_server: {err}");
        return 1;
    }

    0
}

fn main() {
    let return_code = entrypoint();
    std::process::exit(return_code);
}
