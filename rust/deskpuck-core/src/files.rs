use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

/// `Err` is a phrase to follow the path.
pub fn read_capped(path: &Path, cap: u64) -> Result<Option<Vec<u8>>, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    // O_NONBLOCK: opening a FIFO must not hang; it is rejected below.
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::custom_flags(&mut options, libc::O_NONBLOCK);

    let file = match options.open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("could not be opened ({e})")),
    };
    if !file.metadata().is_ok_and(|m| m.is_file()) {
        return Err("is not a regular file".to_owned());
    }
    // Read at most one byte past the cap, even if the file grows meanwhile.
    let mut bytes = Vec::new();
    if file.take(cap + 1).read_to_end(&mut bytes).is_err() {
        return Err("could not be read".to_owned());
    }
    if bytes.len() as u64 > cap {
        return Err(format!("is larger than {} KB", cap / 1024));
    }
    Ok(Some(bytes))
}

pub fn open_lock(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(false).write(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::custom_flags(
        &mut options,
        libc::O_NOFOLLOW | libc::O_NONBLOCK,
    );
    // FILE_FLAG_OPEN_REPARSE_POINT: open a symlink itself, so the check below sees it.
    #[cfg(windows)]
    std::os::windows::fs::OpenOptionsExt::custom_flags(&mut options, 0x0020_0000);

    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("not a regular file"));
    }
    Ok(file)
}

pub fn create_private_dir(dir: &Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}

/// Renames a temp file over `path`, so a symlink there is replaced, never written through.
pub fn write_private(path: &Path, data: &[u8]) -> io::Result<()> {
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    create_private_dir(dir)?;

    let prefix = format!(".{}.", path.file_name().and_then(|n| n.to_str()).unwrap_or("state"));
    let mut temp = tempfile::Builder::new().prefix(&prefix).tempfile_in(dir)?;
    temp.write_all(data)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temp.as_file().set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn create_private_dir_makes_new_folders_owner_only_and_leaves_existing_ones() {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a").join("b");
        create_private_dir(&nested).unwrap();
        assert_eq!((mode(&nested), mode(&dir.path().join("a"))), (0o700, 0o700));

        let existing = dir.path().join("shared");
        std::fs::create_dir(&existing).unwrap();
        std::fs::set_permissions(&existing, std::fs::Permissions::from_mode(0o755)).unwrap();
        create_private_dir(&existing).unwrap();
        assert_eq!(mode(&existing), 0o755);
    }

    #[test]
    fn open_lock_creates_a_missing_file_and_reopens_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deskpuck.lock");
        open_lock(&path).unwrap();
        assert!(path.is_file());
        open_lock(&path).unwrap();
    }

    #[test]
    fn open_lock_refuses_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        assert!(open_lock(dir.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn open_lock_refuses_a_fifo_without_blocking() {
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let fifo_path = dir.path().join("deskpuck.lock");
        let fifo = fifo_path.clone();
        let c_path = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
        // SAFETY: c_path is a valid NUL-terminated path for the duration of the call.
        assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);

        // A blocking open (for write, with no reader) would hang here.
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || tx.send(open_lock(&fifo).map(drop)).unwrap());
        let result = rx.recv_timeout(std::time::Duration::from_secs(5)).expect("open_lock blocked");
        assert!(result.is_err());

        // With a reader the open itself succeeds, so the regular-file check refuses it.
        let _reader =
            OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(&fifo_path).unwrap();
        assert_eq!(open_lock(&fifo_path).unwrap_err().to_string(), "not a regular file");
    }

    #[test]
    fn open_lock_refuses_a_symlink_and_leaves_its_target_alone() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("elsewhere");
        std::fs::write(&target, "keep").unwrap();
        let link = dir.path().join("deskpuck.lock");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        #[cfg(windows)]
        if let Err(e) = std::os::windows::fs::symlink_file(&target, &link) {
            // 1314: no symlink privilege (no admin or Developer Mode); CI runners have it.
            assert!(e.raw_os_error() == Some(1314) && std::env::var_os("CI").is_none(), "{e}");
            eprintln!("skipped: creating a symlink needs admin or Developer Mode");
            return;
        }

        assert!(open_lock(&link).is_err());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep");
    }
}
