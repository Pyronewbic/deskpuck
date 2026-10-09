//! Nothing here starts Bluetooth: dp_controller_start is only exercised on
//! paths that fail before the controller thread exists.

use deskpuck_ble::controller::LinkStatus;
use deskpuck_ffi::*;
use std::collections::BTreeSet;
use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr;
use std::sync::Mutex;

const HEADER: &str = include_str!("../include/deskpuck.h");
const SOURCE: &str = include_str!("../src/lib.rs");

fn take(s: *mut c_char) -> Option<String> {
    if s.is_null() {
        return None;
    }
    // SAFETY: returned by this library, freed exactly once here.
    let out = unsafe { CStr::from_ptr(s) }.to_str().expect("UTF-8").to_owned();
    unsafe { dp_string_free(s) };
    Some(out)
}

fn c(s: &str) -> CString {
    CString::new(s).expect("no NUL")
}

fn json(s: *mut c_char) -> serde_json::Value {
    serde_json::from_str(&take(s).expect("a string")).expect("JSON")
}

fn problems(config: &str) -> Vec<String> {
    let c = c(config);
    serde_json::from_value(json(unsafe { dp_config_problems(c.as_ptr()) }))
        .expect("array of strings")
}

#[test]
fn header_matches_the_exports() {
    let declared: BTreeSet<&str> = HEADER
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|w| w.starts_with("dp_") && HEADER.contains(&format!("{w}(")))
        .collect();
    let exported: BTreeSet<&str> = SOURCE
        .split("extern \"C\" fn ")
        .skip(1)
        .filter_map(|rest| rest.split('(').next())
        .filter(|name| name.starts_with("dp_"))
        .collect();
    assert_eq!(exported.len(), 15, "{exported:?}");
    assert_eq!(declared, exported);
    assert!(HEADER.contains(&format!("#define DP_ABI_VERSION {DP_ABI_VERSION}\n")));
    assert_eq!(dp_abi_version(), DP_ABI_VERSION);
}

#[test]
fn header_status_values_match() {
    for (status, name) in [
        (LinkStatus::BluetoothOff, "DP_STATUS_BLUETOOTH_OFF"),
        (LinkStatus::BluetoothUnauthorized, "DP_STATUS_BLUETOOTH_UNAUTHORIZED"),
        (LinkStatus::Unavailable, "DP_STATUS_UNAVAILABLE"),
        (LinkStatus::Searching, "DP_STATUS_SEARCHING"),
        (LinkStatus::Connecting, "DP_STATUS_CONNECTING"),
        (LinkStatus::Connected, "DP_STATUS_CONNECTED"),
        (LinkStatus::NotPaired, "DP_STATUS_NOT_PAIRED"),
        (LinkStatus::Pairing, "DP_STATUS_PAIRING"),
        (LinkStatus::InUseElsewhere, "DP_STATUS_IN_USE_ELSEWHERE"),
    ] {
        let declared = format!("{name} = {},", status_code(status));
        assert!(HEADER.contains(&declared), "header lacks {declared}");
    }
    let window =
        format!("#define DP_PAIRING_SECONDS {:.0}\n", deskpuck_ble::receiver::PAIRING_WINDOW);
    assert!(HEADER.contains(&window), "header lacks {window}");
}

#[test]
fn defaults_are_valid_and_freeing_null_is_fine() {
    let defaults = take(dp_config_defaults()).expect("defaults");
    assert_eq!(problems(&defaults), Vec::<String>::new());
    assert_eq!(json(dp_config_defaults())["keyMappings"]["RS"], 36);
    unsafe { dp_string_free(ptr::null_mut()) };
    assert!(take(dp_config_default_path()).is_some_and(|p| p.ends_with("config.json")));
}

#[test]
fn problems_reject_bad_input() {
    assert_eq!(problems(r#"{"version": 1, "pointerSpeed": 50}"#).len(), 1);
    assert!(problems("{not json")[0].contains("not valid JSON"));
    assert!(problems(r#"{"pointerSpeed": 2}"#)[0].contains("version"));
    assert!(problems(r#"{"version": 1, "pointerSpeed": 2}"#).is_empty());

    let null = json(unsafe { dp_config_problems(ptr::null()) });
    assert!(null[0].as_str().is_some_and(|p| p.contains("missing argument")), "{null}");
    let bytes = [0xFFu8, 0xFE, 0];
    let not_utf8 = json(unsafe { dp_config_problems(bytes.as_ptr().cast()) });
    assert!(not_utf8[0].as_str().is_some_and(|p| p.contains("not UTF-8")), "{not_utf8}");
}

#[test]
fn save_and_load_round_trip() {
    let dir = std::env::temp_dir().join(format!("dp-ffi-{}", std::process::id()));
    let path = c(dir.join("Deskpuck/config.json").to_str().expect("utf-8"));
    let config = c(
        r#"{"version": 1, "keyMappings": {"A": 49}, "pointerSpeed": 2.5, "scrollEnabled": false}"#,
    );
    assert_eq!(take(unsafe { dp_config_save(config.as_ptr(), path.as_ptr()) }), None);

    let loaded = json(unsafe { dp_config_load(path.as_ptr()) });
    assert_eq!(loaded["warnings"], serde_json::json!([]));
    assert_eq!(loaded["config"]["keyMappings"], serde_json::json!({"A": 49}));
    assert_eq!(loaded["config"]["pointerSpeed"], 2.5);
    assert_eq!(loaded["config"]["scrollEnabled"], false);

    let bad = c(r#"{"version": 1, "pointerSpeed": 50}"#);
    let refused = take(unsafe { dp_config_save(bad.as_ptr(), path.as_ptr()) }).expect("refused");
    assert!(refused.contains("pointerSpeed"), "{refused}");
    assert_eq!(json(unsafe { dp_config_load(path.as_ptr()) })["config"]["pointerSpeed"], 2.5);

    assert!(take(unsafe { dp_config_save(ptr::null(), path.as_ptr()) }).is_some());
    assert!(take(unsafe { dp_config_save(config.as_ptr(), ptr::null()) }).is_some());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn load_falls_back_on_bad_paths() {
    let missing = c("/nonexistent/deskpuck/config.json");
    let loaded = json(unsafe { dp_config_load(missing.as_ptr()) });
    assert_eq!(loaded["warnings"], serde_json::json!([]));
    assert_eq!(loaded["config"]["keyMappings"]["RS"], 36);

    let null = json(unsafe { dp_config_load(ptr::null()) });
    assert_eq!(null["warnings"].as_array().map(Vec::len), Some(1));
    assert_eq!(null["config"]["version"], 1);
}

#[test]
fn update_evaluate_crosses_and_tolerates_null() {
    let newer = c("https://github.com/Pyronewbic/deskpuck/releases/tag/v0.4.0");
    let foreign = c("https://github.com/Attacker/deskpuck/releases/tag/v9.9.9");
    let current = c("0.3.0");
    assert_eq!(
        take(unsafe { dp_update_evaluate(newer.as_ptr(), current.as_ptr()) }).as_deref(),
        Some("0.4.0")
    );
    assert_eq!(take(unsafe { dp_update_evaluate(foreign.as_ptr(), current.as_ptr()) }), None);
    assert_eq!(take(unsafe { dp_update_evaluate(ptr::null(), current.as_ptr()) }), None);
    assert_eq!(take(unsafe { dp_update_evaluate(newer.as_ptr(), ptr::null()) }), None);
    let not_utf8 = CString::new(vec![0xFF, 0xFE]).expect("no NUL");
    assert_eq!(take(unsafe { dp_update_evaluate(not_utf8.as_ptr(), current.as_ptr()) }), None);
}

#[test]
fn controller_functions_tolerate_null() {
    unsafe {
        dp_controller_set_paused(ptr::null_mut(), true);
        dp_controller_start_pairing(ptr::null_mut());
        dp_controller_cancel_pairing(ptr::null_mut());
        assert!(!dp_controller_is_paused(ptr::null()));
        let config = c(r#"{"version": 1}"#);
        let err = take(dp_controller_apply_config(ptr::null_mut(), config.as_ptr()));
        assert_eq!(err.as_deref(), Some("No controller."));
        dp_controller_free(ptr::null_mut());
    }
}

#[test]
fn invalid_config_never_starts_a_controller() {
    for bad in [r#"{"version": 2}"#, "{not json", r#"{"version": 1, "pointerSpeed": 0}"#] {
        let config = c(bad);
        let controller =
            unsafe { dp_controller_start(config.as_ptr(), None, None, ptr::null_mut()) };
        assert!(controller.is_null(), "{bad}");
    }
    assert!(unsafe { dp_controller_start(ptr::null(), None, None, ptr::null_mut()) }.is_null());
}

static CALLS: Mutex<Vec<(usize, i32, Option<String>)>> = Mutex::new(Vec::new());

extern "C" fn record(context: *mut c_void, status: i32, name: *const c_char) {
    let name =
        (!name.is_null()).then(|| unsafe { CStr::from_ptr(name) }.to_string_lossy().into_owned());
    CALLS.lock().unwrap().push((context as usize, status, name));
}

#[test]
fn status_reaches_the_callback_from_another_thread() {
    let mut hook = status_hook(Some(record), 0xC0FFEE);
    std::thread::spawn(move || {
        hook(LinkStatus::Connected, Some("Joy-Con 2 (R)"));
        hook(LinkStatus::Searching, None);
        hook(LinkStatus::Connecting, Some("bad\0name"));
    })
    .join()
    .expect("thread");
    let calls = CALLS.lock().unwrap().clone();
    assert_eq!(
        calls,
        [
            (0xC0FFEE, 5, Some("Joy-Con 2 (R)".into())),
            (0xC0FFEE, 3, None),
            (0xC0FFEE, 4, Some("badname".into())),
        ]
    );
    status_hook(None, 0)(LinkStatus::Connected, Some("x"));
}

#[test]
fn shortcuts_cross_as_json() {
    let copy = r#"{"version": 1, "keyMappings": {"A": {"key": 8, "modifiers": ["control"]}}}"#;
    assert_eq!(problems(copy), Vec::<String>::new());
    let bad =
        problems(r#"{"version": 1, "keyMappings": {"A": {"key": 8, "modifiers": ["meta"]}}}"#);
    assert!(bad.len() == 1 && bad[0].contains("Unknown modifier \"meta\""), "{bad:?}");
}

#[test]
fn modifier_bits_match_the_header() {
    use deskpuck_core::mapping::Modifiers;
    for (name, modifier, _) in Modifiers::ALL {
        let define = format!("#define DP_MODIFIER_{} {}\n", name.to_uppercase(), modifier.bits());
        assert!(HEADER.contains(&define), "header lacks {define}");
    }
}

static LATCHES: Mutex<Vec<(usize, u32)>> = Mutex::new(Vec::new());

extern "C" fn record_latch(context: *mut c_void, modifiers: u32) {
    LATCHES.lock().unwrap().push((context as usize, modifiers));
}

#[test]
fn latch_changes_reach_the_callback() {
    use deskpuck_core::mapping::Modifiers;
    let mut hook = latch_hook(Some(record_latch), 0xBEEF).expect("a hook");
    std::thread::spawn(move || {
        hook(Modifiers::SHIFT.with(Modifiers::COMMAND));
        hook(Modifiers::NONE);
    })
    .join()
    .expect("thread");
    assert_eq!(*LATCHES.lock().unwrap(), [(0xBEEF, 12), (0xBEEF, 0)]);
    assert!(latch_hook(None, 0).is_none());
}
