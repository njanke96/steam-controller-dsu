# Steam Controller DSU — Agent Context

## Crates

- **Root package (`steam-controller-dsu`):** CLI entrypoint for running the UDP server and debugging Steam Controller gyro output.
  All code in this crate directly involves the CLI interface. It should always be possible to replace this crate with another interface
  such as a GUI and have the exact same behavior under a different interface.
- `crates/core`: Core library crate (`scdsu-core`). All device reporting and UDP server logic lives in here, accessible through library functions.

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
