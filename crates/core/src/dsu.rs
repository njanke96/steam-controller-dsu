/// CRC32 used by the CemuHook protocol.
/// Matches the algorithm from SteamDeckGyroDSU.
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
/// Buttons / sticks / touch are intentionally zeroed
pub fn write_data_event(
    buf: &mut [u8; 100],
    frame: &crate::frame::TritonFrame, // TODO: Use a more agnostic struct for frame data, not device specific
    packet_num: u32,
    client_id: u32,
    slot: u8,
    timestamp_us: u64,
    invert_y: bool,
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
    buf[21] = 2; // slotState = connected
    buf[22] = 2; // deviceModel = full gyro
    buf[23] = 1; // connection = USB
    // mac1/mac2/battery already zero
    buf[31] = 1; // connected

    // packetNumber (offset 32)
    buf[32..36].copy_from_slice(&packet_num.to_le_bytes());

    // Buttons, sticks, analog buttons, touch data are already zeroed

    // MotionData timestamp (offset 68)
    buf[68..76].copy_from_slice(&timestamp_us.to_le_bytes());

    // Accelerometer in g (offset 76).
    // Steam Controller IMU orientation matches CemuHook expectations.
    // With invert_y: accY is not negated (opposite of Nintendo Switch).
    let (ax, ay, az) = frame.accel_g();
    buf[76..80].copy_from_slice(&(-ax).to_le_bytes());
    let acc_y = if invert_y { ay } else { -ay };
    buf[80..84].copy_from_slice(&acc_y.to_le_bytes());
    buf[84..88].copy_from_slice(&az.to_le_bytes());

    // Gyroscope in deg/s (offset 88).
    // Steam Controller mapping: yaw = -gyroY, roll = gyroZ
    // Pitch: -gx (Nintendo Switch style) or gx (invert_y).
    let (gx, gy, gz) = frame.gyro_dps();
    let pitch = if invert_y { gx } else { -gx };
    buf[88..92].copy_from_slice(&pitch.to_le_bytes());
    buf[92..96].copy_from_slice(&(-gy).to_le_bytes());
    buf[96..100].copy_from_slice(&gz.to_le_bytes());

    let c = crc32(&buf[..100]);
    buf[8..12].copy_from_slice(&c.to_le_bytes());
}
