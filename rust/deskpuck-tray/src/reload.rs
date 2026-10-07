use deskpuck_core::config::Config;
use deskpuck_core::engine::EngineSettings;

#[derive(Debug, PartialEq)]
pub struct Reload {
    /// Settings to apply; none when they did not change, since applying
    /// releases held keys and latched modifiers.
    pub apply: Option<EngineSettings>,
    pub note: Option<String>,
}

pub fn summarize(warnings: &[String]) -> Option<String> {
    let first = warnings.first()?;
    Some(match warnings.len() {
        1 => format!("Settings: {first}"),
        n => format!("Settings: {first} (and {} more)", n - 1),
    })
}

/// A file that cannot be used at all (half-written, a typo) keeps the
/// settings in use rather than replacing them with the defaults.
pub fn reload(
    current: &mut EngineSettings,
    loaded: Result<(Config, Vec<String>), String>,
) -> Reload {
    match loaded {
        Err(problem) => Reload {
            apply: None,
            note: Some(format!("Settings: {problem}; keeping the current settings.")),
        },
        Ok((config, warnings)) => {
            let settings = config.engine_settings();
            let apply = (settings != *current).then(|| {
                *current = settings.clone();
                settings
            });
            Reload { apply, note: summarize(&warnings) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_speed(speed: f64) -> Config {
        Config { pointer_speed: speed, ..Config::default() }
    }

    #[test]
    fn unchanged_settings_are_not_applied_again() {
        let mut current = Config::default().engine_settings();
        let same = reload(&mut current, Ok((Config::default(), Vec::new())));
        assert_eq!(same, Reload { apply: None, note: None });

        let changed = reload(&mut current, Ok((with_speed(2.0), Vec::new())));
        assert_eq!(changed.apply, Some(with_speed(2.0).engine_settings()), "control");
        assert_eq!(current, with_speed(2.0).engine_settings());
        assert_eq!(reload(&mut current, Ok((with_speed(2.0), Vec::new()))).apply, None);
    }

    #[test]
    fn a_file_that_cannot_be_used_keeps_the_settings_in_use() {
        let mut current = with_speed(2.0).engine_settings();
        let out = reload(&mut current, Err("Config is not valid JSON (EOF)".into()));
        assert_eq!(out.apply, None);
        assert_eq!(
            out.note.as_deref(),
            Some("Settings: Config is not valid JSON (EOF); keeping the current settings.")
        );
        assert_eq!(current, with_speed(2.0).engine_settings());
    }

    #[test]
    fn warnings_become_a_note_and_a_clean_file_clears_it() {
        let mut current = Config::default().engine_settings();
        let warnings = vec!["Unknown setting \"x\" ignored.".to_owned(), "two".to_owned()];
        let out = reload(&mut current, Ok((with_speed(3.0), warnings)));
        assert_eq!(
            out.note.as_deref(),
            Some("Settings: Unknown setting \"x\" ignored. (and 1 more)")
        );
        assert!(out.apply.is_some(), "the usable settings still apply");
        assert_eq!(reload(&mut current, Ok((with_speed(3.0), Vec::new()))).note, None);
    }
}
