//! Settings cross as JSON so the Rust validator stays the only one.

use deskpuck_ble::controller::{
    Controller, Hooks, LatchHook, LinkStatus, PairingSetup, StatusHook,
};
use deskpuck_core::config::Config;
use deskpuck_core::pairing::PairedDevice;
use std::ffi::{CStr, CString, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::ptr;

pub const DP_ABI_VERSION: u32 = 4;

pub type StatusCallback =
    extern "C" fn(context: *mut c_void, status: i32, device_name: *const c_char);
pub type LatchCallback = extern "C" fn(context: *mut c_void, modifiers: u32);

/// Matches `dp_status` in the header.
pub fn status_code(status: LinkStatus) -> i32 {
    match status {
        LinkStatus::BluetoothOff => 0,
        LinkStatus::BluetoothUnauthorized => 1,
        LinkStatus::Unavailable => 2,
        LinkStatus::Searching => 3,
        LinkStatus::Connecting => 4,
        LinkStatus::Connected => 5,
        LinkStatus::NotPaired => 6,
        LinkStatus::Pairing => 7,
        LinkStatus::InUseElsewhere => 8,
    }
}

/// Bridges status changes to the C callback. The context travels as an
/// integer because pointers are not Send; the caller keeps it alive.
pub fn status_hook(on_status: Option<StatusCallback>, context: usize) -> StatusHook {
    Box::new(move |status: LinkStatus, name: Option<&str>| {
        if let Some(callback) = on_status {
            let name = name.and_then(|n| CString::new(n.replace('\0', "")).ok());
            callback(
                context as *mut c_void,
                status_code(status),
                name.as_ref().map_or(ptr::null(), |n| n.as_ptr()),
            );
        }
    })
}

pub fn latch_hook(on_latch: Option<LatchCallback>, context: usize) -> Option<LatchHook> {
    let callback = on_latch?;
    Some(Box::new(move |modifiers| {
        callback(context as *mut c_void, u32::from(modifiers.bits()));
    }))
}

fn into_c(s: String) -> *mut c_char {
    // Interior NULs cannot cross a C string; drop them rather than fail.
    CString::new(s.replace('\0', "")).map_or(ptr::null_mut(), CString::into_raw)
}

/// # Safety
/// `s` must be NULL or a valid NUL-terminated string.
unsafe fn from_c<'a>(s: *const c_char) -> Result<&'a str, String> {
    if s.is_null() {
        return Err("missing argument".into());
    }
    // SAFETY: non-null and NUL-terminated per the caller's contract.
    unsafe { CStr::from_ptr(s) }.to_str().map_err(|_| "argument is not UTF-8".into())
}

/// The fallback is built only after a panic: an eager one that allocates would leak.
fn guard<T>(fallback: impl FnOnce() -> T, body: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or_else(|_| fallback())
}

/// Strict: any warning is a problem.
fn parse_config(json: &str) -> Result<Config, Vec<String>> {
    let (config, warnings) = Config::from_json(json.as_bytes());
    if warnings.is_empty() { Ok(config) } else { Err(warnings) }
}

#[unsafe(no_mangle)]
pub extern "C" fn dp_abi_version() -> u32 {
    DP_ABI_VERSION
}

/// # Safety
/// `string` must be NULL or a pointer returned by this library, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dp_string_free(string: *mut c_char) {
    if !string.is_null() {
        // SAFETY: allocated by CString::into_raw in this library.
        drop(unsafe { CString::from_raw(string) });
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dp_config_default_path() -> *mut c_char {
    guard(ptr::null_mut, || match Config::default_path() {
        Some(path) => into_c(path.to_string_lossy().into_owned()),
        None => ptr::null_mut(),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dp_config_defaults() -> *mut c_char {
    guard(ptr::null_mut, || into_c(Config::default().to_value().to_string()))
}

/// # Safety
/// `path` must be NULL or a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dp_config_load(path: *const c_char) -> *mut c_char {
    guard(ptr::null_mut, || {
        // SAFETY: forwarded from the caller's contract.
        let (config, warnings) = match unsafe { from_c(path) } {
            Ok(path) => Config::load(Path::new(path)),
            Err(e) => (Config::default(), vec![format!("Config path: {e}; using defaults.")]),
        };
        into_c(serde_json::json!({ "config": config.to_value(), "warnings": warnings }).to_string())
    })
}

/// # Safety
/// `config_json` must be NULL or a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dp_config_problems(config_json: *const c_char) -> *mut c_char {
    guard(ptr::null_mut, || {
        // SAFETY: forwarded from the caller's contract.
        let problems = match unsafe { from_c(config_json) } {
            Ok(json) => parse_config(json).err().unwrap_or_default(),
            Err(e) => vec![format!("Config: {e}.")],
        };
        into_c(serde_json::Value::from(problems).to_string())
    })
}

/// # Safety
/// Both arguments must be NULL or valid NUL-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dp_config_save(
    config_json: *const c_char,
    path: *const c_char,
) -> *mut c_char {
    guard(
        || into_c("Could not save the config.".into()),
        || {
            // SAFETY: forwarded from the caller's contract.
            let (json, path) = match unsafe { (from_c(config_json), from_c(path)) } {
                (Ok(json), Ok(path)) => (json, path),
                (Err(e), _) | (_, Err(e)) => {
                    return into_c(format!("Could not save the config: {e}."));
                }
            };
            match parse_config(json) {
                Err(problems) => into_c(problems.join(" ")),
                Ok(config) => match config.save(Path::new(path)) {
                    Ok(()) => ptr::null_mut(),
                    Err(e) => into_c(e.to_string()),
                },
            }
        },
    )
}

pub struct DpController {
    controller: Controller,
}

/// # Safety
/// `config_json` as for `from_c`; `context` must stay valid until `dp_controller_free` returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dp_controller_start(
    config_json: *const c_char,
    on_status: Option<StatusCallback>,
    on_latch: Option<LatchCallback>,
    context: *mut c_void,
) -> *mut DpController {
    guard(ptr::null_mut, || {
        // SAFETY: forwarded from the caller's contract.
        let Ok(config) = unsafe { from_c(config_json) }.map_err(|e| vec![e]).and_then(parse_config)
        else {
            return ptr::null_mut();
        };
        let hooks = Hooks {
            status: status_hook(on_status, context as usize),
            report: None,
            log: None,
            error: Box::new(|message| eprintln!("deskpuck: {message}")),
            latched: latch_hook(on_latch, context as usize),
        };
        #[cfg(target_os = "macos")]
        let sink = deskpuck_inject::macos::MacSink::unchecked();
        #[cfg(not(target_os = "macos"))]
        let Ok(sink) = deskpuck_inject::platform_sink() else { return ptr::null_mut() };
        let pairing = PairingSetup { file: PairedDevice::default_path(), pair_at_start: false };
        match Controller::start(config.engine_settings(), sink, hooks, pairing) {
            Ok(controller) => Box::into_raw(Box::new(DpController { controller })),
            Err(_) => ptr::null_mut(),
        }
    })
}

/// # Safety
/// `controller` must be NULL or a live pointer from `dp_controller_start`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dp_controller_set_paused(controller: *mut DpController, paused: bool) {
    guard(
        || (),
        || {
            // SAFETY: live per the caller's contract.
            if let Some(c) = unsafe { controller.as_ref() } {
                c.controller.set_paused(paused);
            }
        },
    )
}

/// # Safety
/// `controller` must be NULL or a live pointer from `dp_controller_start`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dp_controller_is_paused(controller: *const DpController) -> bool {
    // SAFETY: live per the caller's contract.
    guard(|| false, || unsafe { controller.as_ref() }.is_some_and(|c| c.controller.is_paused()))
}

/// # Safety
/// `controller` must be NULL or a live pointer from `dp_controller_start`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dp_controller_start_pairing(controller: *mut DpController) {
    guard(
        || (),
        || {
            // SAFETY: live per the caller's contract.
            if let Some(c) = unsafe { controller.as_ref() } {
                c.controller.start_pairing();
            }
        },
    )
}

/// # Safety
/// `controller` must be NULL or a live pointer from `dp_controller_start`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dp_controller_cancel_pairing(controller: *mut DpController) {
    guard(
        || (),
        || {
            // SAFETY: live per the caller's contract.
            if let Some(c) = unsafe { controller.as_ref() } {
                c.controller.cancel_pairing();
            }
        },
    )
}

/// # Safety
/// `controller` as for the other controller calls; `config_json` as for `from_c`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dp_controller_apply_config(
    controller: *mut DpController,
    config_json: *const c_char,
) -> *mut c_char {
    guard(
        || into_c("Could not apply the settings.".into()),
        || {
            // SAFETY: live per the caller's contract.
            let Some(c) = (unsafe { controller.as_ref() }) else {
                return into_c("No controller.".into());
            };
            // SAFETY: forwarded from the caller's contract.
            match unsafe { from_c(config_json) }.map_err(|e| vec![e]).and_then(parse_config) {
                Ok(config) => {
                    c.controller.apply_settings(config.engine_settings());
                    ptr::null_mut()
                }
                Err(problems) => into_c(problems.join(" ")),
            }
        },
    )
}

/// # Safety
/// `controller` must be NULL or a pointer from `dp_controller_start`, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dp_controller_free(controller: *mut DpController) {
    guard(
        || (),
        || {
            if !controller.is_null() {
                // SAFETY: allocated by Box::into_raw in dp_controller_start. Dropping
                // joins the background thread, so no callback runs after this.
                drop(unsafe { Box::from_raw(controller) });
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::guard;
    use std::cell::Cell;

    #[test]
    fn guard_builds_the_fallback_only_after_a_panic() {
        let built = Cell::new(0);
        let fallback = || {
            built.set(built.get() + 1);
            -1
        };
        assert_eq!(guard(fallback, || 7), 7);
        assert_eq!(built.get(), 0);
        assert_eq!(guard(fallback, || panic!("boom")), -1);
        assert_eq!(built.get(), 1);
    }
}
