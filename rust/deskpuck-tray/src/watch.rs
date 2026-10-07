//! Notices when config.json changes on disk, so edits from the settings
//! window or an editor apply without Reload Settings.

use std::path::Path;
use std::time::SystemTime;

/// How often the tray looks at the file.
pub const POLL_SECONDS: u64 = 1;

#[derive(Debug, Default)]
pub struct Watch {
    seen: Option<(SystemTime, u64)>,
}

fn stamp(path: &Path) -> Option<(SystemTime, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

impl Watch {
    /// Starts from the file as it is now, which the caller has already loaded.
    pub fn new(path: &Path) -> Self {
        Self { seen: stamp(path) }
    }

    /// True once per change. A file that disappears is not a change: the
    /// settings in use stay until a new file is written.
    pub fn changed(&mut self, path: &Path) -> bool {
        match stamp(path) {
            Some(now) if self.seen != Some(now) => {
                self.seen = Some(now);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{File, FileTimes};
    use std::time::Duration;

    fn touch(path: &Path, contents: &str, at: SystemTime) {
        std::fs::write(path, contents).unwrap();
        File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(FileTimes::new().set_modified(at))
            .unwrap();
    }

    #[test]
    fn each_change_is_reported_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        touch(&path, "{}", t0);
        let mut watch = Watch::new(&path);
        assert!(!watch.changed(&path), "the loaded file is not a change");

        touch(&path, "{} ", t0 + Duration::from_secs(1));
        assert!(watch.changed(&path));
        assert!(!watch.changed(&path), "reported once");

        // Same size, new time: an atomic rewrite with equal length.
        touch(&path, "{}!", t0 + Duration::from_secs(2));
        assert!(watch.changed(&path));

        // Same time, new size: two writes inside one timestamp tick.
        touch(&path, "{}!!", t0 + Duration::from_secs(2));
        assert!(watch.changed(&path));
    }

    #[test]
    fn a_file_that_appears_is_a_change_and_one_that_goes_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut watch = Watch::new(&path);
        assert!(!watch.changed(&path), "still missing");
        std::fs::write(&path, "{}").unwrap();
        assert!(watch.changed(&path), "created by the settings window");
        std::fs::remove_file(&path).unwrap();
        assert!(!watch.changed(&path));
    }
}
