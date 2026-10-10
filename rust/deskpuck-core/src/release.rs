//! One-click update: what to fetch for a newer release, and whether it is the maintainer's. Downloads
//! are attacker bytes until the signature over SHA256SUMS and the package's line in it check out.

use crate::update::{Version, parse};
use minisign_verify::{PublicKey, Signature};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::File as FsFile;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The release signing key; its secret half never leaves the maintainer's Mac.
pub const PUBLIC_KEY: &str = include_str!("../../../packaging/minisign.pub");
pub const SUMS: &str = "SHA256SUMS";
pub const SIGNATURE: &str = "SHA256SUMS.minisig";

const DOWNLOAD_PREFIX: &str = "https://github.com/Pyronewbic/deskpuck/releases/download/v";
/// Where github.com redirects release downloads (observed 2026-10-10).
const ASSET_HOST: &str = "https://release-assets.githubusercontent.com/";
/// The longest redirect a download may follow; GitHub's are about 950 bytes.
pub const MAX_URL: usize = 4096;
const MAX_SUMS: u64 = 64 * 1024;
const MAX_SIGNATURE: u64 = 4 * 1024;
const MAX_PACKAGE: u64 = 200 * 1024 * 1024;

/// What each platform installs from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Package {
    MacZip,
    WindowsSetup,
    LinuxTarball,
}

impl Package {
    pub fn this_platform() -> Option<Self> {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Some(Self::MacZip)
        } else if cfg!(all(windows, target_arch = "x86_64")) {
            Some(Self::WindowsSetup)
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Some(Self::LinuxTarball)
        } else {
            None
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Self::MacZip => "macos-arm64.zip",
            Self::WindowsSetup => "setup.exe",
            Self::LinuxTarball => "linux-x86_64.tar.gz",
        }
    }
}

/// Size caps, a parameter only so tests can reach them cheaply.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub sums: u64,
    pub signature: u64,
    pub package: u64,
}

pub const LIMITS: Limits =
    Limits { sums: MAX_SUMS, signature: MAX_SIGNATURE, package: MAX_PACKAGE };

/// One download: saved in the download folder as `name`, refused past `max_bytes`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Download {
    pub name: String,
    pub url: String,
    pub max_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    version: String,
    package: String,
}

impl Release {
    /// Only for a plain version strictly newer than `current`, so a replayed older
    /// release is never installed.
    pub fn new(version: &str, current: &str, package: Package) -> Option<Self> {
        let wanted = parse(version)?;
        (wanted > parse(current)?).then(|| {
            let version = display(wanted);
            let package = format!("Deskpuck-{version}-{}", package.suffix());
            Self { version, package }
        })
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    /// The package's file name, as SHA256SUMS lists it.
    pub fn package(&self) -> &str {
        &self.package
    }

    /// In order: the checksums, their signature, the package. Built from constants
    /// and the parsed version, never from anything a server sent.
    pub fn downloads(&self) -> [Download; 3] {
        let download = |name: &str, max_bytes| Download {
            name: name.to_owned(),
            url: format!("{DOWNLOAD_PREFIX}{}/{name}", self.version),
            max_bytes,
        };
        [
            download(SUMS, LIMITS.sums),
            download(SIGNATURE, LIMITS.signature),
            download(&self.package, LIMITS.package),
        ]
    }
}

/// Whether a download may follow a redirect to `url`: only GitHub's release-asset
/// host, over HTTPS, in plain URL characters.
pub fn redirect_allowed(url: &str) -> bool {
    url.len() <= MAX_URL
        && url.strip_prefix(ASSET_HOST).is_some_and(|rest| {
            !rest.is_empty()
                && rest.bytes().all(|b| b.is_ascii_alphanumeric() || b"-._~%/?&=+".contains(&b))
        })
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    Unreadable(String),
    TooLarge(String),
    BadSignature,
    NotListed,
    ListedTwice,
    HashMismatch,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable(name) => write!(f, "{name} could not be read"),
            Self::TooLarge(name) => write!(f, "{name} is too large"),
            Self::BadSignature => write!(f, "{SUMS} is not signed with the release key"),
            Self::NotListed => write!(f, "{SUMS} does not list the package"),
            Self::ListedTwice => write!(f, "{SUMS} lists the package twice"),
            Self::HashMismatch => write!(f, "the package does not match {SUMS}"),
        }
    }
}

/// The package's path once the signature over SHA256SUMS verifies with the release
/// key and the package hashes to its one line there. `dir` holds the downloads.
pub fn verify(dir: &Path, release: &Release) -> Result<PathBuf, Refusal> {
    verify_with(PUBLIC_KEY, LIMITS, dir, release)
}

#[doc(hidden)]
pub fn verify_with(
    public_key: &str,
    limits: Limits,
    dir: &Path,
    release: &Release,
) -> Result<PathBuf, Refusal> {
    let sums = read_capped(dir, SUMS, limits.sums)?;
    let signature = read_capped(dir, SIGNATURE, limits.signature)?;
    let signed = std::str::from_utf8(&signature)
        .ok()
        .and_then(|text| Signature::decode(text).ok())
        .zip(PublicKey::decode(public_key).ok())
        .is_some_and(|(signature, key)| key.verify(&sums, &signature, false).is_ok());
    if !signed {
        return Err(Refusal::BadSignature);
    }
    let expected = listed_hash(&sums, release.package())?;
    let path = dir.join(release.package());
    if sha256_capped(&path, release.package(), limits.package)? != expected {
        return Err(Refusal::HashMismatch);
    }
    Ok(path)
}

/// The hash on the one line naming exactly `name`, in `shasum -a 256` format.
fn listed_hash(sums: &[u8], name: &str) -> Result<String, Refusal> {
    let text = std::str::from_utf8(sums).map_err(|_| Refusal::NotListed)?;
    let mut found = text.lines().filter_map(|line| {
        let (hash, file) = line.split_at_checked(64)?;
        let file = file.strip_prefix("  ").or_else(|| file.strip_prefix(" *"))?;
        let hex = hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        (hex && file == name).then_some(hash)
    });
    match (found.next(), found.next()) {
        (Some(hash), None) => Ok(hash.to_owned()),
        (Some(_), Some(_)) => Err(Refusal::ListedTwice),
        (None, _) => Err(Refusal::NotListed),
    }
}

fn read_capped(dir: &Path, name: &str, max: u64) -> Result<Vec<u8>, Refusal> {
    let file = FsFile::open(dir.join(name)).map_err(|_| Refusal::Unreadable(name.into()))?;
    let mut bytes = Vec::new();
    file.take(max + 1).read_to_end(&mut bytes).map_err(|_| Refusal::Unreadable(name.into()))?;
    if bytes.len() as u64 > max {
        return Err(Refusal::TooLarge(name.into()));
    }
    Ok(bytes)
}

fn sha256_capped(path: &Path, name: &str, max: u64) -> Result<String, Refusal> {
    let unreadable = || Refusal::Unreadable(name.into());
    let mut file = FsFile::open(path).map_err(|_| unreadable())?.take(max + 1);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 64 * 1024];
    let mut total = 0u64;
    loop {
        let read = file.read(&mut buffer).map_err(|_| unreadable())?;
        if read == 0 {
            break;
        }
        total += read as u64;
        hasher.update(&buffer[..read]);
    }
    if total > max {
        return Err(Refusal::TooLarge(name.into()));
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn display((major, minor, patch): Version) -> String {
    format!("{major}.{minor}.{patch}")
}
