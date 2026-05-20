//! Provides functionality for working with DSU protocol data.

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
    pub accel_x: f32,
    pub accel_y: f32,
    pub accel_z: f32,
    pub gyro_x: f32,
    pub gyro_y: f32,
    pub gyro_z: f32,
}

/// Write the common CemuHook packet header into `buf`
///
/// `buf` must be at least 16 bytes
/// The CRC32 field (bytes 8..12) is zeroed so the caller can compute it after
/// filling the payload.
fn write_header(buf: &mut [u8], payload_len: u16, client_id: u32) {
    buf[0..4].copy_from_slice(b"DSUS");
    buf[4..6].copy_from_slice(&1001u16.to_le_bytes());
    buf[6..8].copy_from_slice(&payload_len.to_le_bytes());
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
/// `buf` must be at least 22 bytes.
pub fn write_version_response(buf: &mut [u8], client_id: u32) {
    write_header(buf, 2, client_id);
    buf[16..20].copy_from_slice(&0x100000u32.to_le_bytes());
    buf[20..22].copy_from_slice(&1001u16.to_le_bytes());

    let c = crc32(&buf[..22]);
    buf[8..12].copy_from_slice(&c.to_le_bytes());
}

/// Build a CemuHook controller-info response packet into `buf`.
/// `buf` must be at least 32 bytes.
pub fn write_info_response(buf: &mut [u8], slot: u8, client_id: u32, connected: bool) {
    buf.fill(0);
    write_header(buf, 16, client_id); // payload length = 32 - 16
    buf[16..20].copy_from_slice(&0x100001u32.to_le_bytes());

    // SharedResponse
    buf[20] = slot;
    if connected {
        buf[21] = 2; // slotState = connected
        buf[22] = 2; // deviceModel = full gyro
        buf[23] = 1; // connection = USB
    }
    // Info response: byte 31 is a zero byte (not a connected flag).

    let c = crc32(&buf[..32]);
    buf[8..12].copy_from_slice(&c.to_le_bytes());
}

/// Build a CemuHook data-event packet (100 bytes) from a `DSUFrame`.
pub fn write_data_event(
    buf: &mut [u8; 100],
    frame: &DSUFrame,
    packet_num: u32,
    client_id: u32,
    slot: u8,
    timestamp_us: u64,
    invert_pitch: bool,
) {
    buf.fill(0);

    write_header(buf, 84, client_id); // 100 - 16 = 84
    buf[16..20].copy_from_slice(&0x100002u32.to_le_bytes());

    // SharedResponse (11 bytes, offset 20)
    buf[20] = slot; // slot requested by client
    buf[21] = 2; // slotState = connected
    buf[22] = 2; // deviceModel = full gyro
    buf[23] = 1; // connection = USB
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

    let c = crc32(&buf[..100]);
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
