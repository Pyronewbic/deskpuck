use deskpuck_core::update::{allowed_by_config, evaluate};

const TAG: &str = "https://github.com/Pyronewbic/deskpuck/releases/tag/v";

fn at(tag: &str) -> String {
    format!("{TAG}{tag}")
}

#[test]
fn a_newer_release_is_reported() {
    assert_eq!(evaluate(&at("0.4.0"), "0.3.0"), Some("0.4.0".into()));
    assert_eq!(evaluate(&at("0.3.1"), "0.3.0"), Some("0.3.1".into()));
    assert_eq!(evaluate(&at("1.0.0"), "0.99.99"), Some("1.0.0".into()));
}

#[test]
fn versions_compare_as_numbers_not_strings() {
    assert_eq!(evaluate(&at("0.10.0"), "0.9.0"), Some("0.10.0".into()));
    assert_eq!(evaluate(&at("0.9.0"), "0.10.0"), None);
}

#[test]
fn the_same_or_an_older_release_is_not_an_update() {
    assert_eq!(evaluate(&at("0.3.0"), "0.3.0"), None);
    assert_eq!(evaluate(&at("0.2.9"), "0.3.0"), None);
    assert_eq!(evaluate(&at("0.0.1"), "9.9.9"), None);
}

#[test]
fn another_host_scheme_or_repository_is_refused() {
    for location in [
        "https://github.com.evil.example/Pyronewbic/deskpuck/releases/tag/v9.9.9",
        "http://github.com/Pyronewbic/deskpuck/releases/tag/v9.9.9",
        "https://github.com/Attacker/deskpuck/releases/tag/v9.9.9",
        "https://github.com/Pyronewbic/deskpuck-fork/releases/tag/v9.9.9",
        "https://evil.example/https://github.com/Pyronewbic/deskpuck/releases/tag/v9.9.9",
        " https://github.com/Pyronewbic/deskpuck/releases/tag/v9.9.9",
        "HTTPS://GITHUB.COM/Pyronewbic/deskpuck/releases/tag/v9.9.9",
    ] {
        assert_eq!(evaluate(location, "0.3.0"), None, "{location}");
    }
}

#[test]
fn anything_after_the_version_is_refused() {
    for tag in [
        "9.9.9/../evil",
        "9.9.9?x=1",
        "9.9.9#x",
        "9.9.9 ",
        "9.9.9\n",
        "9.9.9-rc.1",
        "9.9.9+build",
        "9.9.9.9",
        "9.9.9/",
    ] {
        assert_eq!(evaluate(&at(tag), "0.3.0"), None, "{tag:?}");
    }
}

#[test]
fn malformed_versions_are_refused() {
    for tag in [
        "",
        "9",
        "9.9",
        "9..9",
        ".9.9",
        "9.9.",
        "x.9.9",
        "-1.0.0",
        "+1.0.0",
        "9.9.+9",
        "01.0.0",
        "0.01.0",
        "1234567.0.0",
        "99999999999999999999.0.0",
        "\u{0663}.0.0",
        "\u{FF19}.0.0",
        "9.9.\u{0039}\u{0301}",
    ] {
        assert_eq!(evaluate(&at(tag), "0.3.0"), None, "{tag:?}");
    }
    assert_eq!(
        evaluate("https://github.com/Pyronewbic/deskpuck/releases/tag/9.9.9", "0.3.0"),
        None
    );
}

#[test]
fn an_unparsable_running_version_never_reports_an_update() {
    for current in ["", "0.3", "v0.3.0", "0.3.0-dev", "dev"] {
        assert_eq!(evaluate(&at("9.9.9"), current), None, "{current:?}");
    }
}

#[test]
fn oversized_input_is_refused() {
    let long = format!("{}{}", at("9.9.9"), "0".repeat(1024 * 1024));
    assert_eq!(evaluate(&long, "0.3.0"), None);
    let padded = format!("{TAG}{}9.9.9", "0".repeat(300));
    assert_eq!(evaluate(&padded, "0.3.0"), None);
    assert_eq!(evaluate("", "0.3.0"), None);
}

#[test]
fn the_largest_accepted_component_still_compares() {
    assert_eq!(evaluate(&at("999999.0.0"), "999998.999999.999999"), Some("999999.0.0".into()));
}

#[test]
fn only_a_readable_config_or_none_allows_the_check() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("config.json");
    assert!(allowed_by_config(&path), "no file means the default, on");
    for (contents, allowed) in [
        (r#"{"version": 1, "checkUpdates": true}"#.to_owned(), true),
        (r#"{"version": 1}"#.to_owned(), true),
        (r#"{"version": 1, "checkUpdates": false}"#.to_owned(), false),
        (r#"{"version": 1, "checkUpdates": fal"#.to_owned(), false),
        ("not json".to_owned(), false),
        (format!(r#"{{"version": 1, "pad": "{}"}}"#, "x".repeat(70 * 1024)), false),
    ] {
        std::fs::write(&path, &contents).expect("write");
        assert_eq!(allowed_by_config(&path), allowed, "{}", &contents[..contents.len().min(60)]);
    }
}
