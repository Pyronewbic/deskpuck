//! Fixtures (tests/fixtures/release, made by its make.sh): SHA256SUMS for 0.4.0 signed
//! with the key in test.pub, plus signatures and SHA256SUMS variants an attacker could serve.

use deskpuck_core::release::*;
use std::path::{Path, PathBuf};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/release");
const MAC_0_4_0: &str = "Deskpuck-0.4.0-macos-arm64.zip";

fn fixture(name: &str) -> PathBuf {
    Path::new(FIXTURES).join(name)
}

fn test_key() -> String {
    std::fs::read_to_string(fixture("test.pub")).unwrap()
}

fn mac_0_4_0() -> Release {
    Release::new("0.4.0", "0.3.0", Package::MacZip).unwrap()
}

/// A download folder as the app leaves it: the checksums, their signature, the package.
fn downloaded(sums: &str, signature: &str, package: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(fixture(sums), dir.path().join(SUMS)).unwrap();
    std::fs::copy(fixture(signature), dir.path().join(SIGNATURE)).unwrap();
    std::fs::copy(fixture(&format!("assets/{package}")), dir.path().join(MAC_0_4_0)).unwrap();
    dir
}

fn check(dir: &Path) -> Result<PathBuf, Refusal> {
    verify_with(&test_key(), LIMITS, dir, &mac_0_4_0())
}

#[test]
fn a_signed_listed_package_verifies() {
    let dir = downloaded("SHA256SUMS", "SHA256SUMS.minisig", MAC_0_4_0);
    assert_eq!(check(dir.path()), Ok(dir.path().join(MAC_0_4_0)));
    for package in [Package::WindowsSetup, Package::LinuxTarball] {
        let release = Release::new("0.4.0", "0.3.0", package).unwrap();
        std::fs::copy(
            fixture(&format!("assets/{}", release.package())),
            dir.path().join(release.package()),
        )
        .unwrap();
        assert!(verify_with(&test_key(), LIMITS, dir.path(), &release).is_ok(), "{package:?}");
    }
}

#[test]
fn a_tampered_package_is_refused() {
    let dir = downloaded("SHA256SUMS", "SHA256SUMS.minisig", "Deskpuck-0.4.0-setup.exe");
    assert_eq!(check(dir.path()), Err(Refusal::HashMismatch));
    let mut bytes = std::fs::read(fixture(&format!("assets/{MAC_0_4_0}"))).unwrap();
    bytes[0] ^= 1;
    std::fs::write(dir.path().join(MAC_0_4_0), bytes).unwrap();
    assert_eq!(check(dir.path()), Err(Refusal::HashMismatch));
}

#[test]
fn a_signature_by_another_key_is_refused() {
    for signature in ["other-key.minisig", "forged-id.minisig"] {
        let dir = downloaded("SHA256SUMS", signature, MAC_0_4_0);
        assert_eq!(check(dir.path()), Err(Refusal::BadSignature), "{signature}");
    }
}

#[test]
fn a_legacy_unhashed_signature_is_refused() {
    let dir = downloaded("SHA256SUMS", "legacy.minisig", MAC_0_4_0);
    assert_eq!(check(dir.path()), Err(Refusal::BadSignature));
}

#[test]
fn edited_checksums_or_signature_are_refused() {
    let dir = downloaded("SHA256SUMS", "SHA256SUMS.minisig", MAC_0_4_0);
    let sums = std::fs::read(fixture("SHA256SUMS")).unwrap();
    std::fs::write(dir.path().join(SUMS), &sums[..sums.len() - 10]).unwrap();
    assert_eq!(check(dir.path()), Err(Refusal::BadSignature), "truncated");

    let mut appended = sums.clone();
    appended.extend_from_slice(
        b"0000000000000000000000000000000000000000000000000000000000000000  x\n",
    );
    std::fs::write(dir.path().join(SUMS), appended).unwrap();
    assert_eq!(check(dir.path()), Err(Refusal::BadSignature), "a line appended");

    std::fs::write(dir.path().join(SUMS), &sums).unwrap();
    let signature = std::fs::read_to_string(fixture("SHA256SUMS.minisig")).unwrap();
    let mut lines: Vec<String> = signature.lines().map(str::to_owned).collect();
    lines[2] = "trusted comment: Deskpuck 99.0.0".into();
    std::fs::write(dir.path().join(SIGNATURE), lines.join("\n")).unwrap();
    assert_eq!(check(dir.path()), Err(Refusal::BadSignature), "trusted comment edited");

    for garbage in [&b""[..], b"not a signature", &[0xff, 0xfe, 0x00]] {
        std::fs::write(dir.path().join(SIGNATURE), garbage).unwrap();
        assert_eq!(check(dir.path()), Err(Refusal::BadSignature), "{garbage:?}");
    }
}

#[test]
fn a_replayed_older_release_cannot_vouch_for_a_newer_package() {
    let dir = downloaded("older/SHA256SUMS", "older/SHA256SUMS.minisig", MAC_0_4_0);
    assert_eq!(check(dir.path()), Err(Refusal::NotListed));
}

#[test]
fn checksums_without_the_package_or_with_it_twice_are_refused() {
    let dir = downloaded("missing/SHA256SUMS", "missing/SHA256SUMS.minisig", MAC_0_4_0);
    assert_eq!(check(dir.path()), Err(Refusal::NotListed));
    let dir = downloaded("duplicate/SHA256SUMS", "duplicate/SHA256SUMS.minisig", MAC_0_4_0);
    assert_eq!(check(dir.path()), Err(Refusal::ListedTwice));
}

#[test]
fn oversized_or_missing_files_are_refused_before_hashing() {
    let dir = downloaded("SHA256SUMS", "SHA256SUMS.minisig", MAC_0_4_0);
    let package_size = std::fs::metadata(dir.path().join(MAC_0_4_0)).unwrap().len();
    let tight = Limits { package: package_size - 1, ..LIMITS };
    assert_eq!(
        verify_with(&test_key(), tight, dir.path(), &mac_0_4_0()),
        Err(Refusal::TooLarge(MAC_0_4_0.into()))
    );
    let exact = Limits { package: package_size, ..LIMITS };
    assert!(verify_with(&test_key(), exact, dir.path(), &mac_0_4_0()).is_ok());

    std::fs::write(dir.path().join(SUMS), vec![b'0'; LIMITS.sums as usize + 1]).unwrap();
    assert_eq!(check(dir.path()), Err(Refusal::TooLarge(SUMS.into())));
    std::fs::copy(fixture("SHA256SUMS"), dir.path().join(SUMS)).unwrap();
    std::fs::write(dir.path().join(SIGNATURE), vec![b'0'; LIMITS.signature as usize + 1]).unwrap();
    assert_eq!(check(dir.path()), Err(Refusal::TooLarge(SIGNATURE.into())));

    std::fs::copy(fixture("SHA256SUMS.minisig"), dir.path().join(SIGNATURE)).unwrap();
    std::fs::remove_file(dir.path().join(MAC_0_4_0)).unwrap();
    assert_eq!(check(dir.path()), Err(Refusal::Unreadable(MAC_0_4_0.into())));
    std::fs::remove_file(dir.path().join(SUMS)).unwrap();
    assert_eq!(check(dir.path()), Err(Refusal::Unreadable(SUMS.into())));
}

#[test]
fn the_release_key_parses_and_is_not_the_test_key() {
    let key = minisign_verify::PublicKey::decode(PUBLIC_KEY);
    assert!(key.is_ok(), "packaging/minisign.pub is not a minisign public key");
    assert_ne!(
        PUBLIC_KEY.lines().nth(1),
        test_key().lines().nth(1),
        "the release key is the test key"
    );
    let dir = downloaded("SHA256SUMS", "SHA256SUMS.minisig", MAC_0_4_0);
    assert_eq!(verify(dir.path(), &mac_0_4_0()), Err(Refusal::BadSignature));
}

#[test]
fn only_a_strictly_newer_plain_version_is_a_release() {
    assert_eq!(mac_0_4_0().version(), "0.4.0");
    assert_eq!(mac_0_4_0().package(), MAC_0_4_0);
    for (version, current) in [
        ("0.3.0", "0.3.0"),
        ("0.2.9", "0.3.0"),
        ("0.9.0", "0.10.0"),
        ("0.4.0-rc1", "0.3.0"),
        ("0.4.0/../../evil", "0.3.0"),
        ("v0.4.0", "0.3.0"),
        ("0.4", "0.3.0"),
        ("", "0.3.0"),
        ("0.4.0", "garbage"),
        ("\u{0660}.4.0", "0.3.0"),
    ] {
        assert_eq!(
            Release::new(version, current, Package::MacZip),
            None,
            "{version} over {current}"
        );
    }
}

#[test]
fn downloads_are_fixed_github_addresses_with_their_caps() {
    let release = Release::new("0.10.0", "0.9.0", Package::LinuxTarball).unwrap();
    let base = "https://github.com/Pyronewbic/deskpuck/releases/download/v0.10.0/";
    let [sums, signature, package] = release.downloads();
    assert_eq!(
        sums,
        Download { name: SUMS.into(), url: format!("{base}SHA256SUMS"), max_bytes: LIMITS.sums }
    );
    assert_eq!(signature.url, format!("{base}SHA256SUMS.minisig"));
    assert_eq!(signature.max_bytes, LIMITS.signature);
    assert_eq!(package.name, "Deskpuck-0.10.0-linux-x86_64.tar.gz");
    assert_eq!(package.url, format!("{base}Deskpuck-0.10.0-linux-x86_64.tar.gz"));
    assert_eq!(package.max_bytes, LIMITS.package);
    let windows = Release::new("1.0.0", "0.3.0", Package::WindowsSetup).unwrap();
    assert_eq!(windows.package(), "Deskpuck-1.0.0-setup.exe");
}

#[test]
fn redirects_go_only_to_the_release_asset_host() {
    let real = "https://release-assets.githubusercontent.com/github-production-release-asset/1409424224/\
                2e6a1453-8acd?sp=r&sv=2018-11-09&se=2026-10-10T17%3A43%3A08Z&rscd=attachment%3B+filename%3D\
                SHA256SUMS&sig=sVVMC4G7%2FqiaMs%3D&jwt=eyJ0eXAi.eyJpc3Mi.wAfVDU8-vpb3T5_gGEw";
    assert!(redirect_allowed(real));
    for url in [
        "http://release-assets.githubusercontent.com/x",
        "https://release-assets.githubusercontent.com.evil.example/x",
        "https://release-assets.githubusercontent.com@evil.example/x",
        "https://release-assets.githubusercontent.com:8443/x",
        "https://objects.githubusercontent.com/x",
        "https://evil.example/https://release-assets.githubusercontent.com/x",
        "HTTPS://RELEASE-ASSETS.GITHUBUSERCONTENT.COM/x",
        "https://release-assets.githubusercontent.com/",
        "https://release-assets.githubusercontent.com/x y",
        "https://release-assets.githubusercontent.com/x\ny",
        "https://release-assets.githubusercontent.com/x\\y",
        "https://release-assets.githubusercontent.com/x#y",
        "https://release-assets.githubusercontent.com/[x]",
        "https://release-assets.githubusercontent.com/{x}",
        "https://release-assets.githubusercontent.com/\u{0430}",
        " https://release-assets.githubusercontent.com/x",
        "",
    ] {
        assert!(!redirect_allowed(url), "{url:?}");
    }
    let long = format!("https://release-assets.githubusercontent.com/{}", "a".repeat(5000));
    assert!(!redirect_allowed(&long));
}
