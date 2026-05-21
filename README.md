# steam-controller-dsu

This is a DSU (CemuHook) UDP server, currently supporting the Gyro data of the 2026 Steam Controller on Linux. Windows is not supported at this time.

## Tested Emulators

- Cemu
  - Quirk: Through Steam input, make sure the controller has no bindings. You need to map controls in Cemu settings through the DSU server,
    existing bindings can mess up the process.
- Ryujinx
- Eden
- Azahar
  - Quirk: If the server is stopped while the emulator is running motion controls might not work until the emulator is started.

## Install

### Cargo

1. [Install Rust and Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html#install-rust-and-cargo)
2. Ensure you have development files for `libudev` (`libudev-dev` on Debian, `systemd-devel` on Fedora, `systemd` on Arch, `eudev-libudev-devel` on Void)
3. `cargo install steam-controller-dsu`

### Manual

1. Ensure you have `libudev` (present on systemd distros)
2. Download the latest release binary, make sure it's executable, and stick it somewhere in `$PATH`.

### From source

1. Download the latest stable versions of Rust and Cargo
2. Ensure you have development files for `libudev` (`libudev-dev` on Debian, `systemd-devel` on Fedora, `systemd` on Arch, `eudev-libudev-devel` on Void)
3. Clone and build with `cargo build --release`

## Usage

```
Usage: steam-controller-dsu [OPTIONS]

Options:
      --debug                      Run in debug mode: open the controller and dump raw IMU frames
      --bind-addr <BIND_ADDR>      UDP bind address for the CemuHook server [default: 0.0.0.0]
      --port <PORT>                UDP port for the CemuHook server [default: 26760]
      --invert-pitch               Invert the pitch axis
      --slot <SLOT>                CemuHook controller slot to report on (0-3 for Controllers 1 through 4). Controller number is slot + 1 [default: 0]
      --device-path <DEVICE_PATH>  Specific device path to open. Example: /dev/hidraw11
  -h, --help                       Print help
  -V, --version                    Print version
```

## Wishlist

- Support Windows
- Support Steam Input-like configuration options (Gyro deadzone, sensitivity, etc)
- Support more devices (Steam Deck, 2015 Steam Controller, future Steam Devices, non-Steam devices?)

## AI Usage

AI assistance from open models (Kimi K2.6, GLM-5) is used in the development of this application. AI is used for quick research and prototyping of new features and boilerplate, not for 
architectural decisions or complex logic. No generated code is currently used in the project without complete understanding by the human involved.
