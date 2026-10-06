use deskpuck_core::pairing::*;

const UUID: &str = "4A3E1C2B-9F10-4D7A-8C55-0123456789AB";

fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("temp dir")
}

#[test]
fn save_and_load_round_trip() {
    let dir = temp_dir();
    let path = dir.path().join("Deskpuck").join("pairing.json");
    let device = PairedDevice::new(UUID, Some("Joy-Con 2 (R)")).expect("valid");
    device.save(&path).expect("save");
    assert_eq!(PairedDevice::load(&path), Ok(Some(device)));

    // Replacing the pairing leaves one file and no temp files.
    let other = PairedDevice::new("hci0/dev_98_B6_E9_00_11_22", None).expect("valid");
    other.save(&path).expect("save again");
    assert_eq!(PairedDevice::load(&path), Ok(Some(other)));
    assert_eq!(std::fs::read_dir(path.parent().expect("parent")).expect("read dir").count(), 1);
}

#[test]
fn missing_file_is_not_paired() {
    assert_eq!(PairedDevice::load(&temp_dir().path().join("pairing.json")), Ok(None));
}

#[test]
fn new_rejects_bad_ids_and_cleans_names() {
    for bad in ["", "has space", "tab\there", "caf\u{e9}", &"x".repeat(MAX_ID_LEN + 1)] {
        assert_eq!(PairedDevice::new(bad, None), None, "{bad:?}");
    }
    assert!(PairedDevice::new(&"x".repeat(MAX_ID_LEN), None).is_some());

    let long = "N".repeat(MAX_NAME_CHARS + 10);
    let device = PairedDevice::new(UUID, Some(&format!("\x1b[2J{long}"))).expect("valid id");
    let name = device.name.clone().expect("name kept");
    assert!(name.chars().count() == MAX_NAME_CHARS && !name.contains('\x1b'), "{name:?}");
    // What new() builds always loads back.
    assert_eq!(PairedDevice::from_json(&device.to_json()), Ok(device));
    assert_eq!(PairedDevice::new(UUID, Some("\n\r")).expect("valid").name, None);
}

#[test]
fn malformed_records_are_rejected() {
    let cases = [
        ("not json", "not valid JSON"),
        ("[]", "not a JSON object"),
        (r#"{"version": 2, "id": "a"}"#, "version"),
        (r#"{"id": "a"}"#, "version"),
        (r#"{"version": 1}"#, "invalid id"),
        (r#"{"version": 1, "id": ""}"#, "invalid id"),
        (r#"{"version": 1, "id": 5}"#, "invalid id"),
        (r#"{"version": 1, "id": "a b"}"#, "invalid id"),
        (r#"{"version": 1, "id": "a", "name": 3}"#, "invalid name"),
        (r#"{"version": 1, "id": "a", "name": ""}"#, "invalid name"),
        (r#"{"version": 1, "id": "a", "name": "bell\u0007"}"#, "invalid name"),
        (r#"{"version": 1, "id": "a", "extra": true}"#, "unknown field"),
    ];
    for (json, why) in cases {
        let err = PairedDevice::from_json(json.as_bytes()).expect_err(json);
        assert!(err.contains(why), "{json}: {err}");
    }
    let long_name = format!(r#"{{"version": 1, "id": "a", "name": "{}"}}"#, "n".repeat(65));
    assert!(PairedDevice::from_json(long_name.as_bytes()).is_err());
    // Positive control: the minimal record and a null name load.
    for ok in [r#"{"version": 1, "id": "a"}"#, r#"{"version": 1, "id": "a", "name": null}"#] {
        assert_eq!(
            PairedDevice::from_json(ok.as_bytes()),
            Ok(PairedDevice::new("a", None).unwrap())
        );
    }
}

#[test]
fn load_reports_unusable_files_with_the_path() {
    let dir = temp_dir();
    let bad = dir.path().join("pairing.json");
    std::fs::write(&bad, "{").expect("write");
    let err = PairedDevice::load(&bad).expect_err("malformed");
    assert!(err.contains("pairing.json") && err.contains("not valid JSON"), "{err}");

    let mut big = format!(r#"{{"version": 1, "id": "{UUID}""#);
    while big.len() as u64 <= MAX_PAIRING_BYTES {
        big.push(' ');
    }
    big.push('}');
    std::fs::write(&bad, &big).expect("write big");
    let err = PairedDevice::load(&bad).expect_err("oversized");
    assert!(err.contains("larger than 4 KB"), "{err}");

    let err = PairedDevice::load(dir.path()).expect_err("directory");
    #[cfg(unix)]
    assert!(err.contains("not a regular file"), "{err}");
    let _ = err;
}

#[cfg(unix)]
#[test]
fn fifo_refused_without_blocking() {
    use std::os::unix::ffi::OsStrExt;
    let dir = temp_dir();
    let fifo = dir.path().join("pairing.json");
    let c_path = std::ffi::CString::new(fifo.as_os_str().as_bytes()).expect("path");
    // SAFETY: c_path is a valid NUL-terminated path for the duration of the call.
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || tx.send(PairedDevice::load(&fifo)).expect("send"));
    let result = rx.recv_timeout(std::time::Duration::from_secs(5)).expect("load did not block");
    assert!(result.is_err_and(|e| e.contains("not a regular file")));
}

#[cfg(unix)]
#[test]
fn saved_owner_only_and_symlink_replaced() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir();
    let victim = dir.path().join("victim.txt");
    std::fs::write(&victim, "untouched").expect("write victim");
    let link = dir.path().join("pairing.json");
    std::os::unix::fs::symlink(&victim, &link).expect("symlink");

    let device = PairedDevice::new(UUID, None).expect("valid");
    device.save(&link).expect("save over symlink");
    assert_eq!(std::fs::read_to_string(&victim).expect("read victim"), "untouched");
    let meta = std::fs::symlink_metadata(&link).expect("lstat");
    assert!(meta.file_type().is_file());
    assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    assert_eq!(PairedDevice::load(&link), Ok(Some(device)));
}

#[test]
fn default_path_sits_beside_the_config() {
    let pairing = PairedDevice::default_path().expect("home directory");
    let config = deskpuck_core::config::Config::default_path().expect("home directory");
    assert_eq!(pairing.parent(), config.parent());
    assert_eq!(pairing.file_name().and_then(|n| n.to_str()), Some("pairing.json"));
}
