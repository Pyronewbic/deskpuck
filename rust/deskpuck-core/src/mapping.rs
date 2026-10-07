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

/// Mouse button bits: 1 left, 2 right, 4 middle.
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const NONE: Modifiers = Modifiers(0);
    pub const CONTROL: Modifiers = Modifiers(1);
    pub const OPTION: Modifiers = Modifiers(2);
    pub const SHIFT: Modifiers = Modifiers(4);
    pub const COMMAND: Modifiers = Modifiers(8);

    /// In the order they are pressed.
    pub const ALL: [(&'static str, Modifiers, KeyCode); 4] = [
        ("control", Modifiers::CONTROL, 59),
        ("option", Modifiers::OPTION, 58),
        ("shift", Modifiers::SHIFT, 56),
        ("command", Modifiers::COMMAND, 55),
    ];

    pub fn from_name(name: &str) -> Option<Modifiers> {
        Self::ALL.iter().find(|(n, ..)| *n == name).map(|(_, m, _)| *m)
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Values are part of the C interface (DP_MODIFIER_*).
    pub fn bits(self) -> u8 {
        self.0
    }

    pub fn contains(self, other: Modifiers) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn with(self, other: Modifiers) -> Modifiers {
        Modifiers(self.0 | other.0)
    }

    pub fn names(self) -> Vec<&'static str> {
        Self::ALL.iter().filter(|(_, m, _)| self.contains(*m)).map(|(n, ..)| *n).collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonAction {
    /// A key with modifiers is a shortcut: it never repeats.
    Key {
        key_code: KeyCode,
        modifiers: Modifiers,
    },
    Modifier {
        modifiers: Modifiers,
        latch: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonKeyMapping {
    pub button_mask: u32,
    pub action: ButtonAction,
}

impl ButtonKeyMapping {
    pub fn key(button_mask: u32, key_code: KeyCode) -> Self {
        Self::shortcut(button_mask, key_code, Modifiers::NONE)
    }

    pub fn shortcut(button_mask: u32, key_code: KeyCode, modifiers: Modifiers) -> Self {
        Self { button_mask, action: ButtonAction::Key { key_code, modifiers } }
    }

    pub fn modifier(button_mask: u32, modifiers: Modifiers, latch: bool) -> Self {
        Self { button_mask, action: ButtonAction::Modifier { modifiers, latch } }
    }

    pub fn key_code(&self) -> Option<KeyCode> {
        match self.action {
            ButtonAction::Key { key_code, .. } => Some(key_code),
            ButtonAction::Modifier { .. } => None,
        }
    }
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
    .map(|(button_mask, key_code)| ButtonKeyMapping::key(button_mask, key_code))
    .collect()
}

/// Repeat is off unless repeat_delay >= 0 and repeat_interval > 0 (NaN counts as off).
/// At most one repeat per key per update, so a stalled packet stream never bursts.
#[derive(Clone, Debug)]
pub struct KeyRepeater {
    mappings: Vec<ButtonKeyMapping>,
    next_repeat_at: Vec<f64>,
    latched: Vec<bool>,
    repeat_delay: f64,
    repeat_interval: f64,
    repeat_enabled: bool,
    last_buttons: u32,
    /// Per `Modifiers::ALL` entry, how many shortcuts and modifier buttons
    /// hold it, so Control goes down with the first and up with the last.
    modifier_holds: [u8; 4],
}

impl KeyRepeater {
    pub fn new(mappings: Vec<ButtonKeyMapping>, repeat_delay: f64, repeat_interval: f64) -> Self {
        Self {
            next_repeat_at: vec![0.0; mappings.len()],
            latched: vec![false; mappings.len()],
            mappings,
            repeat_delay,
            repeat_interval,
            repeat_enabled: repeat_delay >= 0.0 && repeat_interval > 0.0,
            last_buttons: 0,
            modifier_holds: [0; 4],
        }
    }

    fn key(key_code: KeyCode, is_down: bool) -> KeyEvent {
        KeyEvent { key_code, is_down, is_repeat: false }
    }

    fn hold(&mut self, modifiers: Modifiers, events: &mut Vec<KeyEvent>) {
        for (i, (_, modifier, code)) in Modifiers::ALL.iter().enumerate() {
            if modifiers.contains(*modifier) {
                self.modifier_holds[i] += 1;
                if self.modifier_holds[i] == 1 {
                    events.push(Self::key(*code, true));
                }
            }
        }
    }

    fn unhold(&mut self, modifiers: Modifiers, events: &mut Vec<KeyEvent>) {
        for (i, (_, modifier, code)) in Modifiers::ALL.iter().enumerate().rev() {
            if modifiers.contains(*modifier) && self.modifier_holds[i] > 0 {
                self.modifier_holds[i] -= 1;
                if self.modifier_holds[i] == 0 {
                    events.push(Self::key(*code, false));
                }
            }
        }
    }

    /// Modifier buttons go down before keys and up after them, so a modifier
    /// and a key pressed in the same report make a shortcut.
    pub fn update(&mut self, buttons: u32, now: f64) -> Vec<KeyEvent> {
        let mut events = Vec::new();
        let changed = |m: &ButtonKeyMapping| {
            let is_down = buttons & m.button_mask != 0;
            (is_down, is_down != (self.last_buttons & m.button_mask != 0))
        };
        let edges: Vec<(bool, bool)> = self.mappings.iter().map(changed).collect();

        for (i, &(is_down, changed)) in edges.iter().enumerate() {
            if let ButtonAction::Modifier { modifiers, latch } = self.mappings[i].action
                && changed
                && is_down
            {
                if !latch {
                    self.hold(modifiers, &mut events);
                } else if self.latched[i] {
                    self.latched[i] = false;
                    self.unhold(modifiers, &mut events);
                } else {
                    self.latched[i] = true;
                    self.hold(modifiers, &mut events);
                }
            }
        }
        for (i, &(is_down, changed)) in edges.iter().enumerate() {
            let ButtonAction::Key { key_code, modifiers } = self.mappings[i].action else {
                continue;
            };
            if changed {
                if is_down {
                    self.hold(modifiers, &mut events);
                    events.push(Self::key(key_code, true));
                } else {
                    events.push(Self::key(key_code, false));
                    self.unhold(modifiers, &mut events);
                }
                self.next_repeat_at[i] = now + self.repeat_delay;
            } else if is_down
                && modifiers.is_empty()
                && self.repeat_enabled
                && now >= self.next_repeat_at[i]
            {
                events.push(KeyEvent { key_code, is_down: true, is_repeat: true });
                self.next_repeat_at[i] = now + self.repeat_interval;
            }
        }
        for (i, &(is_down, changed)) in edges.iter().enumerate() {
            if let ButtonAction::Modifier { modifiers, latch: false } = self.mappings[i].action
                && changed
                && !is_down
            {
                self.unhold(modifiers, &mut events);
            }
        }
        self.last_buttons = buttons;
        events
    }

    pub fn latched(&self) -> Modifiers {
        self.mappings.iter().zip(&self.latched).filter(|(_, on)| **on).fold(
            Modifiers::NONE,
            |all, (m, _)| match m.action {
                ButtonAction::Modifier { modifiers, .. } => all.with(modifiers),
                ButtonAction::Key { .. } => all,
            },
        )
    }

    /// Latched modifiers are released too.
    pub fn release_all(&mut self) -> Vec<KeyEvent> {
        let mut events = Vec::new();
        for i in 0..self.mappings.len() {
            let held = self.last_buttons & self.mappings[i].button_mask != 0;
            match self.mappings[i].action {
                ButtonAction::Key { key_code, modifiers } if held => {
                    events.push(Self::key(key_code, false));
                    self.unhold(modifiers, &mut events);
                }
                ButtonAction::Modifier { modifiers, latch: false } if held => {
                    self.unhold(modifiers, &mut events);
                }
                ButtonAction::Modifier { modifiers, latch: true } if self.latched[i] => {
                    self.latched[i] = false;
                    self.unhold(modifiers, &mut events);
                }
                _ => {}
            }
        }
        self.last_buttons = 0;
        events
    }
}
