//! Provides functionality for working with DSU protocol data.

const PROTOCOL_VERSION: u16 = 1001;

pub(crate) const HEADER_LEN: usize = 16;
pub(crate) const VERSION_MESSAGE_LEN: usize = 22;
pub(crate) const INFO_MESSAGE_LEN: usize = 32;
pub(crate) const DATA_MESSAGE_LEN: usize = 100;
pub(crate) const RUMBLE_INFO_MESSAGE_LEN: usize = 32;

const VERSION_TYPE: u32 = 0x100000;
const INFO_TYPE: u32 = 0x100001;
const DATA_TYPE: u32 = 0x100002;
const RUMBLE_INFO_TYPE: u32 = 0x110001;
const RUMBLE_MOTOR_TYPE: u32 = 0x110002;
const UNKNOWN_MESSAGE_TYPE: u32 = 0xFFFFFF;

/// DSU Message Types
#[derive(Debug, Clone)]
pub enum DSUMessageType {
    Version,
    Info,
    Data,
    RumbleInfo,
    RumbleMotor,
    Unknown,
}

impl From<u32> for DSUMessageType {
    fn from(value: u32) -> Self {
        match value {
            VERSION_TYPE => DSUMessageType::Version,
            INFO_TYPE => DSUMessageType::Info,
            DATA_TYPE => DSUMessageType::Data,
            RUMBLE_INFO_TYPE => DSUMessageType::RumbleInfo,
            RUMBLE_MOTOR_TYPE => DSUMessageType::RumbleMotor,
            _ => DSUMessageType::Unknown,
        }
    }
}

impl From<DSUMessageType> for u32 {
    fn from(value: DSUMessageType) -> Self {
        match value {
            DSUMessageType::Version => VERSION_TYPE,
            DSUMessageType::Info => INFO_TYPE,
            DSUMessageType::Data => DATA_TYPE,
            DSUMessageType::RumbleInfo => RUMBLE_INFO_TYPE,
            DSUMessageType::RumbleMotor => RUMBLE_MOTOR_TYPE,
            DSUMessageType::Unknown => UNKNOWN_MESSAGE_TYPE,
        }
    }
}

/// DSU frame representing all controller data sent over the CemuHook protocol.
/// DSU protocol reference can be found [`here`](https://v1993.github.io/cemuhook-protocol/).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DSUFrame {
    pub dpad_left: bool,
    pub dpad_down: bool,
    pub dpad_right: bool,
    pub dpad_up: bool,
    pub options: bool,
    pub r3: bool,
    pub l3: bool,
    pub share: bool,
    pub y: bool,
    pub b: bool,
    pub a: bool,
    pub x: bool,
    pub r1: bool,
    pub l1: bool,
    pub r2: bool,
    pub l2: bool,
    pub home: bool,
    pub touch: bool,
    pub left_stick_x: u8,
    pub left_stick_y: u8,
    pub right_stick_x: u8,
    pub right_stick_y: u8,
    pub analog_r2: u8,
    pub analog_l2: u8,
    pub raw_accel_x: f32,
    pub raw_accel_y: f32,
    pub raw_accel_z: f32,
    pub raw_gyro_x: f32,
    pub raw_gyro_y: f32,
    pub raw_gyro_z: f32,
    pub accel_x: f32,
    pub accel_y: f32,
    pub accel_z: f32,
    pub gyro_x: f32,
    pub gyro_y: f32,
    pub gyro_z: f32,
}

/// Write the common CemuHook packet header into `buf`
///
/// The CRC32 field (bytes 8..12) is zeroed so the caller can compute it after
/// filling the payload.
///
/// # Panics
///
/// `buf` must be at least [`HEADER_LEN`] bytes, or else this panics.
fn write_header(buf: &mut [u8], payload_len: usize, client_id: u32) {
    buf[0..4].copy_from_slice(b"DSUS");
    buf[4..6].copy_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    buf[6..8].copy_from_slice(&usize_to_u16(payload_len).to_le_bytes());
    buf[8..12].fill(0); // crc32 placeholder
    buf[12..16].copy_from_slice(&client_id.to_le_bytes());
}

/// CRC32 used by the CemuHook protocol.
/// Matches the algorithm from SteamDeckGyroDSU.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Build a CemuHook protocol-version response packet into `buf`.
pub fn write_version_response(buf: &mut [u8; VERSION_MESSAGE_LEN], client_id: u32) {
    let message_type: u32 = DSUMessageType::Version.into();

    buf.fill(0);
    write_header(buf, VERSION_MESSAGE_LEN - HEADER_LEN, client_id);
    buf[16..20].copy_from_slice(&message_type.to_le_bytes());
    buf[20..22].copy_from_slice(&PROTOCOL_VERSION.to_le_bytes());

    let c = crc32(&buf[..22]);
    buf[8..12].copy_from_slice(&c.to_le_bytes());
}

/// Write the shared response fields into `buf`.
///
/// Writes the slot number and, when `connected` is true, the slot state,
/// device model, and connection type.
fn write_shared_response(buf: &mut [u8], slot: u8, connected: bool) {
    buf[20] = slot;
    if connected {
        buf[21] = 2; // slotState = connected
        buf[22] = 2; // deviceModel = full gyro
        buf[23] = 1; // connection = USB
    }
}

/// Build a CemuHook controller-info response packet into `buf`.
pub fn write_info_response(
    buf: &mut [u8; INFO_MESSAGE_LEN],
    slot: u8,
    client_id: u32,
    connected: bool,
) {
    let message_type: u32 = DSUMessageType::Info.into();

    buf.fill(0);
    write_header(buf, INFO_MESSAGE_LEN - HEADER_LEN, client_id);
    buf[16..20].copy_from_slice(&message_type.to_le_bytes());

    write_shared_response(buf, slot, connected);

    let c = crc32(&buf[..32]);
    buf[8..12].copy_from_slice(&c.to_le_bytes());
}

/// Build a CemuHook data-event packet from a `DSUFrame`.
pub fn write_data_event(
    buf: &mut [u8; DATA_MESSAGE_LEN],
    frame: &DSUFrame,
    packet_num: u32,
    client_id: u32,
    slot: u8,
    timestamp_us: u64,
    invert_pitch: bool,
) {
    let message_type: u32 = DSUMessageType::Data.into();

    buf.fill(0);
    write_header(buf, DATA_MESSAGE_LEN - HEADER_LEN, client_id);
    buf[16..20].copy_from_slice(&message_type.to_le_bytes());

    write_shared_response(buf, slot, true);

    // mac1/mac2/battery already zero
    buf[31] = 1; // connected

    // packetNumber (offset 32)
    buf[32..36].copy_from_slice(&packet_num.to_le_bytes());

    // Buttons (offset 36)
    buf[36] = get_bitmask(&[
        (frame.dpad_left, 7),
        (frame.dpad_down, 6),
        (frame.dpad_right, 5),
        (frame.dpad_up, 4),
        (frame.options, 3),
        (frame.r3, 2),
        (frame.l3, 1),
        (frame.share, 0),
    ]);
    buf[37] = get_bitmask(&[
        (frame.y, 7),
        (frame.b, 6),
        (frame.a, 5),
        (frame.x, 4),
        (frame.r1, 3),
        (frame.l1, 2),
        (frame.r2, 1),
        (frame.l2, 0),
    ]);
    buf[38] = u8::from(frame.home);
    buf[39] = u8::from(frame.touch);

    // Sticks (offset 40)
    buf[40] = frame.left_stick_x;
    buf[41] = frame.left_stick_y;
    buf[42] = frame.right_stick_x;
    buf[43] = frame.right_stick_y;

    // Analog buttons (offset 44)
    // Cemu reads these analog values even for digital buttons.
    buf[44] = if frame.dpad_left { u8::MAX } else { 0 };
    buf[45] = if frame.dpad_down { u8::MAX } else { 0 };
    buf[46] = if frame.dpad_right { u8::MAX } else { 0 };
    buf[47] = if frame.dpad_up { u8::MAX } else { 0 };
    buf[48] = if frame.y { u8::MAX } else { 0 };
    buf[49] = if frame.b { u8::MAX } else { 0 };
    buf[50] = if frame.a { u8::MAX } else { 0 };
    buf[51] = if frame.x { u8::MAX } else { 0 };
    buf[52] = if frame.r1 { u8::MAX } else { 0 };
    buf[53] = if frame.l1 { u8::MAX } else { 0 };
    buf[54] = frame.analog_r2;
    buf[55] = frame.analog_l2;

    // Touch data (bytes 56-67) are already zeroed.

    // MotionData timestamp (offset 68)
    buf[68..76].copy_from_slice(&timestamp_us.to_le_bytes());

    // Accelerometer in g (offset 76)
    let acc_x = frame.accel_x;
    let acc_y = if invert_pitch {
        -frame.accel_y
    } else {
        frame.accel_y
    };
    let acc_z = frame.accel_z;

    buf[76..80].copy_from_slice(&acc_x.to_le_bytes());
    buf[80..84].copy_from_slice(&acc_y.to_le_bytes());
    buf[84..88].copy_from_slice(&acc_z.to_le_bytes());

    // Gyroscope in deg/s (offset 88)
    let pitch = if invert_pitch {
        -frame.gyro_x
    } else {
        frame.gyro_x
    };

    // when gravity reference is flipped with invert_pitch, this needs to be flipped too
    let yaw = if invert_pitch {
        -frame.gyro_y
    } else {
        frame.gyro_y
    };

    let roll = frame.gyro_z;

    buf[88..92].copy_from_slice(&pitch.to_le_bytes());
    buf[92..96].copy_from_slice(&yaw.to_le_bytes());
    buf[96..100].copy_from_slice(&roll.to_le_bytes());

    let c = crc32(&buf[..DATA_MESSAGE_LEN]);
    buf[8..12].copy_from_slice(&c.to_le_bytes());
}

/// Get a DSU button bitmask from a slice of bool and bit position pairs
fn get_bitmask(bits: &[(bool, u8)]) -> u8 {
    let mut mask = 0u8;
    for &(on, pos) in bits {
        if on {
            mask |= 1u8 << pos;
        }
    }
    mask
}

pub fn write_rumble_info_response(
    buf: &mut [u8; RUMBLE_INFO_MESSAGE_LEN],
    slot: u8,
    client_id: u32,
    connected: bool,
) {
    let message_type: u32 = DSUMessageType::RumbleInfo.into();

    buf.fill(0);
    write_header(buf, RUMBLE_INFO_MESSAGE_LEN - HEADER_LEN, client_id);
    buf[16..20].copy_from_slice(&message_type.to_le_bytes());

    write_shared_response(buf, slot, connected);

    // Two vibration motors supported
    buf[31] = 2;

    let c = crc32(&buf[..RUMBLE_INFO_MESSAGE_LEN]);
    buf[8..12].copy_from_slice(&c.to_le_bytes());
}

/// usize to u16 for header_len
///
/// # Panics
///
/// Panics when the len does not fit in a u16
fn usize_to_u16(len: usize) -> u16 {
    len.try_into().expect("len does not fit in a u16")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_bitmask_empty() {
        assert_eq!(get_bitmask(&[]), 0);
    }

    #[test]
    fn test_get_bitmask_single_bit() {
        assert_eq!(get_bitmask(&[(true, 0)]), 0b00000001);
        assert_eq!(get_bitmask(&[(true, 7)]), 0b10000000);
    }

    #[test]
    fn test_get_bitmask_multiple_bits() {
        assert_eq!(get_bitmask(&[(true, 0), (true, 1), (true, 2)]), 0b00000111);
        assert_eq!(get_bitmask(&[(true, 0), (true, 7)]), 0b10000001);
    }

    #[test]
    fn test_write_version_response() {
        let mut buf = [0u8; 22];
        write_version_response(&mut buf, 0x12345678);

        assert_eq!(&buf[0..4], b"DSUS");
        assert_eq!(u16::from_le_bytes([buf[4], buf[5]]), 1001);
        assert_eq!(u16::from_le_bytes([buf[6], buf[7]]), 22 - 16);
        assert_eq!(
            u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            0x12345678
        );
        assert_eq!(
            u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]),
            0x100000
        );
        assert_eq!(u16::from_le_bytes([buf[20], buf[21]]), 1001);
    }

    #[test]
    fn test_write_info_response_connected() {
        let mut buf = [0u8; 32];
        write_info_response(&mut buf, 1, 0xABCD_EF01, true);

        assert_eq!(&buf[0..4], b"DSUS");
        assert_eq!(u16::from_le_bytes([buf[6], buf[7]]), 16);
        assert_eq!(
            u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            0xABCD_EF01
        );
        assert_eq!(buf[20], 1);
        assert_eq!(buf[21], 2);
        assert_eq!(buf[22], 2);
        assert_eq!(buf[23], 1);
    }

    #[test]
    fn test_write_info_response_disconnected() {
        let mut buf = [0u8; 32];
        write_info_response(&mut buf, 0, 0, false);

        assert_eq!(&buf[0..4], b"DSUS");
        assert_eq!(buf[20], 0);
        assert_eq!(buf[21], 0);
        assert_eq!(buf[22], 0);
        assert_eq!(buf[23], 0);
    }

    #[test]
    fn test_write_rumble_info_response_connected() {
        let mut buf = [0u8; 32];
        write_rumble_info_response(&mut buf, 1, 0xABCD_EF01, true);

        assert_eq!(&buf[0..4], b"DSUS");
        assert_eq!(u16::from_le_bytes([buf[4], buf[5]]), 1001);
        assert_eq!(u16::from_le_bytes([buf[6], buf[7]]), 16);
        assert_eq!(
            u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            0xABCD_EF01
        );
        assert_eq!(
            u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]),
            0x110001
        );
        assert_eq!(buf[20], 1);
        assert_eq!(buf[21], 2);
        assert_eq!(buf[22], 2);
        assert_eq!(buf[23], 1);
        assert_eq!(buf[31], 1);
    }

    fn create_test_frame() -> DSUFrame {
        DSUFrame {
            dpad_left: true,
            dpad_down: false,
            dpad_right: true,
            dpad_up: false,
            options: true,
            r3: false,
            l3: true,
            share: false,
            y: true,
            b: false,
            a: true,
            x: false,
            r1: true,
            l1: false,
            r2: true,
            l2: false,
            home: true,
            touch: false,
            left_stick_x: 128,
            left_stick_y: 64,
            right_stick_x: 200,
            right_stick_y: 50,
            analog_r2: 200,
            analog_l2: 100,
            raw_accel_x: 1.0,
            raw_accel_y: 0.5,
            raw_accel_z: -0.5,
            raw_gyro_x: 10.0,
            raw_gyro_y: -5.0,
            raw_gyro_z: 2.5,
            accel_x: 1.0,
            accel_y: 0.5,
            accel_z: -0.5,
            gyro_x: 10.0,
            gyro_y: -5.0,
            gyro_z: 2.5,
        }
    }

    #[test]
    fn test_write_data_event_basic() {
        let mut buf = [0u8; 100];
        let frame = create_test_frame();
        write_data_event(&mut buf, &frame, 12345, 0xDEAD_BEEF, 0, 1_000_000, false);

        assert_eq!(&buf[0..4], b"DSUS");
        assert_eq!(u16::from_le_bytes([buf[6], buf[7]]), 84);
        assert_eq!(
            u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            0xDEAD_BEEF
        );
        assert_eq!(buf[20], 0);
        assert_eq!(
            u32::from_le_bytes([buf[32], buf[33], buf[34], buf[35]]),
            12345
        );
    }

    #[test]
    fn test_write_data_event_buttons() {
        let mut buf = [0u8; 100];
        let frame = create_test_frame();
        write_data_event(&mut buf, &frame, 0, 0, 0, 0, false);

        assert_eq!(buf[36], 0b1010_1010);
        assert_eq!(buf[37], 0b1010_1010);
        assert_eq!(buf[38], 1);
        assert_eq!(buf[39], 0);
    }

    #[test]
    fn test_write_data_event_sticks() {
        let mut buf = [0u8; 100];
        let frame = create_test_frame();
        write_data_event(&mut buf, &frame, 0, 0, 0, 0, false);

        assert_eq!(buf[40], 128);
        assert_eq!(buf[41], 64);
        assert_eq!(buf[42], 200);
        assert_eq!(buf[43], 50);
    }

    #[test]
    fn test_write_data_event_analog_buttons() {
        let mut buf = [0u8; 100];
        let frame = create_test_frame();
        write_data_event(&mut buf, &frame, 0, 0, 0, 0, false);

        assert_eq!(buf[44], 255);
        assert_eq!(buf[45], 0);
        assert_eq!(buf[46], 255);
        assert_eq!(buf[47], 0);
        assert_eq!(buf[48], 255);
        assert_eq!(buf[49], 0);
        assert_eq!(buf[50], 255);
        assert_eq!(buf[51], 0);
        assert_eq!(buf[52], 255);
        assert_eq!(buf[53], 0);
        assert_eq!(buf[54], 200);
        assert_eq!(buf[55], 100);
    }

    #[test]
    fn test_write_data_event_timestamp() {
        let mut buf = [0u8; 100];
        let frame = create_test_frame();
        write_data_event(&mut buf, &frame, 0, 0, 0, 9_876_543_210, false);

        assert_eq!(
            u64::from_le_bytes(buf[68..76].try_into().unwrap()),
            9_876_543_210
        );
    }

    #[test]
    fn test_write_data_event_accel_normal() {
        let mut buf = [0u8; 100];
        let frame = create_test_frame();
        write_data_event(&mut buf, &frame, 0, 0, 0, 0, false);

        assert_eq!(f32::from_le_bytes(buf[76..80].try_into().unwrap()), 1.0);
        assert_eq!(f32::from_le_bytes(buf[80..84].try_into().unwrap()), 0.5);
        assert_eq!(f32::from_le_bytes(buf[84..88].try_into().unwrap()), -0.5);
    }

    #[test]
    fn test_write_data_event_accel_inverted() {
        let mut buf = [0u8; 100];
        let frame = create_test_frame();
        write_data_event(&mut buf, &frame, 0, 0, 0, 0, true);

        assert_eq!(f32::from_le_bytes(buf[76..80].try_into().unwrap()), 1.0);
        assert_eq!(f32::from_le_bytes(buf[80..84].try_into().unwrap()), -0.5);
        assert_eq!(f32::from_le_bytes(buf[84..88].try_into().unwrap()), -0.5);
    }

    #[test]
    fn test_write_data_event_gyro_normal() {
        let mut buf = [0u8; 100];
        let frame = create_test_frame();
        write_data_event(&mut buf, &frame, 0, 0, 0, 0, false);

        assert_eq!(f32::from_le_bytes(buf[88..92].try_into().unwrap()), 10.0);
        assert_eq!(f32::from_le_bytes(buf[92..96].try_into().unwrap()), -5.0);
        assert_eq!(f32::from_le_bytes(buf[96..100].try_into().unwrap()), 2.5);
    }

    #[test]
    fn test_write_data_event_gyro_inverted() {
        let mut buf = [0u8; 100];
        let frame = create_test_frame();
        write_data_event(&mut buf, &frame, 0, 0, 0, 0, true);

        assert_eq!(f32::from_le_bytes(buf[88..92].try_into().unwrap()), -10.0);
        assert_eq!(f32::from_le_bytes(buf[92..96].try_into().unwrap()), 5.0);
        assert_eq!(f32::from_le_bytes(buf[96..100].try_into().unwrap()), 2.5);
    }
}
