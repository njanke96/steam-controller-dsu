# Steam Controller DSU — Agent Context

## Project Overview
A Rust reimplementation of SteamDeckGyroDSU for the **new 2026 Steam Controller** (codename "Triton", USB VID `0x28de` / PID `0x1304`). It exposes motion data (accelerometer + gyroscope) via the CemuHook UDP protocol so emulators can use the controller's IMU.

## Crates

- **Root package (`steam-controller-dsu`):** CLI entrypoint for running the UDP server and debugging Steam Controller gyro output.
  All code in this crate directly involves the CLI interface. It should always be possible to replace this crate with another interface
  such as a GUI and have the exact same behavior under a different interface.
- `crates/core`: Core library crate (`scdsu-core`). All device reporting and UDP server logic lives in here, accessible through library functions.

## Critical Hardware Findings

### USB IDs
- **VID:** `0x28de` (Valve Software)
- **PID:** `0x1304` (Steam Controller Puck / new Steam Controller)
- The dongle exposes **multiple hidraw nodes**; only the vendor interface (`usage_page = 0xFF00`) accepts feature reports.

### Feature Report Quirk
`hidapi` (both C and Rust bindings) fails to send feature reports to this controller with `EPIPE`. **Direct `ioctl(HIDIOCSFEATURE)` on a separately opened `std::fs::File` works.**

- **Working format:** 64-byte buffer, Report ID `0x01` in byte 0, command in byte 1.
- **SDL/Steam Deck format:** 65-byte buffer, Report ID `0x00` — **rejected by this controller.**
- Retry loop: up to 50 attempts with 500 µs sleep (matches SDL `RADIO_WORKAROUND_SLEEP_ATTEMPTS`).

### IMU Enable Sequence
```
Report ID 1, 64 bytes:
[0x01, 0x81, 0x00, ...]               # CLEAR_DIGITAL_MAPPINGS (disable lizard mode)
[0x01, 0x8E, 0x00, ...]               # LOAD_DEFAULT_SETTINGS
[0x01, 0x87, 0x09,
 0x07, 0x07, 0x00,                    # LEFT_TRACKPAD_MODE = NONE (7)
 0x08, 0x07, 0x00,                    # RIGHT_TRACKPAD_MODE = NONE (7)
 0x30, 0x18, 0x00, ...]               # IMU_MODE = RAW_ACCEL (8) | RAW_GYRO (16) = 24
```

### Input Report Layout — Report ID `0x42` (TritonMTUFull)
54 bytes total (1 Report ID + 53 payload). Documented in SDL `controller_structs.h`.

| Offset | Size | Field |
|--------|------|-------|
| 0 | 1 | `seq_num` |
| 1 | 4 | `buttons` (u32 LE) |
| 5 | 2 | `trigger_left` (i16) |
| 7 | 2 | `trigger_right` (i16) |
| 9 | 2 | `left_stick_x` (i16) |
| 11 | 2 | `left_stick_y` (i16) |
| 13 | 2 | `right_stick_x` (i16) |
| 15 | 2 | `right_stick_y` (i16) |
| 17 | 2 | `left_pad_x` (i16) |
| 19 | 2 | `left_pad_y` (i16) |
| 21 | 2 | `pressure_left` (u16) |
| 23 | 2 | `right_pad_x` (i16) |
| 25 | 2 | `right_pad_y` (i16) |
| 27 | 2 | `pressure_right` (u16) |
| 29 | 4 | `imu_timestamp` (u32 LE) |
| 33 | 2 | `accel_x` (i16) |
| 35 | 2 | `accel_y` (i16) |
| 37 | 2 | `accel_z` (i16) |
| 39 | 2 | `gyro_x` (i16) |
| 41 | 2 | `gyro_y` (i16) |
| 43 | 2 | `gyro_z` (i16) |
| 45 | 2 | `quat_w` (i16) |
| 47 | 2 | `quat_x` (i16) |
| 49 | 2 | `quat_y` (i16) |
| 51 | 2 | `quat_z` (i16) |

### Sensor Scales (identical to Steam Deck)
- **Accel:** `16384` counts = 1 g → divide by `16384.0`
- **Gyro:** `16` counts = 1 deg/s → divide by `16.0`

Gyroscope outputs **angular velocity** (rate of change), not absolute orientation. CemuHook clients perform sensor fusion themselves.

## Sources for reference
1. **SDL `SDL_hidapi_steam.c`** — command IDs, retry logic, packet assembly, `ResetSteamController()` sequence.
   - `https://raw.githubusercontent.com/libsdl-org/SDL/main/src/joystick/hidapi/SDL_hidapi_steam.c`
2. **SDL `controller_structs.h`** — `TritonMTUFull_t`, `TritonMTUIMU_t`, `FeatureReportMsg`, output report structs.
   - `https://raw.githubusercontent.com/libsdl-org/SDL/main/src/joystick/hidapi/steam/controller_structs.h`
3. **Linux kernel `hid-steam.c`** — original Steam Controller protocol reverse-engineering, `ID_SET_SETTINGS_VALUES`, `SETTING_IMU_MODE`, `SETTING_GYRO_MODE_*` constants.
   - `https://raw.githubusercontent.com/torvalds/linux/master/drivers/hid/hid-steam.c`
4. **CemuHook Protocol Spec** — UDP packet formats, event types, CRC32 rules.
   - `https://v1993.github.io/cemuhook-protocol/`

## Build
```bash
cargo build
cargo run --bin steam-controller-dsu -- --debug
```

## Guidelines

- Always use context7 MCP when available.
- Run `cargo fmt` after any code changes, as well as `cargo clippy` for lints.
- Avoid using `unwrap`, `expect`, or any methods that panic unless certain a panic is not possible in that context.
