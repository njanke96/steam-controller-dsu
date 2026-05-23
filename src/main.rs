use std::sync::{Arc, atomic};

use clap::Parser;
use scdsu_core::devices;

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

    /// When set invert the motion controls pitch axis.
    #[arg(long, default_value_t = false)]
    pub invert_pitch: bool,

    /// CemuHook controller slot to report on (0-3 for Controllers 1 through 4). Controller number is slot + 1.
    #[arg(long, default_value_t = 0)]
    pub slot: u8,

    /// Specific device path to open. Example: /dev/hidraw11
    #[arg(long)]
    pub device_path: Option<String>,

    /// Don't enable lizard mode when the device is closed (such as on program exit)
    #[arg(short = 'L', long, default_value_t = false)]
    pub no_enable_lizard_mode_on_close: bool,

    /// Comma-separated list of buttons/sensors that activate gyro reporting.
    ///
    /// Example value: left_grip,right_grip
    ///
    /// Possible values to include in the list: dpad_left, dpad_down, dpad_right, dpad_up, start, select,
    /// guide, quaternary, a, b, x, y, l1, r1, l2, r2, l3, r3, l4, l5, r4, r5,
    /// left_stick_touch, right_stick_touch, left_pad_touch, right_pad_touch,
    /// left_grip, right_grip
    #[arg(short = 'b', long, value_delimiter = ',')]
    gyro_activation_buttons: Vec<devices::DeviceButton>,

    /// Gyro activation mode
    ///
    /// Possible values: any, all
    ///
    /// When any is specified, at least one gyro activation button must be pressed.
    /// When all is specified, all gyro activation buttons must be pressed.
    #[arg(long, default_value_t = devices::GyroActivationMode::default())]
    gyro_activation_mode: devices::GyroActivationMode,

    /// Gyro deadzone in degrees per second. Values below this threshold are reported as zero.
    #[arg(long, default_value_t = 0.0)]
    gyro_deadzone: f32,

    /// Scale factor for the pitch gyro axis.
    #[arg(long, default_value_t = 1.0)]
    gyro_pitch_scale: f32,

    /// Scale factor for the yaw gyro axis.
    #[arg(long, default_value_t = 1.0)]
    gyro_yaw_scale: f32,

    /// Scale factor for the roll gyro axis.
    #[arg(long, default_value_t = 1.0)]
    gyro_roll_scale: f32,
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

    let device_config = scdsu_core::devices::DeviceConfig {
        no_enable_lizard_mode_on_close: args.no_enable_lizard_mode_on_close,
        gyro_activation_inputs: args.gyro_activation_buttons,
        gyro_activation_mode: args.gyro_activation_mode,
        gyro_deadzone: args.gyro_deadzone,
        gyro_pitch_scale: args.gyro_pitch_scale,
        gyro_yaw_scale: args.gyro_yaw_scale,
        gyro_roll_scale: args.gyro_roll_scale,
    };

    if args.debug {
        if let Err(err) =
            scdsu_core::run_debug_dump(running, args.device_path.as_deref(), Some(device_config))
        {
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

    if let Err(err) = scdsu_core::run_server(running, config, device_config) {
        log::error!("Error from run_server: {err}");
        return 1;
    }

    0
}

fn main() {
    let return_code = entrypoint();
    std::process::exit(return_code);
}
