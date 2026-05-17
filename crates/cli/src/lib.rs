use clap::Parser;

#[derive(Parser)]
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

    /// Invert the pitch axis (opposite of Nintendo Switch behavior).
    #[arg(long, default_value_t = false)]
    pub invert_y: bool,
}

/// The CLI entrypoint to be called from the root crate's `[[bin]]`
pub fn entrypoint() -> i32 {
    env_logger::init();

    let args = Args::parse();

    if args.debug {
        if let Err(err) = scdsu_core::run_debug_dump() {
            log::error!("Error from run_debug_dump: {err}");
        }
        return 1;
    }

    let config = scdsu_core::ServerConfig {
        bind_addr: args.bind_addr,
        port: args.port,
        invert_y: args.invert_y,
    };

    log::debug!("Server configuration from cli args: {config:?}");

    if let Err(err) = scdsu_core::run_server(config) {
        log::error!("Error from run_server: {err}");
        return 1;
    }

    0
}
