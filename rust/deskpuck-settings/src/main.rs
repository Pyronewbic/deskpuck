//! Deskpuck's settings window for Linux and Windows; the tray app opens it.
#![cfg_attr(windows, windows_subsystem = "windows")]

use deskpuck_core::config::Mapping;
use deskpuck_settings::config_path;
use deskpuck_settings::keys::{choices, describe, modifier_label, modifiers};
use deskpuck_settings::model::{DELAY_RANGE, Model, RATE_RANGE, RIGHT_JOYCON, SPEED_RANGE};
use deskpuck_settings::recorder::{Recorded, clipboard_key, record};
use eframe::egui::{self, RichText, Slider, SliderClamping};
use std::ffi::OsString;
use std::process::ExitCode;
use std::time::{Duration, Instant};

const USAGE: &str = "Usage: deskpuck-settings [--config PATH]";

struct App {
    model: Model,
    start: Instant,
    /// A close was stopped because the last change could not be saved; the
    /// next close goes through.
    close_stopped: bool,
    next_file_check: f64,
}

/// How often the window looks for edits made outside it, in seconds.
const FILE_CHECK: f64 = 1.0;

/// Below 1/s (only a hand-edited interval) the rate needs a decimal.
fn rate_text(rate: f64) -> String {
    if rate < 1.0 { format!("{rate:.1}/s") } else { format!("{rate:.0}/s") }
}

fn label(mapping: Option<&Mapping>) -> String {
    mapping.map_or_else(|| "None".to_owned(), describe)
}

/// A slider that leaves a hand-edited value outside its range alone until moved.
fn slider(
    value: &mut f64,
    (min, max, step): (f64, f64, f64),
    format: fn(f64) -> String,
) -> Slider<'_> {
    Slider::new(value, min..=max)
        .step_by(step)
        .clamping(SliderClamping::Edits)
        .custom_formatter(move |n, _| format(n))
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

    fn buttons(&mut self, ui: &mut egui::Ui, now: f64) {
        ui.heading("Buttons");
        egui::Grid::new("buttons").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
            for row in RIGHT_JOYCON {
                ui.label(row.label);
                if self.model.recording == Some(row.id) {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Press a shortcut (Esc cancels; add Super after)").weak(),
                        );
                        if ui.button("Cancel").clicked() {
                            self.model.recording = None;
                        }
                    });
                } else {
                    let current = self.model.mapping(row.id);
                    // Keeps a recorded shortcut or hand-edited key selectable instead of "None".
                    let mut options = choices();
                    if !options.contains(&current) {
                        options.push(current);
                    }
                    let (mut picked, mut start_recording, mut toggled) = (None, false, None);
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt(row.id)
                            .selected_text(label(current.as_ref()))
                            .width(200.0)
                            .show_ui(ui, |ui| {
                                for option in &options {
                                    let text = label(option.as_ref());
                                    if ui.selectable_label(*option == current, text).clicked() {
                                        picked = Some(*option);
                                    }
                                }
                                ui.separator();
                                start_recording =
                                    ui.selectable_label(false, "Record Shortcut...").clicked();
                            });
                        // The modifiers held with a key, including Super, which
                        // the recorder cannot see.
                        if let Some(Mapping::Shortcut(shortcut)) = current {
                            for modifier in modifiers() {
                                let on = shortcut.modifiers.contains(modifier);
                                let text = RichText::new(modifier_label(modifier)).small();
                                if ui.selectable_label(on, text).clicked() {
                                    toggled = Some(modifier);
                                }
                            }
                        }
                    });
                    if let Some(modifier) = toggled {
                        self.model.toggle_modifier(row.id, modifier, now);
                    }
                    if let Some(mapping) = picked.filter(|m| *m != current) {
                        self.model.set_mapping(row.id, mapping, now);
                    }
                    if start_recording {
                        self.model.recording = Some(row.id);
                    }
                }
                ui.end_row();
            }
            for (button, action) in [("R", "Left click"), ("ZR", "Right click")] {
                ui.label(button);
                ui.label(RichText::new(action).weak());
                ui.end_row();
            }
        });
    }

    fn sliders(&mut self, ui: &mut egui::Ui, now: f64) {
        let values = &mut self.model.values;
        let mut changed = false;
        ui.heading("Mouse");
        egui::Grid::new("mouse").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
            ui.label("Pointer speed");
            let speed =
                slider(&mut values.config.pointer_speed, SPEED_RANGE, |n| format!("{n:.2}x"));
            changed |= ui.add(speed).changed();
            ui.end_row();
        });
        changed |=
            ui.checkbox(&mut values.config.scroll_enabled, "Scroll with the stick").changed();

        ui.add_space(12.0);
        ui.heading("Key repeat");
        changed |= ui.checkbox(&mut values.repeat_enabled, "Repeat keys while held").changed();
        if values.repeat_enabled {
            egui::Grid::new("repeat").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
                ui.label("Delay before repeating");
                let delay =
                    slider(&mut values.config.repeat_delay, DELAY_RANGE, |n| format!("{n:.2} s"));
                changed |= ui.add(delay).changed();
                ui.end_row();
                ui.label("Repeat speed");
                let rate = slider(&mut values.repeat_rate, RATE_RANGE, rate_text);
                changed |= ui.add(rate).changed();
                ui.end_row();
            });
        }
        if changed {
            self.model.edited(now);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let now = self.start.elapsed().as_secs_f64();
        self.record_keys(ui, now);
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                if !self.model.load_warnings.is_empty() {
                    ui.label(
                        RichText::new(
                            "Your settings file had problems; defaults were used for these",
                        )
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
                self.buttons(ui, now);
                ui.add_space(12.0);
                self.sliders(ui, now);
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if ui.button("Restore Defaults").clicked() {
                        self.model.restore_defaults(now);
                    }
                    if let Some(error) = &self.model.save_error {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                });
            });
        });
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
        let wait = self.model.save_due_in(now).unwrap_or(FILE_CHECK).min(FILE_CHECK);
        ui.ctx().request_repaint_after(Duration::from_secs_f64(wait));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.model.flush(self.start.elapsed().as_secs_f64());
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let Some(path) = config_path(&args) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let app = App {
        model: Model::load(path),
        start: Instant::now(),
        close_stopped: false,
        next_file_check: FILE_CHECK,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Deskpuck Settings")
            .with_app_id("deskpuck-settings")
            .with_inner_size([470.0, 680.0])
            .with_min_inner_size([380.0, 320.0]),
        ..Default::default()
    };
    match eframe::run_native("Deskpuck Settings", options, Box::new(|_| Ok(Box::new(app)))) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("deskpuck-settings: could not open the window: {e}");
            ExitCode::from(1)
        }
    }
}
