//! What the tray shows, kept apart from the tray library so it is tested on
//! every OS. Mirrors the Mac app's menu (Sources/Deskpuck/AppDelegate.swift).

use deskpuck_ble::controller::LinkStatus;
use deskpuck_ble::receiver::PAIRING_WINDOW;
use deskpuck_core::mapping::Modifiers;

pub const PAIR: &str = "Pair New Joy-Con...";
pub const CANCEL_PAIRING: &str = "Cancel Pairing";
pub const WAITING: &str = "Waiting for a system tray";

/// Times are seconds since the tray started, like the receiver's clock.
#[derive(Debug)]
pub struct Model {
    status: LinkStatus,
    name: Option<String>,
    paused: bool,
    pairing: bool,
    pairing_ends: Option<f64>,
    latched: Modifiers,
    note: Option<String>,
    /// The note came from loading config.json, so a clean load clears it.
    note_from_settings: bool,
    failure: Option<String>,
    host: bool,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            status: LinkStatus::Searching,
            name: None,
            paused: false,
            pairing: false,
            pairing_ends: None,
            latched: Modifiers::NONE,
            note: None,
            note_from_settings: false,
            failure: None,
            host: false,
        }
    }
}

impl Model {
    pub fn status(&mut self, status: LinkStatus, name: Option<String>) {
        self.status = status;
        self.name = name;
        if status != LinkStatus::Pairing {
            self.pairing_ends = None;
        }
        // A pairing Joy-Con passes through Connecting before it is paired.
        if status != LinkStatus::Pairing && status != LinkStatus::Connecting {
            self.pairing = false;
        }
        if status == LinkStatus::Connected {
            self.note = None;
            self.note_from_settings = false;
        }
    }

    /// False while paused: pausing stops all connecting, so a window opened
    /// now could never pair. The caller starts pairing only when true.
    pub fn start_pairing(&mut self, now: f64) -> bool {
        if self.paused {
            return false;
        }
        self.pairing = true;
        self.pairing_ends = Some(now + PAIRING_WINDOW);
        true
    }

    pub fn cancel_pairing(&mut self) {
        self.pairing = false;
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn pairing(&self) -> bool {
        self.pairing
    }

    pub fn latched(&mut self, modifiers: Modifiers) {
        self.latched = modifiers;
    }

    /// A problem worth showing until the next connection: a refused input
    /// event, a settings warning.
    pub fn note(&mut self, message: impl Into<String>) {
        self.note = Some(message.into());
        self.note_from_settings = false;
    }

    /// The settings file's state after a load: a problem replaces the note,
    /// and a clean load clears a note that came from the file, nothing else.
    pub fn settings_note(&mut self, note: Option<String>) {
        match note {
            Some(note) => {
                self.note = Some(note);
                self.note_from_settings = true;
            }
            None if self.note_from_settings => {
                self.note = None;
                self.note_from_settings = false;
            }
            None => {}
        }
    }

    /// Whether something is showing the icon. Without it nobody could see the
    /// status or reach Pause and Quit, so nothing connects until it appears.
    pub fn set_host(&mut self, present: bool) {
        self.host = present;
    }

    pub fn host(&self) -> bool {
        self.host
    }

    /// Nothing can run (no input injection, no controller): the problem
    /// replaces the status line and only Quit is left.
    pub fn fail(&mut self, problem: impl Into<String>) {
        self.failure = Some(problem.into());
    }

    pub fn failed(&self) -> bool {
        self.failure.is_some()
    }

    pub fn connected(&self) -> bool {
        self.status == LinkStatus::Connected
    }

    pub fn status_text(&self, now: f64) -> String {
        if let Some(problem) = &self.failure {
            return problem.clone();
        }
        if !self.host {
            return WAITING.into();
        }
        let name = self.name.as_deref().unwrap_or("Joy-Con");
        let paused = self.paused;
        match self.status {
            LinkStatus::BluetoothOff => "Bluetooth is off".into(),
            LinkStatus::BluetoothUnauthorized => "Bluetooth access needed".into(),
            LinkStatus::Unavailable => "Bluetooth is unavailable".into(),
            LinkStatus::NotPaired => "Not paired: choose Pair New Joy-Con".into(),
            LinkStatus::Pairing if paused => "Pairing paused".into(),
            LinkStatus::Pairing => match self.pairing_ends {
                Some(ends) => {
                    let left = (ends - now).round().max(0.0);
                    format!("Pairing: hold SYNC on the Joy-Con ({left:.0} s left)")
                }
                None => "Pairing: hold SYNC on the Joy-Con".into(),
            },
            LinkStatus::Searching | LinkStatus::InUseElsewhere if paused => {
                "Paused: not looking for a Joy-Con".into()
            }
            LinkStatus::Searching => "Searching: hold SYNC on the paired Joy-Con".into(),
            LinkStatus::InUseElsewhere => "Joy-Con is in use by another app".into(),
            LinkStatus::Connecting if paused => format!("Connecting to {name}... (paused)"),
            LinkStatus::Connecting => format!("Connecting to {name}..."),
            LinkStatus::Connected if paused => format!("Connected to {name} (paused)"),
            LinkStatus::Connected => format!("Connected to {name}"),
        }
    }

    pub fn pair_label(&self) -> &'static str {
        if self.pairing { CANCEL_PAIRING } else { PAIR }
    }

    /// Cancel is always allowed; starting is not while paused.
    pub fn pair_enabled(&self) -> bool {
        !self.failed() && self.host && (self.pairing || !self.paused)
    }

    pub fn latched_text(&self) -> Option<String> {
        if self.latched.is_empty() {
            return None;
        }
        let labels: Vec<_> = self.latched.names().into_iter().map(modifier_label).collect();
        Some(format!("Latched: {}", labels.join("+")))
    }

    pub fn note_text(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// Hover text: the status, then whatever else the menu would show.
    pub fn tooltip(&self, now: f64) -> String {
        let mut lines = vec![format!("Deskpuck: {}", self.status_text(now))];
        lines.extend(self.latched_text());
        lines.extend(self.note.clone());
        lines.join("\n")
    }
}

/// Hosts may render the tooltip text as markup (the StatusNotifierItem spec
/// allows it), and it carries device names and OS messages.
#[cfg_attr(windows, allow(dead_code))]
pub fn escape_markup(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// The key each config name presses on this OS (see deskpuck-inject's
/// keymap): config.json keeps the Mac names.
pub fn modifier_label(name: &str) -> &'static str {
    match name {
        "control" => "Ctrl",
        "option" => "Alt",
        "shift" => "Shift",
        "command" if cfg!(windows) => "Win",
        "command" => "Super",
        _ => "?",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown() -> Model {
        let mut model = Model::default();
        model.set_host(true);
        model
    }

    #[test]
    fn a_clean_settings_load_clears_only_its_own_note() {
        let mut model = shown();
        model.settings_note(Some("Settings: not valid JSON".into()));
        assert_eq!(model.note_text(), Some("Settings: not valid JSON"));
        model.settings_note(None);
        assert_eq!(model.note_text(), None, "fixed file, note gone");

        model.note("Cannot post input: denied");
        model.settings_note(None);
        assert_eq!(model.note_text(), Some("Cannot post input: denied"), "not a settings note");

        model.settings_note(Some("Settings: bad".into()));
        model.note("Cannot post input: denied");
        model.settings_note(None);
        assert_eq!(
            model.note_text(),
            Some("Cannot post input: denied"),
            "replaced by another note"
        );
    }

    #[test]
    fn nothing_is_offered_until_a_tray_shows_the_icon() {
        let mut model = Model::default();
        assert_eq!(model.status_text(0.0), WAITING);
        assert!(!model.pair_enabled());
        model.set_host(true);
        assert_eq!(model.status_text(0.0), "Searching: hold SYNC on the paired Joy-Con");
        assert!(model.pair_enabled());
        model.set_host(false);
        assert_eq!(model.status_text(0.0), WAITING);
    }

    #[test]
    fn countdown_starts_at_the_pairing_window_and_never_goes_negative() {
        let mut model = shown();
        assert!(model.start_pairing(10.0));
        model.status(LinkStatus::Pairing, None);
        assert_eq!(model.status_text(10.0), "Pairing: hold SYNC on the Joy-Con (60 s left)");
        assert_eq!(model.status_text(69.6), "Pairing: hold SYNC on the Joy-Con (0 s left)");
        assert_eq!(model.status_text(500.0), "Pairing: hold SYNC on the Joy-Con (0 s left)");
    }

    #[test]
    fn pairing_survives_connecting_and_ends_on_anything_else() {
        let mut model = shown();
        model.start_pairing(0.0);
        model.status(LinkStatus::Pairing, None);
        model.status(LinkStatus::Connecting, Some("Joy-Con 2 (R)".into()));
        assert!(model.pairing(), "a pairing Joy-Con connects before it is paired");
        assert_eq!(model.pair_label(), CANCEL_PAIRING);

        model.status(LinkStatus::Connected, Some("Joy-Con 2 (R)".into()));
        assert!(!model.pairing());
        assert_eq!(model.pair_label(), PAIR);
        assert_eq!(model.status_text(0.0), "Connected to Joy-Con 2 (R)");

        model.start_pairing(0.0);
        model.status(LinkStatus::NotPaired, None);
        assert!(!model.pairing(), "a window that closes with nothing paired ends pairing");
    }

    #[test]
    fn pausing_blocks_new_pairing_but_not_cancelling() {
        let mut model = shown();
        model.set_paused(true);
        assert!(!model.start_pairing(0.0));
        assert!(!model.pairing());
        assert!(!model.pair_enabled());

        model.set_paused(false);
        model.start_pairing(0.0);
        model.status(LinkStatus::Pairing, None);
        model.set_paused(true);
        assert!(model.pair_enabled(), "cancel stays available while paused");
        assert_eq!(model.status_text(0.0), "Pairing paused");
        model.cancel_pairing();
        assert_eq!(model.pair_label(), PAIR);
    }

    #[test]
    fn paused_wording_matches_the_mac_app() {
        let mut model = shown();
        model.set_paused(true);
        for (status, text) in [
            (LinkStatus::Searching, "Paused: not looking for a Joy-Con"),
            (LinkStatus::InUseElsewhere, "Paused: not looking for a Joy-Con"),
            (LinkStatus::Connecting, "Connecting to Joy-Con... (paused)"),
            (LinkStatus::Connected, "Connected to Joy-Con (paused)"),
            (LinkStatus::NotPaired, "Not paired: choose Pair New Joy-Con"),
        ] {
            model.status(status, None);
            assert_eq!(model.status_text(0.0), text, "{status:?}");
        }
        model.set_paused(false);
        model.status(LinkStatus::InUseElsewhere, None);
        assert_eq!(model.status_text(0.0), "Joy-Con is in use by another app");
    }

    #[test]
    fn latched_modifiers_use_this_os_key_names_in_pressing_order() {
        let mut model = shown();
        assert_eq!(model.latched_text(), None);
        model.latched(Modifiers::COMMAND.with(Modifiers::SHIFT).with(Modifiers::OPTION));
        let command = if cfg!(windows) { "Win" } else { "Super" };
        assert_eq!(model.latched_text(), Some(format!("Latched: Alt+Shift+{command}")));
        assert!(model.tooltip(0.0).ends_with(&format!("Latched: Alt+Shift+{command}")));
        model.latched(Modifiers::NONE);
        assert_eq!(model.latched_text(), None);
    }

    #[test]
    fn markup_characters_are_escaped_ampersand_first() {
        assert_eq!(escape_markup("<b>Joy & Con</b>"), "&lt;b&gt;Joy &amp; Con&lt;/b&gt;");
        assert_eq!(escape_markup("&lt;"), "&amp;lt;");
        assert_eq!(escape_markup("plain"), "plain");
    }

    #[test]
    fn every_modifier_has_a_label() {
        for (name, ..) in Modifiers::ALL {
            assert_ne!(modifier_label(name), "?", "{name}");
        }
    }

    #[test]
    fn a_failure_replaces_the_status_and_disables_pairing() {
        let mut model = shown();
        model.fail("Cannot post input: /dev/uinput: permission denied");
        model.status(LinkStatus::Searching, None);
        assert_eq!(model.status_text(0.0), "Cannot post input: /dev/uinput: permission denied");
        assert!(!model.pair_enabled());
        assert!(model.tooltip(0.0).starts_with("Deskpuck: Cannot post input"));
    }

    #[test]
    fn a_note_shows_until_the_next_connection() {
        let mut model = shown();
        model.note("Settings: pointer_speed is out of range");
        model.status(LinkStatus::Searching, None);
        assert_eq!(model.note_text(), Some("Settings: pointer_speed is out of range"));
        assert!(model.tooltip(0.0).contains("pointer_speed"));
        model.status(LinkStatus::Connected, None);
        assert_eq!(model.note_text(), None);
    }
}
