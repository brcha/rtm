//! The Settings dialog (RTM-134), opened from the header's menu: the app-wide sync settings. The
//! Git-or-Pijul choice is per file (RTM-124) and stays the question asked on open.

use eframe::egui::{self, Align, Color32, Frame, Layout, Modal, RichText, Stroke, Ui};

use crate::config::AppConfig;
use crate::theme::{self, Palette};

/// The longest interval the dialog offers, a day. A longer one set in `config.toml` is kept until
/// the field is edited.
const MAX_INTERVAL: u64 = 24 * 60;

/// The dialog's fields, copied from the config when it opens and written back on Save.
pub struct SettingsForm {
    pub sync: bool,
    pub interval: u64,
}

pub enum Outcome {
    Save,
    Cancel,
}

impl SettingsForm {
    pub fn new(config: &AppConfig) -> Self {
        SettingsForm {
            sync: config.sync,
            interval: config.sync_interval_minutes,
        }
    }

    /// Writes the fields to `config`; every other key is left as it is.
    pub fn apply(&self, config: &mut AppConfig) {
        config.sync = self.sync;
        config.sync_interval_minutes = self.interval;
    }
}

/// The modal, framed like the edit dialog. Escape and a click on the backdrop cancel.
pub fn show(ctx: &egui::Context, p: &Palette, form: &mut SettingsForm) -> Option<Outcome> {
    let frame = Frame::new()
        .fill(p.dialog_bg)
        .stroke(Stroke::new(1.0, p.dialog_border))
        .corner_radius(8.0)
        .inner_margin(24.0)
        .shadow(egui::Shadow {
            offset: [0, 4],
            blur: 20,
            spread: 0,
            color: Color32::from_black_alpha(51),
        });
    let mut outcome = None;
    let modal = Modal::new(egui::Id::unique("settings-dialog"))
        .frame(frame)
        .backdrop_color(Color32::from_black_alpha(128))
        .show(ctx, |ui| {
            ui.set_width(452.0);
            outcome = form_ui(ui, p, form);
        });
    if modal.should_close() {
        outcome = Some(Outcome::Cancel);
    }
    outcome
}

fn form_ui(ui: &mut Ui, p: &Palette, form: &mut SettingsForm) -> Option<Outcome> {
    ui.spacing_mut().item_spacing = egui::vec2(8.0, 16.0);
    ui.label(
        RichText::new("Settings")
            .font(theme::bold(17.5))
            .color(p.text),
    );
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.checkbox(
            &mut form.sync,
            RichText::new("Sync through the file's repository").size(12.6),
        );
        hint(
            ui,
            p,
            "Commits every change and pulls and pushes when the todo.txt is tracked in Git or Pijul.",
        );
    });
    ui.add_enabled_ui(form.sync, |ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.horizontal(|ui| {
                let label = ui.label(RichText::new("Sync every").size(12.6));
                ui.add(
                    egui::DragValue::new(&mut form.interval)
                        .range(0..=MAX_INTERVAL)
                        .clamp_existing_to_range(false)
                        .suffix(" minutes"),
                )
                .labelled_by(label.id);
            });
            hint(ui, p, "0: only on open, on close and with Sync now.");
        });
    });
    ui.add_space(4.0);
    let mut outcome = None;
    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        if theme::success_button(ui, "Save").clicked() {
            outcome = Some(Outcome::Save);
        }
        if theme::secondary_button(ui, "Cancel").clicked() {
            outcome = Some(Outcome::Cancel);
        }
    });
    outcome
}

fn hint(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(RichText::new(text).size(11.9).color(p.muted));
}

#[cfg(test)]
#[path = "tests/settings_form_tests.rs"]
mod form_tests;
