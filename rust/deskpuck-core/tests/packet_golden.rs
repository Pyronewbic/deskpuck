use deskpuck_core::packet::parse_report;

const FIXTURE: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/joycon2_r_capture.txt");

fn hex_bytes(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0, "odd hex length");
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex byte"))
        .collect()
}

// Every captured report must decode to what the original upstream parser printed.
#[test]
fn captured_reports_match_upstream_output() {
    let text = std::fs::read_to_string(FIXTURE).expect("fixture readable");
    let mut records = 0;
    for line in text.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let (hex, expected) = line.split_once(" | ").expect("hex | expected");
        let bytes = hex_bytes(hex);
        assert_eq!(bytes.len(), 63);

        let f: Vec<&str> = expected.split_whitespace().collect();
        assert_eq!(f.len(), 22, "field count in {line}");
        let n = |i: usize| f[i].parse::<i64>().expect("integer field");

        let r = parse_report(&bytes).expect("captured report parses");
        assert_eq!(i64::from(r.packet_id), n(0));
        assert_eq!(r.buttons, u32::from_str_radix(f[1], 16).expect("hex buttons"));
        assert_eq!((i64::from(r.trigger_l), i64::from(r.trigger_r)), (n(2), n(3)));
        assert_eq!((i64::from(r.left_stick_x), i64::from(r.left_stick_y)), (n(4), n(5)));
        assert_eq!((i64::from(r.right_stick_x), i64::from(r.right_stick_y)), (n(6), n(7)));
        assert_eq!([r.accel_x, r.accel_y, r.accel_z].map(i64::from), [n(8), n(9), n(10)]);
        assert_eq!([r.gyro_x, r.gyro_y, r.gyro_z].map(i64::from), [n(11), n(12), n(13)]);
        assert_eq!([r.mag_x, r.mag_y, r.mag_z].map(i64::from), [n(14), n(15), n(16)]);
        assert_eq!((i64::from(r.mouse_x), i64::from(r.mouse_y)), (n(17), n(18)));
        assert_eq!(format!("{:.2}", r.battery_voltage()), f[19]);
        assert_eq!(format!("{:.2}", r.battery_current()), f[20]);
        assert_eq!(format!("{:.1}", r.temperature()), f[21]);
        records += 1;
    }
    assert_eq!(records, 18);
}
