//! Reading and writing the small per-user state files (config.json,
//! pairing.json) with the same guards, so neither file can be the weaker one.

use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::path::Path;

/// `Ok(None)` if the file does not exist. Reads at most `cap` bytes and
/// refuses anything but a regular file. `Err` is a phrase to follow the path.
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

/// Writes a temp file beside `path` (owner-only) and renames it over the
/// target, so a symlink at `path` is replaced, never written through.
pub fn write_private(path: &Path, data: &[u8]) -> io::Result<()> {
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)?;

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
