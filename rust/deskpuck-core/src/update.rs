//! Decides whether github.com's latest-release redirect names a newer version.
//! The Location header is attacker-influenced, so only an exact, bounded shape passes.

/// Fetched with redirects off (GitHub answers with a 302 to the latest tag's page), and
/// the page the app opens: a constant, so nothing from the response reaches the browser.
pub const LATEST_URL: &str = "https://github.com/Pyronewbic/deskpuck/releases/latest";
/// Set to any non-empty value to turn the check off.
pub const DISABLE_ENV: &str = "DESKPUCK_NO_UPDATE_CHECK";

const TAG_PREFIX: &str = "https://github.com/Pyronewbic/deskpuck/releases/tag/v";
const MAX_COMPONENT_DIGITS: usize = 6;

type Version = (u32, u32, u32);

/// `Some(version)` only when `location` is the latest-release tag page of this
/// repository for a version strictly newer than `current`; anything else is unknown.
pub fn evaluate(location: &str, current: &str) -> Option<String> {
    let tag = location.strip_prefix(TAG_PREFIX)?;
    let latest = parse(tag)?;
    let running = parse(current)?;
    (latest > running).then(|| format!("{}.{}.{}", latest.0, latest.1, latest.2))
}

/// `true` when the environment turns the check off.
pub fn disabled_by_env() -> bool {
    std::env::var_os(DISABLE_ENV).is_some_and(|value| !value.is_empty())
}

fn parse(version: &str) -> Option<Version> {
    let mut parts = version.split('.');
    let version = (component(parts.next()?)?, component(parts.next()?)?, component(parts.next()?)?);
    parts.next().is_none().then_some(version)
}

fn component(digits: &str) -> Option<u32> {
    let valid = (1..=MAX_COMPONENT_DIGITS).contains(&digits.len())
        && digits.bytes().all(|b| b.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'));
    valid.then(|| digits.parse().ok()).flatten()
}
