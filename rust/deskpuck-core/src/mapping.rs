/// macOS virtual key code. This is the canonical key space of the config file;
/// backends on other platforms translate from it.
pub type KeyCode = u16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn max_x(&self) -> f64 {
        self.x + self.width
    }

    pub fn max_y(&self) -> f64 {
        self.y + self.height
    }

    /// Half-open like CGRectContainsPoint: the max edges are outside.
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.max_x() && p.y >= self.y && p.y < self.max_y()
    }
}

/// The cursor may cross onto any display; off all displays it is clamped to the
/// display it came from, stopping on the last pixel so edge triggers (Dock) fire.
pub fn clamp_to_displays(
    target: Point,
    from: Point,
    display_at: impl Fn(Point) -> Option<Rect>,
    fallback: Rect,
) -> Point {
    if display_at(target).is_some() {
        return target;
    }
    let bounds = display_at(from).unwrap_or(fallback);
    Point {
        x: target.x.min(bounds.max_x() - 1.0).max(bounds.x),
        y: target.y.min(bounds.max_y() - 1.0).max(bounds.y),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// What a pointer move must be posted as. macOS hot corners and the Dock only
/// react to the correct kind, so backends must not collapse drags into moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveKind {
    Moved,
    Dragged(MouseButton),
}

/// Mouse button bits: 1 left, 2 right, 4 middle. Left wins when several are held.
pub fn mouse_move_kind(mouse_buttons: u8) -> MoveKind {
    if mouse_buttons & 1 != 0 {
        MoveKind::Dragged(MouseButton::Left)
    } else if mouse_buttons & 2 != 0 {
        MoveKind::Dragged(MouseButton::Right)
    } else if mouse_buttons & 4 != 0 {
        MoveKind::Dragged(MouseButton::Middle)
    } else {
        MoveKind::Moved
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonKeyMapping {
    pub button_mask: u32,
    pub key_code: KeyCode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key_code: KeyCode,
    pub is_down: bool,
    pub is_repeat: bool,
}

pub fn default_button_key_mappings() -> Vec<ButtonKeyMapping> {
    [
        (0x0004_0000, 36),  // RS -> Return
        (0x0000_0200, 126), // X -> Up
        (0x0000_0400, 125), // B -> Down
        (0x0000_0100, 123), // Y -> Left
        (0x0000_0800, 124), // A -> Right
    ]
    .into_iter()
    .map(|(button_mask, key_code)| ButtonKeyMapping { button_mask, key_code })
    .collect()
}

/// Turns button states into key down/up events, repeating held keys like a keyboard.
/// Repeat is off unless repeat_delay >= 0 and repeat_interval > 0 (NaN counts as off).
/// At most one repeat per key per update, so a stalled packet stream never bursts.
#[derive(Clone, Debug)]
pub struct KeyRepeater {
    mappings: Vec<ButtonKeyMapping>,
    next_repeat_at: Vec<f64>,
    repeat_delay: f64,
    repeat_interval: f64,
    repeat_enabled: bool,
    last_buttons: u32,
}

impl KeyRepeater {
    pub fn new(mappings: Vec<ButtonKeyMapping>, repeat_delay: f64, repeat_interval: f64) -> Self {
        Self {
            next_repeat_at: vec![0.0; mappings.len()],
            mappings,
            repeat_delay,
            repeat_interval,
            repeat_enabled: repeat_delay >= 0.0 && repeat_interval > 0.0,
            last_buttons: 0,
        }
    }

    pub fn update(&mut self, buttons: u32, now: f64) -> Vec<KeyEvent> {
        let mut events = Vec::new();
        for (mapping, next) in self.mappings.iter().zip(self.next_repeat_at.iter_mut()) {
            let is_down = buttons & mapping.button_mask != 0;
            let was_down = self.last_buttons & mapping.button_mask != 0;
            if is_down != was_down {
                events.push(KeyEvent { key_code: mapping.key_code, is_down, is_repeat: false });
                *next = now + self.repeat_delay;
            } else if is_down && self.repeat_enabled && now >= *next {
                events.push(KeyEvent {
                    key_code: mapping.key_code,
                    is_down: true,
                    is_repeat: true,
                });
                *next = now + self.repeat_interval;
            }
        }
        self.last_buttons = buttons;
        events
    }

    /// Key-up for every held key, e.g. when the controller disconnects mid-press.
    pub fn release_all(&mut self) -> Vec<KeyEvent> {
        let events = self
            .mappings
            .iter()
            .filter(|m| self.last_buttons & m.button_mask != 0)
            .map(|m| KeyEvent { key_code: m.key_code, is_down: false, is_repeat: false })
            .collect();
        self.last_buttons = 0;
        events
    }
}
