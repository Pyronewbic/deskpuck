//! One tray at a time: a lock on a file in the settings folder, which the
//! system releases however the program ends, crash included.

use std::fs::{File, TryLockError};
use std::path::Path;

pub enum Instance {
    /// This is the only tray; it holds the lock while the file stays open.
    Only(File),
    /// Another tray holds the lock.
    Running,
}

pub fn claim(path: &Path) -> Result<Instance, String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let file =
        deskpuck_core::files::open_lock(path).map_err(|e| format!("{}: {e}", path.display()))?;
    match file.try_lock() {
        Ok(()) => Ok(Instance::Only(file)),
        Err(TryLockError::WouldBlock) => Ok(Instance::Running),
        Err(TryLockError::Error(e)) => Err(format!("{}: {e}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_claim_finds_the_first_and_a_released_lock_is_free_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Deskpuck").join("deskpuck.lock");
        let first = claim(&path).unwrap();
        assert!(matches!(first, Instance::Only(_)), "creates the folder and takes the lock");
        assert!(matches!(claim(&path).unwrap(), Instance::Running));
        drop(first);
        assert!(matches!(claim(&path).unwrap(), Instance::Only(_)), "released on close");
    }

    #[test]
    fn a_folder_that_cannot_be_made_is_an_error_not_a_second_tray() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("file");
        std::fs::write(&blocker, "").unwrap();
        assert!(claim(&blocker.join("deskpuck.lock")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_at_the_lock_path_is_an_error_and_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("elsewhere");
        std::fs::write(&target, "keep").unwrap();
        let link = dir.path().join("deskpuck.lock");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(claim(&link).is_err());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep");
    }
}
