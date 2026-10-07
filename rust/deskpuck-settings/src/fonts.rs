//! Draws the window in the desktop's own UI font where one can be found:
//! Segoe UI on Windows, fontconfig's sans-serif on Linux. egui's bundled
//! fonts stay behind it, for missing glyphs and for when there is none.

use eframe::egui::{Context, FontData, FontDefinitions, FontFamily};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Larger than any UI font; a bigger file is not one.
pub const MAX_FONT_BYTES: u64 = 32 * 1024 * 1024;

const SYSTEM_FONT: &str = "system-ui";

/// A font file and its face index within it (non-zero only in collections).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontFile {
    pub path: PathBuf,
    pub index: u32,
}

/// `fc-match -f '%{file}\n%{index}'` output: an absolute path to a TrueType
/// or OpenType file, then its face index.
pub fn parse_fc_match(output: &str) -> Option<FontFile> {
    let (file, index) = output.split_once('\n')?;
    let path = PathBuf::from(file);
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    if !path.is_absolute() || !["ttf", "otf", "ttc"].contains(&extension.as_str()) {
        return None;
    }
    Some(FontFile { path, index: index.trim().parse().ok()? })
}

/// Reads a font file that is a regular file of a plausible size.
pub fn read_font(path: &Path) -> Option<Vec<u8>> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_FONT_BYTES {
        return None;
    }
    std::fs::read(path).ok()
}

/// Puts `font` first in the proportional family, ahead of egui's fonts.
pub fn with_font(font: FontData) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(SYSTEM_FONT.to_owned(), Arc::new(font));
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, SYSTEM_FONT.to_owned());
    fonts
}

/// True if egui can use the face; egui panics on one it cannot parse.
pub fn usable(bytes: &[u8], index: u32) -> bool {
    skrifa::FontRef::from_index(bytes, index).is_ok()
}

/// Uses the system UI font if there is one; otherwise egui's stay.
pub fn install(ctx: &Context) {
    let Some(file) = system_font() else { return };
    let Some(bytes) = read_font(&file.path).filter(|b| usable(b, file.index)) else { return };
    let mut font = FontData::from_owned(bytes);
    font.index = file.index;
    ctx.set_fonts(with_font(font));
}

#[cfg(windows)]
fn system_font() -> Option<FontFile> {
    let fonts = PathBuf::from(std::env::var_os("WINDIR")?).join("Fonts");
    // Segoe UI Variable is Windows 11's UI font; Segoe UI is Windows 10's.
    ["SegUIVar.ttf", "segoeui.ttf"]
        .into_iter()
        .map(|name| fonts.join(name))
        .find(|path| path.is_file())
        .map(|path| FontFile { path, index: 0 })
}

#[cfg(target_os = "linux")]
fn system_font() -> Option<FontFile> {
    use std::process::{Command, Stdio};
    use std::sync::mpsc::channel;
    use std::time::Duration;
    let (tx, rx) = channel();
    // fontconfig answers in milliseconds; never hold the window up for it.
    std::thread::spawn(move || {
        let output = Command::new("fc-match")
            .args(["-f", "%{file}\n%{index}", "sans-serif"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output();
        let _ = tx.send(output);
    });
    let output = rx.recv_timeout(Duration::from_secs(1)).ok()?.ok()?;
    if !output.status.success() || output.stdout.len() > 4096 {
        return None;
    }
    parse_fc_match(std::str::from_utf8(&output.stdout).ok()?)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn system_font() -> Option<FontFile> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // fontconfig paths are Unix paths; on Windows "/usr/..." is not absolute.
    #[cfg(unix)]
    #[test]
    fn fc_match_output_is_read_and_anything_odd_is_refused() {
        let found = parse_fc_match("/usr/share/fonts/noto/NotoSans-Regular.ttf\n0");
        assert_eq!(
            found,
            Some(FontFile { path: "/usr/share/fonts/noto/NotoSans-Regular.ttf".into(), index: 0 })
        );
        let collection = parse_fc_match("/usr/share/fonts/Cantarell.TTC\n2").unwrap();
        assert_eq!(collection.index, 2);
        for odd in [
            "",
            "/usr/share/fonts/NotoSans-Regular.ttf",
            "relative/NotoSans-Regular.ttf\n0",
            "/usr/share/fonts/NotoSans.pcf.gz\n0",
            "/usr/share/fonts/NotoSans-Regular.ttf\n-1",
            "/usr/share/fonts/NotoSans-Regular.ttf\nnope",
        ] {
            assert_eq!(parse_fc_match(odd), None, "{odd:?}");
        }
    }

    #[test]
    fn only_regular_files_of_a_plausible_size_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let font = dir.path().join("font.ttf");
        std::fs::write(&font, b"font bytes").unwrap();
        assert_eq!(read_font(&font).as_deref(), Some(&b"font bytes"[..]));
        assert_eq!(read_font(dir.path()), None, "a directory");
        assert_eq!(read_font(&dir.path().join("missing.ttf")), None);
        let huge = dir.path().join("huge.ttf");
        std::fs::File::create(&huge).unwrap().set_len(MAX_FONT_BYTES + 1).unwrap();
        assert_eq!(read_font(&huge), None, "too big to be a UI font");
    }

    #[test]
    fn only_a_font_egui_can_parse_is_used() {
        let bundled = FontDefinitions::default();
        let real = &bundled.font_data["Ubuntu-Light"].font;
        assert!(usable(real, 0));
        assert!(!usable(real, 1), "no second face in a single-font file");
        // The same check egui makes before it would panic: the 12-byte font
        // header must parse; tables are read lazily, and safely, after that.
        assert!(!usable(&real[..11], 0), "cut inside the header");
        assert!(usable(&real[..12], 0));
        assert!(!usable(b"not a font", 0));
        assert!(!usable(&[], 0));
    }

    #[test]
    fn the_system_font_comes_first_and_egui_fonts_stay_as_fallback() {
        let fonts = with_font(FontData::from_static(b"not really a font"));
        let family = &fonts.families[&FontFamily::Proportional];
        assert_eq!(family[0], SYSTEM_FONT);
        assert!(family.len() > 1, "egui's own fonts are kept behind it: {family:?}");
        assert!(fonts.font_data.contains_key(SYSTEM_FONT));
        assert_eq!(
            fonts.families[&FontFamily::Monospace],
            FontDefinitions::default().families[&FontFamily::Monospace]
        );
    }
}
