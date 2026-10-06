use crate::mapping::{
    ButtonKeyMapping, KeyEvent, KeyRepeater, Modifiers, default_button_key_mappings,
};
use crate::packet::Report;

/// Raw optical-sensor counts per screen point at pointer speed 1.0.
pub const MOUSE_COUNTS_PER_POINT: f64 = 5.0;

const BUTTON_R: u32 = 0x0000_4000;
const BUTTON_ZR: u32 = 0x0000_8000;
const BUTTON_RS: u32 = 0x0004_0000;
const BUTTON_LS: u32 = 0x0008_0000;
const BUTTON_L: u32 = 0x4000_0000;
const BUTTON_ZL: u32 = 0x8000_0000;

#[derive(Clone, Debug, PartialEq)]
pub struct EngineSettings {
    pub key_mappings: Vec<ButtonKeyMapping>,
    pub pointer_speed: f64,
    pub repeat_delay: f64,
    pub repeat_interval: f64,
    pub scroll_enabled: bool,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            key_mappings: default_button_key_mappings(),
            pointer_speed: 1.0,
            repeat_delay: 0.4,
            repeat_interval: 0.06,
            scroll_enabled: true,
        }
    }
}

/// Mouse button bits as used by `mouse_move_kind`: 1 left, 2 right, 4 middle.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EngineOutput {
    pub dx: f64,
    pub dy: f64,
    pub mouse_buttons: u8,
    pub wheel: i32,
    pub keys: Vec<KeyEvent>,
}

pub fn mouse_buttons_for_joycon_buttons(buttons: u32) -> u8 {
    let mut mouse = 0;
    if buttons & (BUTTON_R | BUTTON_ZL) != 0 {
        mouse |= 1;
    }
    if buttons & (BUTTON_ZR | BUTTON_L) != 0 {
        mouse |= 2;
    }
    if buttons & BUTTON_LS != 0 {
        mouse |= 4;
    }
    mouse
}

/// 20 speed levels of 60 stick units each; the first 60 units are the deadzone.
/// Stick up scrolls up.
pub fn wheel_for_stick_deviation(deviation: i32) -> i32 {
    let speed = (deviation.saturating_abs() / 60).min(20) * 5;
    if deviation > 0 { -speed } else { speed }
}

/// Turns Joy-Con reports into pointer, click, scroll and key output. No OS calls.
#[derive(Clone, Debug)]
pub struct InputEngine {
    settings: EngineSettings,
    keys: KeyRepeater,
    last_mouse: Option<(i16, i16)>,
    stick_centre_y: Option<(i32, i32)>,
}

impl Default for InputEngine {
    fn default() -> Self {
        Self::new(EngineSettings::default())
    }
}

impl InputEngine {
    pub fn new(settings: EngineSettings) -> Self {
        let keys = repeater_for(&settings);
        Self { settings, keys, last_mouse: None, stick_centre_y: None }
    }

    /// Call when a controller connects; the next report recentres the scroll stick
    /// and becomes the baseline for pointer motion.
    pub fn connection_started(&mut self) {
        self.last_mouse = None;
        self.stick_centre_y = None;
    }

    pub fn process(&mut self, report: &Report, now: f64) -> EngineOutput {
        let (last_x, last_y) = self.last_mouse.unwrap_or((report.mouse_x, report.mouse_y));
        // The sensor counters are 16-bit and wrap.
        let raw_dx = report.mouse_x.wrapping_sub(last_x);
        let raw_dy = report.mouse_y.wrapping_sub(last_y);
        self.last_mouse = Some((report.mouse_x, report.mouse_y));

        let (left_centre, right_centre) = *self
            .stick_centre_y
            .get_or_insert((i32::from(report.left_stick_y), i32::from(report.right_stick_y)));

        let mut wheel = 0;
        if self.settings.scroll_enabled {
            // A stick being clicked also tilts a little; don't let that scroll.
            if report.buttons & BUTTON_LS == 0 {
                wheel += wheel_for_stick_deviation(i32::from(report.left_stick_y) - left_centre);
            }
            if report.buttons & BUTTON_RS == 0 {
                wheel += wheel_for_stick_deviation(i32::from(report.right_stick_y) - right_centre);
            }
            wheel = wheel.clamp(-127, 127);
        }

        EngineOutput {
            dx: f64::from(raw_dx) * self.settings.pointer_speed / MOUSE_COUNTS_PER_POINT,
            dy: f64::from(raw_dy) * self.settings.pointer_speed / MOUSE_COUNTS_PER_POINT,
            mouse_buttons: mouse_buttons_for_joycon_buttons(report.buttons),
            wheel,
            keys: self.keys.update(report.buttons, now),
        }
    }

    /// Modifiers toggled on by latching modifier buttons.
    pub fn latched(&self) -> Modifiers {
        self.keys.latched()
    }

    /// Releases every held key. Mouse buttons in the output are all up.
    pub fn disconnected(&mut self) -> EngineOutput {
        EngineOutput { keys: self.keys.release_all(), ..EngineOutput::default() }
    }

    /// Swaps settings mid-session. Returns key-ups for keys held under the old
    /// mapping; pointer baseline and scroll centre carry over.
    pub fn apply_settings(&mut self, settings: EngineSettings) -> Vec<KeyEvent> {
        let released = self.keys.release_all();
        self.keys = repeater_for(&settings);
        self.settings = settings;
        released
    }
}

fn repeater_for(settings: &EngineSettings) -> KeyRepeater {
    KeyRepeater::new(settings.key_mappings.clone(), settings.repeat_delay, settings.repeat_interval)
}
