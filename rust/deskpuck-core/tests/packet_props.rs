use deskpuck_core::packet::{REPORT_MIN_SIZE, Report, parse_report};
use proptest::prelude::*;

// Encodes a report at the documented offsets independently of the parser's readers.
fn encode(r: &Report, len: usize, filler: u8) -> Vec<u8> {
    let mut b = vec![filler; len];
    let mut put =
        |offset: usize, bytes: &[u8]| b[offset..offset + bytes.len()].copy_from_slice(bytes);
    put(0x00, &r.packet_id.to_le_bytes()[..3]);
    put(0x03, &r.buttons.to_le_bytes());
    let left = u32::from(r.left_stick_x) | (u32::from(r.left_stick_y) << 12);
    let right = u32::from(r.right_stick_x) | (u32::from(r.right_stick_y) << 12);
    put(0x0A, &left.to_le_bytes()[..3]);
    put(0x0D, &right.to_le_bytes()[..3]);
    for (offset, v) in [
        (0x10, r.mouse_x),
        (0x12, r.mouse_y),
        (0x14, r.mouse_unknown),
        (0x16, r.mouse_distance),
        (0x18, r.mag_x),
        (0x1A, r.mag_y),
        (0x1C, r.mag_z),
        (0x28, r.battery_current_raw),
        (0x2E, r.temperature_raw),
        (0x30, r.accel_x),
        (0x32, r.accel_y),
        (0x34, r.accel_z),
        (0x36, r.gyro_x),
        (0x38, r.gyro_y),
        (0x3A, r.gyro_z),
    ] {
        put(offset, &v.to_le_bytes());
    }
    put(0x1F, &r.battery_voltage_raw.to_le_bytes());
    put(0x3C, &[r.trigger_l, r.trigger_r]);
    b
}

prop_compose! {
    fn any_report()(
        packet_id in 0u32..1 << 24,
        buttons in any::<u32>(),
        sticks in [0u16..1 << 12, 0u16..1 << 12, 0u16..1 << 12, 0u16..1 << 12],
        words in any::<[i16; 15]>(),
        battery_voltage_raw in any::<u16>(),
        triggers in any::<[u8; 2]>(),
    ) -> Report {
        Report {
            packet_id,
            buttons,
            left_stick_x: sticks[0],
            left_stick_y: sticks[1],
            right_stick_x: sticks[2],
            right_stick_y: sticks[3],
            mouse_x: words[0],
            mouse_y: words[1],
            mouse_unknown: words[2],
            mouse_distance: words[3],
            mag_x: words[4],
            mag_y: words[5],
            mag_z: words[6],
            battery_voltage_raw,
            battery_current_raw: words[7],
            temperature_raw: words[8],
            accel_x: words[9],
            accel_y: words[10],
            accel_z: words[11],
            gyro_x: words[12],
            gyro_y: words[13],
            gyro_z: words[14],
            trigger_l: triggers[0],
            trigger_r: triggers[1],
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
        let parsed = parse_report(&bytes);
        prop_assert_eq!(parsed.is_some(), bytes.len() >= REPORT_MIN_SIZE);
    }

    #[test]
    fn encoded_reports_round_trip(
        report in any_report(),
        extra in 0usize..64,
        filler in any::<u8>(),
    ) {
        let bytes = encode(&report, REPORT_MIN_SIZE + extra, filler);
        prop_assert_eq!(parse_report(&bytes), Some(report));
    }

    #[test]
    fn trailing_bytes_are_ignored(
        bytes in proptest::collection::vec(any::<u8>(), REPORT_MIN_SIZE..=REPORT_MIN_SIZE),
        tail in proptest::collection::vec(any::<u8>(), 1..64),
    ) {
        let mut longer = bytes.clone();
        longer.extend(tail);
        prop_assert_eq!(parse_report(&longer), parse_report(&bytes));
    }
}
