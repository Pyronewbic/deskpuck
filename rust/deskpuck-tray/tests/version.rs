//! The tray shows the crate version, so it must be the app's release version.

#[test]
fn workspace_version_matches_the_app_version() {
    let script = include_str!("../../../scripts/make-app.sh");
    let line = script.lines().find(|line| line.starts_with("VERSION=")).expect("VERSION= line");
    assert_eq!(
        line,
        format!("VERSION=\"{}\"", env!("CARGO_PKG_VERSION")),
        "bump [workspace.package] version in rust/Cargo.toml with VERSION in scripts/make-app.sh"
    );
}
