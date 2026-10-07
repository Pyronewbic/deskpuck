//! Start at Login: an autostart entry for this copy of the program, a
//! desktop file in the XDG autostart folder on Linux and a Run value in
//! the registry on Windows. Both are per user and need no admin rights.

use std::path::Path;

/// Quotes `program` as one argument of a desktop entry's Exec key, which
/// escapes `"`, `` ` ``, `$` and `\` with a backslash inside the quotes.
pub fn desktop_exec(program: &Path) -> String {
    let mut quoted = String::from("\"");
    for ch in program.to_string_lossy().chars() {
        if matches!(ch, '"' | '`' | '$' | '\\') {
            quoted.push('\\');
        }
        quoted.push(ch);
    }
    quoted.push('"');
    quoted
}

/// The autostart desktop file that starts `program`.
pub fn desktop_entry(program: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Deskpuck\n\
         Comment=Use a Switch 2 Joy-Con as a mouse and keyboard\n\
         Exec={}\nIcon=deskpuck\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        desktop_exec(program)
    )
}

/// The Run value that starts `program`: the path in quotes, as Windows
/// otherwise splits it at the first space.
pub fn run_value(program: &Path) -> String {
    format!("\"{}\"", program.display())
}

/// Turns the autostart desktop file at `file` on or off for `program`.
pub fn set_desktop(file: &Path, program: &Path, on: bool) -> Result<(), String> {
    let result = if on {
        deskpuck_core::files::write_private(file, desktop_entry(program).as_bytes())
    } else {
        match std::fs::remove_file(file) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    };
    result.map_err(|e| format!("Could not change {}: {e}", file.display()))
}

/// On only if the file starts this copy of the program; an entry left by a
/// copy since moved or deleted reads as off, and turning on rewrites it.
pub fn desktop_is_on(file: &Path, program: &Path) -> bool {
    deskpuck_core::files::read_capped(file, 64 * 1024)
        .ok()
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .is_some_and(|text| {
            text.lines().any(|line| line == format!("Exec={}", desktop_exec(program)))
        })
}

#[cfg(target_os = "linux")]
mod platform {
    use std::path::{Path, PathBuf};

    /// ~/.config/autostart/deskpuck.desktop, or under $XDG_CONFIG_HOME.
    fn file() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("autostart").join("deskpuck.desktop"))
    }

    pub fn is_on(program: &Path) -> bool {
        file().is_some_and(|file| super::desktop_is_on(&file, program))
    }

    pub fn set(program: &Path, on: bool) -> Result<(), String> {
        let file = file().ok_or("No settings folder on this system")?;
        super::set_desktop(&file, program, on)
    }
}

#[cfg(windows)]
mod platform {
    use std::path::Path;
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
    };

    pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &str = "Deskpuck";

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    /// The string value `name` under HKEY_CURRENT_USER\`key`, if there is one.
    pub fn read(key: &str, name: &str) -> Option<String> {
        let (key, name) = (wide(key), wide(name));
        let mut buffer = vec![0u16; 4096];
        let mut bytes = (buffer.len() * 2) as u32;
        // SAFETY: the strings are NUL-terminated and `bytes` is the buffer's size in bytes.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let len = (bytes as usize / 2).min(buffer.len());
        let text = &buffer[..len];
        let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
        String::from_utf16(&text[..end]).ok()
    }

    pub fn write(key: &str, name: &str, value: &str) -> Result<(), u32> {
        let (key, name, value) = (wide(key), wide(name), wide(value));
        // SAFETY: the strings are NUL-terminated; the size is the value's in bytes, NUL included.
        let status = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                REG_SZ,
                value.as_ptr().cast(),
                (value.len() * 2) as u32,
            )
        };
        if status == ERROR_SUCCESS { Ok(()) } else { Err(status) }
    }

    pub fn delete(key: &str, name: &str) -> Result<(), u32> {
        let (key, name) = (wide(key), wide(name));
        // SAFETY: both strings are NUL-terminated.
        let status = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) };
        match status {
            ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => Ok(()),
            other => Err(other),
        }
    }

    pub fn is_on(program: &Path) -> bool {
        read(RUN_KEY, VALUE).is_some_and(|value| value == super::run_value(program))
    }

    pub fn set(program: &Path, on: bool) -> Result<(), String> {
        let result = if on {
            write(RUN_KEY, VALUE, &super::run_value(program))
        } else {
            delete(RUN_KEY, VALUE)
        };
        result.map_err(|code| format!("Could not change Start at Login (error {code})"))
    }
}

#[cfg(any(target_os = "linux", windows))]
pub use platform::{is_on, set};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn exec_paths_are_quoted_and_escaped_for_desktop_entries() {
        assert_eq!(desktop_exec(Path::new("/usr/bin/deskpuck")), "\"/usr/bin/deskpuck\"");
        assert_eq!(
            desktop_exec(Path::new("/home/a b/$x/\"q\"/`c`/back\\slash/deskpuck")),
            r#""/home/a b/\$x/\"q\"/\`c\`/back\\slash/deskpuck""#
        );
        assert!(desktop_entry(Path::new("/opt/deskpuck")).contains("\nExec=\"/opt/deskpuck\"\n"));
    }

    #[test]
    fn run_values_quote_the_path() {
        let program = PathBuf::from(r"C:\Users\A B\Deskpuck\deskpuck.exe");
        assert_eq!(run_value(&program), r#""C:\Users\A B\Deskpuck\deskpuck.exe""#);
    }

    #[test]
    fn the_autostart_file_turns_on_and_off_for_this_copy_only() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("autostart").join("deskpuck.desktop");
        let (here, moved) = (Path::new("/opt/a/deskpuck"), Path::new("/opt/b/deskpuck"));
        assert!(!desktop_is_on(&file, here), "no file");
        set_desktop(&file, here, true).unwrap();
        assert!(desktop_is_on(&file, here));
        assert!(!desktop_is_on(&file, moved), "an entry for another copy reads as off");
        set_desktop(&file, moved, true).unwrap();
        assert!(desktop_is_on(&file, moved), "turning on rewrites it for this copy");
        set_desktop(&file, moved, false).unwrap();
        assert!(!file.exists());
        set_desktop(&file, moved, false).expect("turning off twice is fine");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_in_the_autostart_folder_is_replaced_not_written_through() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("elsewhere");
        std::fs::write(&target, "keep").unwrap();
        let file = dir.path().join("deskpuck.desktop");
        std::os::unix::fs::symlink(&target, &file).unwrap();
        set_desktop(&file, Path::new("/opt/deskpuck"), true).unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep");
        assert!(!std::fs::symlink_metadata(&file).unwrap().file_type().is_symlink());
    }

    #[cfg(windows)]
    #[test]
    fn registry_values_are_written_read_and_deleted() {
        use windows_sys::Win32::System::Registry::HKEY_CURRENT_USER;
        // A scratch key, never the real Run key.
        let key = r"Software\Deskpuck-test-login";
        let value = run_value(Path::new(r"C:\A B\deskpuck.exe"));
        platform::write(key, "Deskpuck", &value).unwrap();
        assert_eq!(platform::read(key, "Deskpuck").as_deref(), Some(value.as_str()));
        platform::delete(key, "Deskpuck").unwrap();
        assert_eq!(platform::read(key, "Deskpuck"), None);
        platform::delete(key, "Deskpuck").expect("deleting twice is fine");
        let key: Vec<u16> = key.encode_utf16().chain(Some(0)).collect();
        // SAFETY: a NUL-terminated name of the scratch key this test made.
        unsafe {
            windows_sys::Win32::System::Registry::RegDeleteKeyW(HKEY_CURRENT_USER, key.as_ptr())
        };
    }
}
