//! The Settings dialog, opened from the header's menu: the app-wide sync and archive settings and
//! the update check. The Git-or-Pijul choice is per file and stays the question asked on open.

use eframe::egui::{self, Align, Color32, Layout, Modal, RichText, Ui};

use crate::config::AppConfig;
use crate::theme::{self, Palette};

/// The longest interval the dialog offers, a day. A longer one set in `config.toml` is kept until
/// the field is edited.
const MAX_INTERVAL: u64 = 24 * 60;

/// The longest archive age the dialog offers, ten years; a longer one is kept the same way.
const MAX_ARCHIVE_DAYS: u64 = 3650;

/// The dialog's fields, copied from the config when it opens and written back on Save.
pub struct SettingsForm {
    pub sync: bool,
    pub interval: u64,
    pub auto_archive: bool,
    pub archive_days: u64,
    pub check_for_updates: bool,
    /// The beta channel; off is stable.
    pub beta: bool,
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
            auto_archive: config.auto_archive,
            archive_days: config.archive_older_than_days,
            check_for_updates: config.check_for_updates,
            beta: config.beta(),
        }
    }

    /// Writes the fields to `config`; every other key is left as it is.
    pub fn apply(&self, config: &mut AppConfig) {
        config.sync = self.sync;
        config.sync_interval_minutes = self.interval;
        config.auto_archive = self.auto_archive;
        config.archive_older_than_days = self.archive_days;
        config.check_for_updates = self.check_for_updates;
        config.update_channel = if self.beta { "beta" } else { "stable" }.to_owned();
    }
}

/// The modal, framed like the edit dialog. Escape and a click on the backdrop cancel.
pub fn show(ctx: &egui::Context, p: &Palette, form: &mut SettingsForm) -> Option<Outcome> {
    let frame = theme::dialog_frame(p);
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
    archive_ui(ui, p, form);
    // Builds without the update check (the Nix package) have nothing to set.
    if cfg!(feature = "update-check") {
        updates_ui(ui, p, form);
    }
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

fn archive_ui(ui: &mut Ui, p: &Palette, form: &mut SettingsForm) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.checkbox(
            &mut form.auto_archive,
            RichText::new("Archive done items automatically").size(12.6),
        );
        // The age stays editable with the checkbox off: the menu's Archive Done Items uses it.
        ui.horizontal(|ui| {
            let label = ui.label(RichText::new("Older than").size(12.6));
            ui.add(
                egui::DragValue::new(&mut form.archive_days)
                    .range(0..=MAX_ARCHIVE_DAYS)
                    .clamp_existing_to_range(false)
                    .suffix(" days"),
            )
            .labelled_by(label.id);
        });
        hint(
            ui,
            p,
            "Moves done items to done.txt (or <name>.done.txt) next to the file, on open and once \
             a day. 0: everything completed before today.",
        );
    });
}

fn updates_ui(ui: &mut Ui, p: &Palette, form: &mut SettingsForm) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.checkbox(
            &mut form.check_for_updates,
            RichText::new("Check for updates").size(12.6),
        );
        hint(
            ui,
            p,
            "Asks rtm.brcha.com once a day whether a newer release exists; nothing about you or \
             your files is sent.",
        );
    });
    ui.add_enabled_ui(form.check_for_updates, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Channel").size(12.6));
            ui.radio_value(&mut form.beta, false, RichText::new("Stable").size(12.6));
            ui.radio_value(&mut form.beta, true, RichText::new("Beta").size(12.6));
        });
    });
}

fn hint(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(RichText::new(text).size(11.9).color(p.muted));
}

#[cfg(test)]
#[path = "tests/settings_form_tests.rs"]
mod form_tests;
