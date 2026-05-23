# steam-controller-dsu

[![Crates.io Version](https://img.shields.io/crates/v/steam-controller-dsu)](https://crates.io/crates/steam-controller-dsu)
![Crates.io License](https://img.shields.io/crates/l/steam-controller-dsu)

This is a DSU (CemuHook) UDP server, currently supporting the Gyro data of the 2026 Steam Controller on Linux. Windows should work, but is not the main focus.

## Install

### Cargo

1. [Install Rust and Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html#install-rust-and-cargo)
2. Ensure you have development files for `libudev` (`libudev-dev` on Debian, `systemd-devel` on Fedora, `systemd` on Arch, `eudev-libudev-devel` on Void)
3. `cargo install steam-controller-dsu`

### Manual

1. Ensure you have `libudev` (it should be already present on systemd distros, it is `eudev-libudev` on Void)
2. Download the latest release binary from [here](https://github.com/njanke96/steam-controller-dsu/releases), make sure it's executable, and stick it somewhere in `$PATH`.

### From source

1. Download the latest stable versions of Rust and Cargo
2. Ensure you have development files for `libudev` (`libudev-dev` on Debian, `systemd-devel` on Fedora, `systemd` on Arch, `eudev-libudev-devel` on Void)
3. Clone and build with `cargo build --release`

## Usage

### Examples

Start a server with the default options:

```
steam-controller-dsu
```

Start a server with the gyro only enabled when the gyro grips are activated, with a small deadzone:

```
steam-controller-dsu -b left_grip,right_grip --gyro-activation-mode all --gyro-deadzone 20
```

### Steam launch option example

```bash
/path/to/steam-controller-dsu -L & env MANGOHUD=1 %command; pgrep -f steam-controller-dsu | xargs kill
```

### Full usage

```
Options:
      --debug
          Run in debug mode: open the controller and dump raw IMU frames

      --bind-addr <BIND_ADDR>
          UDP bind address for the CemuHook server
          
          [default: 0.0.0.0]

      --port <PORT>
          UDP port for the CemuHook server
          
          [default: 26760]

      --invert-pitch
          When set invert the motion controls pitch axis

      --slot <SLOT>
          CemuHook controller slot to report on (0-3 for Controllers 1 through 4). Controller number is slot + 1
          
          [default: 0]

      --device-path <DEVICE_PATH>
          Specific device path to open. Example: /dev/hidraw11

  -L, --no-enable-lizard-mode-on-close
          Don't enable lizard mode when the device is closed (such as on program exit)

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

Gyro Options:
  -b, --gyro-activation-buttons <GYRO_ACTIVATION_BUTTONS>
          Comma-separated list of buttons/sensors that activate gyro reporting.
          
          Depending on the emulated game, turning the gyro on/off might not work how you expect!
          
          Example value: left_grip,right_grip
          
          Possible values to include in the list: dpad_left, dpad_down, dpad_right, dpad_up, start, select, guide, quaternary,
a, b, x, y, l1, r1, l2, r2, l3, r3, l4, l5, r4, r5, left_stick_touch, right_stick_touch, left_pad_touch, right_pad_touch, left_grip, right_grip

      --gyro-activation-mode <GYRO_ACTIVATION_MODE>
          Gyro activation mode
          
          Possible values: any, all
          
          When any is specified, at least one gyro activation button must be pressed. When all is specified, all gyro activation buttons must be pressed.
          
          [default: any]

      --gyro-deadzone <GYRO_DEADZONE>
          Gyro deadzone in degrees per second. Values below this threshold are reported as zero
          
          [default: 0]

      --gyro-pitch-scale <GYRO_PITCH_SCALE>
          Scale factor for the pitch gyro axis
          
          [default: 1]

      --gyro-yaw-scale <GYRO_YAW_SCALE>
          Scale factor for the yaw gyro axis
          
          [default: 1]

      --gyro-roll-scale <GYRO_ROLL_SCALE>
          Scale factor for the roll gyro axis
          
          [default: 1]
```

## Tested Emulators

- Cemu
  - Quirk: Through Steam input, make sure the controller has no bindings. You need to map controls in Cemu settings through the DSU server,
    existing bindings can mess up the process.
- Ryujinx
- Eden
- Azahar
  - Quirk: If the server is stopped while the emulator is running motion controls might not work until the emulator is started.

## Wishlist

- Support more devices (Steam Deck, 2015 Steam Controller, future Steam Devices, non-Steam devices?)

## AI Usage

AI assistance from open models (Kimi K2.6, GLM-5) is used in the development of this application. AI is used for quick research and prototyping of new features and boilerplate, not for 
architectural decisions or complex logic. No generated code is currently used in the project without complete understanding by the human involved.
