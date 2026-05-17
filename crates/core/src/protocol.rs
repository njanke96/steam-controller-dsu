/// CRC32 used by the CemuHook protocol.
/// Matches the bit-by-bit algorithm from the reference C++ code.
pub fn crc32(data: &[u8]) -> u32 {
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
    buf[0..4].copy_from_slice(b"DSUS");
    buf[4..6].copy_from_slice(&1001u16.to_le_bytes());
    buf[6..8].copy_from_slice(&2u16.to_le_bytes()); // payload length
    buf[8..12].fill(0); // crc32 placeholder
    buf[12..16].copy_from_slice(&client_id.to_le_bytes());
    buf[16..20].copy_from_slice(&0x100000u32.to_le_bytes());
    buf[20..22].copy_from_slice(&1001u16.to_le_bytes());

    let c = crc32(&buf[..22]);
    buf[8..12].copy_from_slice(&c.to_le_bytes());
}

/// Build a CemuHook controller-info response packet into `buf`.
/// `buf` must be at least 32 bytes.
pub fn write_info_response(buf: &mut [u8], slot: u8, client_id: u32, connected: bool) {
    buf.fill(0);
    buf[0..4].copy_from_slice(b"DSUS");
    buf[4..6].copy_from_slice(&1001u16.to_le_bytes());
    buf[6..8].copy_from_slice(&16u16.to_le_bytes()); // payload length = 32 - 16
    buf[8..12].fill(0); // crc32 placeholder
    buf[12..16].copy_from_slice(&client_id.to_le_bytes());
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

/// Build a CemuHook data-event packet (100 bytes) from a `TritonFrame`.
/// Buttons / sticks / touch are zeroed — only motion data is populated.
///
/// # CemuHook wire layout (absolute offsets, 0-indexed)
/// ```text
///  0-3   : magic "DSUS"
///  4-5   : version (1001)
///  6-7   : length (84)
///  8-11  : crc32
///  12-15 : client id
///  16-19 : event type (0x100002)
///  20-30 : SharedResponse (11 bytes)
///  31    : connected (1)
///  32-35 : packetNumber
///  36-55 : buttons, sticks, analog buttons (20 bytes)
///  56-67 : touch data (12 bytes)
///  68-75 : timestamp (µs, u64 LE)
///  76-79 : accX (f32 LE)
///  80-83 : accY (f32 LE)
///  84-87 : accZ (f32 LE)
///  88-91 : pitch / gyroX (f32 LE)
///  92-95 : yaw   / gyroY (f32 LE)
///  96-99 : roll  / gyroZ (f32 LE)
/// ```
pub fn write_data_event(
    buf: &mut [u8; 100],
    frame: &crate::frame::TritonFrame,
    packet_num: u32,
    client_id: u32,
    slot: u8,
    timestamp_us: u64,
) {
    buf.fill(0);

    // Header (16 bytes)
    buf[0..4].copy_from_slice(b"DSUS");
    buf[4..6].copy_from_slice(&1001u16.to_le_bytes());
    buf[6..8].copy_from_slice(&84u16.to_le_bytes()); // 100 - 16 = 84
    buf[8..12].fill(0); // crc32 placeholder
    buf[12..16].copy_from_slice(&client_id.to_le_bytes());
    buf[16..20].copy_from_slice(&0x100002u32.to_le_bytes());

    // SharedResponse (11 bytes, offset 20)
    buf[20] = slot; // slot requested by client
    buf[21] = 2;    // slotState = connected
    buf[22] = 2;    // deviceModel = full gyro
    buf[23] = 1;    // connection = USB
    // mac1/mac2/battery already zero
    buf[31] = 1;    // connected

    // packetNumber (offset 32)
    buf[32..36].copy_from_slice(&packet_num.to_le_bytes());

    // Buttons, sticks, analog buttons, touch data — all zero (already cleared).

    // MotionData timestamp (offset 68)
    buf[68..76].copy_from_slice(&timestamp_us.to_le_bytes());

    // Accelerometer in g (offset 76).
    // The reference SteamDeckGyroDSU negates X and Y to match the Deck's
    // physical orientation relative to CemuHook expectations.
    let (ax, ay, az) = frame.accel_g();
    buf[76..80].copy_from_slice(&(-ax).to_le_bytes());
    buf[80..84].copy_from_slice(&(-ay).to_le_bytes());
    buf[84..88].copy_from_slice(&az.to_le_bytes());

    // Gyroscope in deg/s (offset 88).
    // Reference mapping: pitch = gyroX, yaw = -gyroY, roll = gyroZ.
    let (gx, gy, gz) = frame.gyro_dps();
    buf[88..92].copy_from_slice(&gx.to_le_bytes());
    buf[92..96].copy_from_slice(&(-gy).to_le_bytes());
    buf[96..100].copy_from_slice(&gz.to_le_bytes());

    let c = crc32(&buf[..100]);
    buf[8..12].copy_from_slice(&c.to_le_bytes());
}
