# steam-controller-dsu

This is a DSU (CemuHook) UDP server, currently supporting the Gyro data of the 2026 Steam Controller on Linux. Windows is not supported at this time.

## Install

This is not yet published as I'm still refining the implementation and testing it myself. I plan to publish to crates.io eventually.

## Usage

```
Usage: steam-controller-dsu [OPTIONS]

Options:
      --debug                  Run in debug mode: open the controller and dump raw IMU frames
      --bind-addr <BIND_ADDR>  UDP bind address for the CemuHook server [default: 0.0.0.0]
      --port <PORT>            UDP port for the CemuHook server [default: 26760]
      --invert-pitch           Invert the pitch axis
  -h, --help                   Print help
```

## Building from source

1. Download the latest stable versions of Rust and Cargo
2. Ensure you have development files for `libudev` (`libudev-dev` on Debian, `systemd-devel` on Fedora, `systemd` on Arch, `eudev-libudev-devel` on Void)
3. Clone and build with `cargo build --release`

## Wishlist

- Support Windows
- Support Steam Input-like configuration options (Gyro deadzone, sensitivity, etc)
- Support more devices (Steam Deck, 2015 Steam Controller, future Steam Devices)

## AI Usage

AI assistance from open models (Kimi K2.6, GLM-5) is used in the development of this application. AI is used for quick research and prototyping of new features and boilerplate, not for 
architectural decisions or complex logic. No generated code is currently used in the project without complete understanding by the human involved.
