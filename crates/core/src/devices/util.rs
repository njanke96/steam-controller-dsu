/// Returns true if a button in `buttons` is pressed according to `mask`
pub fn is_u32_masked_button_pressed(buttons: u32, mask: u32) -> bool {
    buttons & mask != 0
}

/// Convert a signed 16-bit stick axis to an unsigned 8-bit DSU value
pub fn scale_stick_to_byte(axis: i16) -> u8 {
    let normalized = (axis as f32 / 256.0) + 128.0;
    normalized.clamp(0.0, 255.0) as u8
}

/// Convert a signed 16-bit trigger axis to an unsigned 8-bit DSU value
pub fn scale_trigger_to_byte(axis: i16) -> u8 {
    let clamped = axis.max(0) as f32;
    let normalized = (clamped / 32767.0) * 255.0;
    normalized.clamp(0.0, 255.0) as u8
}
