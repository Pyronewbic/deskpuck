/// Smallest input report that contains every field below (triggers end at 0x3D).
pub const REPORT_MIN_SIZE: usize = 0x3E;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub packet_id: u32,
    pub buttons: u32,
    pub left_stick_x: u16,
    pub left_stick_y: u16,
    pub right_stick_x: u16,
    pub right_stick_y: u16,
    pub mouse_x: i16,
    pub mouse_y: i16,
    pub mouse_unknown: i16,
    pub mouse_distance: i16,
    pub mag_x: i16,
    pub mag_y: i16,
    pub mag_z: i16,
    pub battery_voltage_raw: u16,
    pub battery_current_raw: i16,
    pub temperature_raw: i16,
    pub accel_x: i16,
    pub accel_y: i16,
    pub accel_z: i16,
    pub gyro_x: i16,
    pub gyro_y: i16,
    pub gyro_z: i16,
    pub trigger_l: u8,
    pub trigger_r: u8,
}

impl Report {
    pub fn battery_voltage(&self) -> f32 {
        f32::from(self.battery_voltage_raw) / 1000.0
    }

    pub fn battery_current(&self) -> f32 {
        f32::from(self.battery_current_raw) / 100.0
    }

    pub fn temperature(&self) -> f32 {
        25.0 + f32::from(self.temperature_raw) / 127.0
    }
}

// Little-endian reads; callers guarantee offset + width <= data.len().
pub fn read_i16_le(data: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes([data[offset], data[offset + 1]])
}

pub fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

pub fn read_u24_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], 0])
}

pub fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
}

/// Two 12-bit values packed into 3 bytes: X in the low 12 bits, Y in the high 12.
pub fn parse_stick(data: &[u8], offset: usize) -> (u16, u16) {
    let packed = read_u24_le(data, offset);
    ((packed & 0xFFF) as u16, ((packed >> 12) & 0xFFF) as u16)
}

/// Returns `None` if the report is too short to hold every field.
pub fn parse_report(data: &[u8]) -> Option<Report> {
    if data.len() < REPORT_MIN_SIZE {
        return None;
    }
    let (left_stick_x, left_stick_y) = parse_stick(data, 0x0A);
    let (right_stick_x, right_stick_y) = parse_stick(data, 0x0D);
    Some(Report {
        packet_id: read_u24_le(data, 0x00),
        buttons: read_u32_le(data, 0x03),
        left_stick_x,
        left_stick_y,
        right_stick_x,
        right_stick_y,
        mouse_x: read_i16_le(data, 0x10),
        mouse_y: read_i16_le(data, 0x12),
        mouse_unknown: read_i16_le(data, 0x14),
        mouse_distance: read_i16_le(data, 0x16),
        mag_x: read_i16_le(data, 0x18),
        mag_y: read_i16_le(data, 0x1A),
        mag_z: read_i16_le(data, 0x1C),
        battery_voltage_raw: read_u16_le(data, 0x1F),
        battery_current_raw: read_i16_le(data, 0x28),
        temperature_raw: read_i16_le(data, 0x2E),
        accel_x: read_i16_le(data, 0x30),
        accel_y: read_i16_le(data, 0x32),
        accel_z: read_i16_le(data, 0x34),
        gyro_x: read_i16_le(data, 0x36),
        gyro_y: read_i16_le(data, 0x38),
        gyro_z: read_i16_le(data, 0x3A),
        trigger_l: data[0x3C],
        trigger_r: data[0x3D],
    })
}

const BUTTONS: [(u32, &str); 23] = [
    (0x0000_0100, "Y"),
    (0x0000_0200, "X"),
    (0x0000_0400, "B"),
    (0x0000_0800, "A"),
    (0x0000_1000, "SR"),
    (0x0000_2000, "SL"),
    (0x0000_4000, "R"),
    (0x0000_8000, "ZR"),
    (0x0001_0000, "MINUS"),
    (0x0002_0000, "PLUS"),
    (0x0004_0000, "RS"),
    (0x0008_0000, "LS"),
    (0x0010_0000, "HOME"),
    (0x0020_0000, "CAPTURE"),
    (0x0040_0000, "CHAT"),
    (0x0100_0000, "DOWN"),
    (0x0200_0000, "UP"),
    (0x0400_0000, "RIGHT"),
    (0x0800_0000, "LEFT"),
    (0x1000_0000, "SR_L"),
    (0x2000_0000, "SL_L"),
    (0x4000_0000, "L"),
    (0x8000_0000, "ZL"),
];

pub fn button_names(buttons: u32) -> Vec<&'static str> {
    BUTTONS.iter().filter(|(mask, _)| buttons & mask != 0).map(|&(_, name)| name).collect()
}

/// Single-button lookup by one of the names above; `None` if unknown.
pub fn button_mask(name: &str) -> Option<u32> {
    BUTTONS.iter().find(|&&(_, n)| n == name).map(|&(mask, _)| mask)
}

/// Name of a single-button mask; `None` for unknown or multi-bit masks.
pub fn button_name(mask: u32) -> Option<&'static str> {
    BUTTONS.iter().find(|&&(m, _)| m == mask).map(|&(_, name)| name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn little_endian_reads() {
        assert_eq!(read_i16_le(&[0x00, 0x80], 0), -32768);
        assert_eq!(read_i16_le(&[0xFF, 0x7F], 0), 32767);
        assert_eq!(read_i16_le(&[0xFF, 0xFF], 0), -1);
        assert_eq!(read_u16_le(&[0xFF, 0xFF], 0), 0xFFFF);

        let word = [0x01, 0x02, 0x03, 0x04];
        assert_eq!(read_u16_le(&word, 1), 0x0302);
        assert_eq!(read_u24_le(&word, 0), 0x03_0201);
        assert_eq!(read_u32_le(&word, 0), 0x0403_0201);
        assert_eq!(read_u32_le(&[0x00, 0x00, 0x00, 0x80], 0), 0x8000_0000);
    }

    #[test]
    fn stick_nibbles() {
        assert_eq!(parse_stick(&[0xFF, 0x0F, 0x00], 0), (0xFFF, 0));
        assert_eq!(parse_stick(&[0x00, 0xF0, 0xFF], 0), (0, 0xFFF));
        // The middle byte is split: its low nibble belongs to X, its high nibble to Y.
        assert_eq!(parse_stick(&[0x21, 0x43, 0x65], 0), (0x321, 0x654));
    }

    #[test]
    fn synthetic_fields() {
        let mut bytes = vec![0u8; 63];
        // Buttons at 0x03: ZL plus noise in the low byte.
        bytes[0x03] = 0xFF;
        bytes[0x06] = 0x80;
        // Mouse X = -2, Y = 300, distance = 7.
        bytes[0x10] = 0xFE;
        bytes[0x11] = 0xFF;
        bytes[0x12] = 0x2C;
        bytes[0x13] = 0x01;
        bytes[0x16] = 0x07;
        bytes[0x3C] = 0x11;
        bytes[0x3D] = 0x22;

        let r = parse_report(&bytes).expect("63-byte report parses");
        assert_eq!(r.buttons, 0x8000_00FF);
        assert_eq!((r.mouse_x, r.mouse_y, r.mouse_distance), (-2, 300, 7));
        assert_eq!((r.trigger_l, r.trigger_r), (0x11, 0x22));
    }

    #[test]
    fn short_reports_rejected() {
        // 60 and 61 bytes pass a ">= 60" check but the triggers live at 0x3C and 0x3D.
        for size in [0usize, 1, 59, 60, 61] {
            assert_eq!(parse_report(&vec![0xAB; size]), None, "size {size}");
        }
        // Positive control: the smallest report holding both trigger bytes is accepted.
        let mut smallest = vec![0u8; 62];
        smallest[0x3D] = 0x7F;
        assert_eq!(parse_report(&smallest).map(|r| r.trigger_r), Some(0x7F));
    }

    #[test]
    fn names_of_button_sets() {
        assert!(button_names(0).is_empty());
        assert!(button_names(0x0000_00FF).is_empty());
        assert_eq!(button_names(0x0000_0800), ["A"]);
        assert_eq!(button_names(0x0004_0200), ["X", "RS"]);
        assert_eq!(button_names(u32::MAX).len(), 23);
    }

    #[test]
    fn single_button_lookups() {
        assert_eq!(button_mask("RS"), Some(0x0004_0000));
        assert_eq!(button_mask("ZL"), Some(0x8000_0000));
        assert_eq!(button_name(0x0000_0800), Some("A"));

        assert_eq!(button_mask("Q"), None);
        assert_eq!(button_mask("rs"), None);
        assert_eq!(button_mask(""), None);
        assert_eq!(button_name(0x0000_0C00), None);
        assert_eq!(button_name(0x0000_00FF), None);

        for name in button_names(u32::MAX) {
            assert_eq!(button_mask(name).and_then(button_name), Some(name));
        }
    }
}
