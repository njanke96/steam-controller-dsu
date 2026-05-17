use clap::Parser;

#[derive(Parser)]
pub struct Args {
    #[arg(long)]
    pub name: Option<String>,
}

pub fn main() {
    let args = Args::parse();

    if let Some(name) = args.name {
        println!("hello {name}");
        return;
    }

    // Temporary integration test: open controller and dump IMU frames
    let api = match scdsu_core::hidapi::HidApi::new() {
        Ok(api) => api,
        Err(e) => {
            eprintln!("Failed to initialize HID API: {e}");
            return;
        }
    };

    let device = match scdsu_core::device::open_controller(&api) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to open controller: {e}");
            return;
        }
    };

    println!("Controller opened. Enabling IMU...");
    if let Err(e) = scdsu_core::device::enable_imu(&device) {
        eprintln!("Failed to enable IMU: {e}");
        return;
    }
    println!("IMU enabled. Reading 30 frames...\n");

    let (reader, rx) = scdsu_core::reader::Reader::start(device.hid);

    loop {
        match rx.recv() {
            Ok(frame) => {
                let (ax, ay, az) = frame.accel_g();
                let (gx, gy, gz) = frame.gyro_dps();
                println!(
                    "Frame | seq={:3} | accel=({:7.3},{:7.3},{:7.3}) g | gyro=({:8.1},{:8.1},{:8.1}) dps",
                    frame.seq_num, ax, ay, az, gx, gy, gz
                );
            }
            Err(e) => {
                eprintln!("Recv error: {e}");
                break;
            }
        }
    }

    drop(rx);
    reader.join();
    println!("\nDone.");
}
