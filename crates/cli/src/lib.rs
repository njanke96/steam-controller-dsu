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
pub fn entrypoint() {
    env_logger::init();

    let args = Args::parse();

    if args.debug {
        scdsu_core::run_debug_dump();
        return;
    }

    scdsu_core::run_server(args.bind_addr, args.port, args.invert_y);
}
