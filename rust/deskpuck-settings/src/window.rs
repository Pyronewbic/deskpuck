use crate::keys::{choices, describe, modifier_label, modifiers};
use crate::model::{DELAY_RANGE, Model, RATE_RANGE, RIGHT_JOYCON, SPEED_RANGE};
use crate::recorder::{Recorded, clipboard_key, record};
use crate::viewport;
use crate::widgets::{section, segments, toggle};
use crate::{fonts, text, theme};
use deskpuck_core::config::{Appearance, Mapping};
use eframe::egui::{self, RichText, Slider, SliderClamping};
use std::path::PathBuf;
use std::time::{Duration, Instant};

struct App {
    model: Model,
    start: Instant,
    close_stopped: bool,
    next_file_check: f64,
    accent: theme::AccentReader,
    focused: bool,
    asked_height: Option<f32>,
}

const FILE_CHECK: f64 = 1.0;
const ACCENT_POLL: f64 = 0.1;

fn label(mapping: Option<&Mapping>) -> String {
    mapping.map_or_else(|| "None".to_owned(), describe)
}

fn slider_row(
    ui: &mut egui::Ui,
    value: &mut f64,
    (min, max, step): (f64, f64, f64),
    format: fn(f64) -> String,
) -> bool {
    let text = format(*value);
    ui.allocate_ui_with_layout(
        egui::vec2(VALUE_WIDTH, ui.spacing().interact_size.y),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            ui.set_min_width(VALUE_WIDTH);
            ui.label(text)
        },
    );
    let slider = Slider::new(value, min..=max)
        .step_by(step)
        .clamping(SliderClamping::Edits)
        .show_value(false);
    ui.add(slider).changed()
}

const VALUE_WIDTH: f32 = 52.0;
const WIDTH: f32 = 440.0;
const MARGIN: i8 = 16;

fn to_preference(appearance: Appearance) -> egui::ThemePreference {
    match appearance {
        Appearance::System => egui::ThemePreference::System,
        Appearance::Light => egui::ThemePreference::Light,
        Appearance::Dark => egui::ThemePreference::Dark,
    }
}

impl App {
    fn record_keys(&mut self, ui: &egui::Ui, now: f64) {
        let Some(button) = self.model.recording else { return };
        let presses: Vec<_> = ui.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Key {
                        key,
                        physical_key,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    } => Some((*key, *physical_key, *modifiers)),
                    _ => clipboard_key(event)
                        .map(|key| (key, None, egui::Modifiers { ctrl: true, ..input.modifiers })),
                })
                .collect()
        });
        for (key, physical_key, held) in presses {
            match record(key, physical_key, held) {
                Recorded::Shortcut(mapping) => {
                    self.model.set_mapping(button, Some(mapping), now);
                    self.model.recording = None;
                    return;
                }
                Recorded::Cancel => {
                    self.model.recording = None;
                    return;
                }
                Recorded::Unknown => {}
            }
        }
    }

    fn button_picker(&mut self, ui: &mut egui::Ui, id: &'static str, now: f64) {
        let current = self.model.mapping(id);
        // Keeps a recorded shortcut or hand-edited key selectable instead of "None".
        let mut options = choices();
        if !options.contains(&current) {
            options.push(current);
        }
        let (mut picked, mut start_recording, mut toggled) = (None, false, None);
        egui::ComboBox::from_id_salt(id)
            .selected_text(label(current.as_ref()))
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show_ui(ui, |ui| {
                for option in &options {
                    if ui.selectable_label(*option == current, label(option.as_ref())).clicked() {
                        picked = Some(*option);
                        ui.close();
                    }
                }
                if let Some(Mapping::Shortcut(shortcut)) = current {
                    ui.separator();
                    ui.label(RichText::new("Hold with the key").small().weak());
                    let all: Vec<_> = modifiers().collect();
                    let names: Vec<_> = all.iter().map(|m| modifier_label(*m)).collect();
                    let on = |i: usize| shortcut.modifiers.contains(all[i]);
                    toggled = segments(ui, (id, "modifiers"), &names, on).map(|i| all[i]);
                }
                ui.separator();
                if ui.selectable_label(false, "Record Shortcut...").clicked() {
                    start_recording = true;
                    ui.close();
                }
            });
        if let Some(modifier) = toggled {
            self.model.toggle_modifier(id, modifier, now);
        }
        if let Some(mapping) = picked.filter(|m| *m != current) {
            self.model.set_mapping(id, mapping, now);
        }
        if start_recording {
            self.model.recording = Some(id);
        }
    }

    fn buttons(&mut self, ui: &mut egui::Ui, now: f64) {
        section(ui, "Buttons", |card| {
            for row in RIGHT_JOYCON {
                card.row(row.label, |ui| {
                    if self.model.recording == Some(row.id) {
                        if ui.button("Cancel").clicked() {
                            self.model.recording = None;
                        }
                        ui.label(RichText::new("Press a shortcut (Esc cancels)").weak());
                    } else {
                        self.button_picker(ui, row.id, now);
                    }
                });
            }
            for (button, action) in [("R", "Left click"), ("ZR", "Right click")] {
                card.row(button, |ui| ui.label(RichText::new(action).weak()));
            }
        });
    }

    fn controls(&mut self, ui: &mut egui::Ui, now: f64) {
        let values = &mut self.model.values;
        let mut changed = false;
        section(ui, "Mouse", |card| {
            changed |= card.row("Pointer speed", |ui| {
                slider_row(ui, &mut values.config.pointer_speed, SPEED_RANGE, text::speed)
            });
            changed |= card.row("Scroll with the stick", |ui| {
                toggle(ui, &mut values.config.scroll_enabled, "Scroll with the stick").changed()
            });
        });
        section(ui, "Key repeat", |card| {
            changed |= card.row("Repeat keys while held", |ui| {
                toggle(ui, &mut values.repeat_enabled, "Repeat keys while held").changed()
            });
            if values.repeat_enabled {
                changed |= card.row("Delay before repeating", |ui| {
                    slider_row(ui, &mut values.config.repeat_delay, DELAY_RANGE, text::delay)
                });
                changed |= card.row("Repeat speed", |ui| {
                    slider_row(ui, &mut values.repeat_rate, RATE_RANGE, text::rate)
                });
            }
        });
        section(ui, "Appearance", |card| {
            card.row("Theme", |ui| {
                let all = Appearance::ALL;
                let current = values.config.appearance;
                if let Some(i) =
                    segments(ui, "appearance", &["System", "Light", "Dark"], |i| all[i] == current)
                    && all[i] != current
                {
                    values.config.appearance = all[i];
                    changed = true;
                }
            });
        });
        if changed {
            self.model.edited(now);
        }
    }

    fn problems(&self, ui: &mut egui::Ui) {
        if !self.model.load_warnings.is_empty() {
            ui.label(
                RichText::new("Your settings file had problems; defaults were used for these")
                    .strong(),
            );
            for warning in &self.model.load_warnings {
                ui.colored_label(ui.visuals().warn_fg_color, warning);
            }
            if self.model.backs_up() {
                ui.label("Your first change keeps a copy of the file as config.json.bak.");
            }
            ui.add_space(12.0);
        }
        if let Some(notice) = &self.model.notice {
            ui.colored_label(ui.visuals().warn_fg_color, notice);
            ui.add_space(12.0);
        }
    }

    fn fit_height(&mut self, ui: &egui::Ui, content: f32) {
        let (inner, outer, monitor) = ui.input(|i| {
            let viewport = i.viewport();
            (viewport.inner_rect, viewport.outer_rect, viewport.monitor_size)
        });
        // Leaves room for a taskbar or panel, which the monitor size includes.
        let limit = monitor.map_or(f32::INFINITY, |m| m.y * 0.85);
        let wanted = (content + 2.0 * f32::from(MARGIN)).min(limit).round();
        let current = inner.map(|r| r.height().round());
        if current == Some(wanted) || self.asked_height == Some(wanted) {
            return;
        }
        let first = self.asked_height.is_none();
        self.asked_height = Some(wanted);
        let ctx = ui.ctx();
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(WIDTH, wanted)));
        // The window opened centred at its first guess; centre it again at its real height.
        if first && let (Some(outer), Some(inner), Some(monitor)) = (outer, inner, monitor) {
            let frame = outer.height() - inner.height();
            let top = ((monitor.y - wanted - frame) / 2.0).max(0.0);
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                outer.min.x,
                top,
            )));
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let now = self.start.elapsed().as_secs_f64();
        let focused = ui.input(|i| i.focused);
        if focused && !self.focused {
            self.accent.refresh();
        }
        self.focused = focused;
        if let Some(accent) = self.accent.poll() {
            theme::apply(ui.ctx(), accent);
        }
        let preference = to_preference(self.model.values.config.appearance);
        if ui.ctx().options(|o| o.theme_preference) != preference {
            ui.ctx().set_theme(preference);
        }
        self.record_keys(ui, now);
        let frame = egui::Frame::central_panel(ui.style()).inner_margin(MARGIN);
        let content = egui::CentralPanel::default()
            .frame(frame)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .show(ui, |ui| {
                        self.problems(ui);
                        self.buttons(ui, now);
                        self.controls(ui, now);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Restore Defaults").clicked() {
                                self.model.restore_defaults(now);
                            }
                            if let Some(error) = &self.model.save_error {
                                ui.colored_label(ui.visuals().error_fg_color, error);
                            }
                        });
                    })
                    .content_size
                    .y
            })
            .inner;
        self.fit_height(ui, content);
        if now >= self.next_file_check {
            self.next_file_check = now + FILE_CHECK;
            self.model.check_file();
        }
        self.model.save_if_due(now);
        // Closing saves now; if that fails, the window stays open once to show why.
        if ui.input(|i| i.viewport().close_requested())
            && !self.model.flush(now)
            && self.model.save_error.is_some()
            && !self.close_stopped
        {
            self.close_stopped = true;
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        let mut wait = self.model.save_due_in(now).unwrap_or(FILE_CHECK).min(FILE_CHECK);
        if self.accent.reading() {
            wait = wait.min(ACCENT_POLL);
        }
        ui.ctx().request_repaint_after(Duration::from_secs_f64(wait));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.model.flush(self.start.elapsed().as_secs_f64());
    }
}

pub fn run(path: Option<PathBuf>) -> Result<(), String> {
    let app = App {
        model: Model::load(path),
        start: Instant::now(),
        close_stopped: false,
        next_file_check: FILE_CHECK,
        accent: theme::AccentReader::default(),
        focused: false,
        asked_height: None,
    };
    let icon =
        eframe::icon_data::from_png_bytes(include_bytes!("../../icons/deskpuck-128.png")).ok();
    let options = eframe::NativeOptions {
        viewport: viewport(WIDTH, icon),
        // Fitted to its content, it is tall; centred, it clears taskbars and panels.
        centered: true,
        ..Default::default()
    };
    let creator = Box::new(|cc: &eframe::CreationContext| {
        fonts::install(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx, theme::Accent::BRAND);
        Ok(Box::new(app) as Box<dyn eframe::App>)
    });
    eframe::run_native("Deskpuck Settings", options, creator)
        .map_err(|e| format!("could not open the window: {e}"))
}
