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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_u32_masked_button_pressed_when_pressed() {
        assert!(is_u32_masked_button_pressed(0b00000001, 0b00000001));
        assert!(is_u32_masked_button_pressed(0b11111111, 0b00000001));
        assert!(is_u32_masked_button_pressed(0xFFFFFFFF, 0x80000000));
    }

    #[test]
    fn test_is_u32_masked_button_pressed_when_not_pressed() {
        assert!(!is_u32_masked_button_pressed(0b00000000, 0b00000001));
        assert!(!is_u32_masked_button_pressed(0b11111110, 0b00000001));
        assert!(!is_u32_masked_button_pressed(0x7FFFFFFF, 0x80000000));
    }

    #[test]
    fn test_scale_stick_to_byte_center() {
        assert_eq!(scale_stick_to_byte(0), 128);
    }

    #[test]
    fn test_scale_stick_to_byte_min() {
        let result = scale_stick_to_byte(-32768);
        assert!(result < 1);
    }

    #[test]
    fn test_scale_stick_to_byte_max() {
        let result = scale_stick_to_byte(32767);
        assert!(result > 254);
    }

    #[test]
    fn test_scale_trigger_to_byte_min() {
        assert_eq!(scale_trigger_to_byte(-32767), 0);
    }

    #[test]
    fn test_scale_trigger_to_byte_max() {
        assert_eq!(scale_trigger_to_byte(32767), 255);
    }
}
