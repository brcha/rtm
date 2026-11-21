//! The archive settings of one file, opened from its tab's context menu: whether it follows the
//! app-wide archive settings or has its own, and where its archive is.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Align, Color32, Layout, Modal, RichText, Ui};

use crate::config::AppConfig;
use crate::settings::{self, Outcome};
use crate::theme::{self, Palette};

/// The dialog's fields, from the config when it opens, written to the file's entry on Save.
pub struct ArchiveForm {
    /// The todo file, as its entry names it.
    pub path: PathBuf,
    /// Its tab's label.
    pub name: String,
    pub archive: PathBuf,
    pub use_app_wide: bool,
    pub auto_archive: bool,
    pub days: u64,
    /// The app-wide settings, for the checkbox's label.
    app_wide: (bool, u64),
}

impl ArchiveForm {
    pub fn new(config: &AppConfig, path: &Path, name: &str, archive: PathBuf) -> Self {
        let entry = config.entry(path);
        let (auto_archive, days) = config.archive_rule(path);
        ArchiveForm {
            path: path.to_path_buf(),
            name: name.to_owned(),
            archive,
            use_app_wide: entry
                .is_none_or(|e| e.auto_archive.is_none() && e.archive_older_than_days.is_none()),
            auto_archive,
            days,
            app_wide: (config.auto_archive, config.archive_older_than_days),
        }
    }

    /// Writes the fields to the file's entry; "use the app-wide setting" clears its own.
    pub fn apply(&self, config: &mut AppConfig) {
        let rule = (!self.use_app_wide).then_some((self.auto_archive, self.days));
        config.set_archive(&self.path.to_string_lossy(), rule);
    }
}

/// The modal, framed like the Settings dialog. `ignored_by` names the VCS that ignores the
/// archive, when its last check found it ignored. Escape and a click on the backdrop cancel.
pub fn show(
    ctx: &egui::Context,
    p: &Palette,
    form: &mut ArchiveForm,
    ignored_by: Option<&str>,
) -> Option<Outcome> {
    let mut outcome = None;
    let modal = Modal::new(egui::Id::unique("archive-dialog"))
        .frame(theme::dialog_frame(p))
        .backdrop_color(Color32::from_black_alpha(128))
        .show(ctx, |ui| {
            ui.set_width(452.0);
            outcome = form_ui(ui, p, form, ignored_by);
        });
    if modal.should_close() {
        outcome = Some(Outcome::Cancel);
    }
    outcome
}

fn form_ui(
    ui: &mut Ui,
    p: &Palette,
    form: &mut ArchiveForm,
    ignored_by: Option<&str>,
) -> Option<Outcome> {
    ui.spacing_mut().item_spacing = egui::vec2(8.0, 16.0);
    ui.label(
        RichText::new(format!("Archive settings for {}", form.name))
            .font(theme::bold(17.5))
            .color(p.text),
    );
    let (auto, days) = form.app_wide;
    let state = if auto { "on" } else { "off" };
    ui.checkbox(
        &mut form.use_app_wide,
        RichText::new(format!("Use the app-wide setting ({state}, {days} days)")).size(12.6),
    );
    ui.add_enabled_ui(!form.use_app_wide, |ui| {
        settings::archive_fields(ui, &mut form.auto_archive, &mut form.days);
    });
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.label(
            RichText::new(format!("Archive: {}", form.archive.display()))
                .size(11.9)
                .color(p.muted),
        );
        if let Some(vcs) = ignored_by {
            let name = form
                .archive
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            ui.label(
                RichText::new(format!(
                    "{name} is ignored by {vcs}; archived items stay on this computer."
                ))
                .size(11.9)
                .color(p.muted),
            );
        }
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

#[cfg(test)]
#[path = "tests/archive_form_tests.rs"]
mod form_tests;
