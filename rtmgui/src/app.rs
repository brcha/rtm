use std::path::{Path, PathBuf};

use eframe::egui::{
    self, Align, Color32, Frame, Layout, Margin, RichText, Stroke, Ui,
    text::{LayoutJob, TextFormat},
};
use todotxt::TodoLibrary;

use crate::config::{self, AppConfig};
use crate::list;
use crate::theme::{self, Palette};

pub struct App {
    config: AppConfig,
    config_path: PathBuf,
    library: Option<TodoLibrary>,
    /// Shown above the list until the next successful action.
    error: Option<String>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        let config_path = config::config_path();
        let config = AppConfig::load(&config_path);
        let mut error = None;
        let library = match config.file_name.as_deref().map(Path::new) {
            Some(path) if path.exists() => match load_library(path) {
                Ok(library) => Some(library),
                Err(e) => {
                    error = Some(e);
                    None
                }
            },
            _ => None,
        };
        App {
            config,
            config_path,
            library,
            error,
        }
    }

    /// Asks for a file and loads it. The dialog blocks the UI thread while it is open.
    fn pick_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Text Files", &["txt"])
            .pick_file()
        else {
            return;
        };
        match load_library(&path) {
            Ok(library) => {
                self.config.file_name = Some(library.file_name.clone());
                self.library = Some(library);
                self.save_config();
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Writes the config; replaces the error line with the result.
    fn save_config(&mut self) {
        self.error = self
            .config
            .save(&self.config_path)
            .err()
            .map(|e| format!("Could not save the config: {e}"));
    }

    /// The four view settings. A change is saved at once, as in `rtmapp`.
    fn filters(&mut self, ui: &mut Ui) {
        let mut changed = false;
        ui.scope(|ui| {
            // CSS: 16 px boxes, 8 px gaps, 0.9rem labels.
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.spacing_mut().icon_width = 16.0;
            ui.spacing_mut().icon_spacing = 8.0;
            let config = &mut self.config;
            for (value, label) in [
                (&mut config.show_completed_items, "Show completed items"),
                (&mut config.show_future_items, "Show future items"),
                (&mut config.hide_no_date, "Hide items without due date"),
                (&mut config.reverse_sort, "Reverse sort order"),
            ] {
                changed |= ui
                    .checkbox(value, RichText::new(label).size(12.6))
                    .changed();
            }
        });
        if changed {
            self.save_config();
        }
    }

    fn sidebar(&mut self, ui: &mut Ui, p: &Palette) {
        ui.spacing_mut().item_spacing.y = 16.0;
        if theme::primary_button(ui, "Load File").clicked() {
            self.pick_file();
        }
        Frame::new()
            .fill(p.info_bg)
            .corner_radius(6.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let name = self.config.file_name.as_deref().unwrap_or("No file loaded");
                // CSS: word-break: break-all, since a path has no spaces to wrap at.
                let mut job = LayoutJob::single_section(
                    name.to_owned(),
                    TextFormat::simple(egui::FontId::proportional(11.9), p.muted),
                );
                job.wrap.break_anywhere = true;
                ui.add(egui::Label::new(job));
            });
        self.filters(ui);
        let count = self.library.as_ref().map_or(0, TodoLibrary::item_count);
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.label(
                RichText::new(format!("Total items: {count}"))
                    .size(11.9)
                    .color(p.muted),
            );
        });
    }

    fn content(&self, ui: &mut Ui, p: &Palette) {
        if let Some(error) = &self.error {
            ui.label(RichText::new(error).color(p.due));
            ui.add_space(8.0);
        }
        Frame::new()
            .fill(p.panel)
            .stroke(Stroke::new(1.0, p.border))
            .corner_radius(6.0)
            .show(ui, |ui| {
                // The list box fills the content area, as in the web app.
                ui.set_min_size(ui.available_size());
                let Some(library) = &self.library else {
                    empty_state(
                        ui,
                        p,
                        "No file loaded. Use 'Load File' to select a todo.txt file.",
                    );
                    return;
                };
                let today = chrono::Local::now().date_naive();
                let visible = list::visible(&library.items, &self.config, today);
                if visible.is_empty() {
                    // Also when every item is filtered out, as in `rtmapp`.
                    empty_state(ui, p, "No items found.");
                } else {
                    list::show(ui, p, &library.items, &visible);
                }
            });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let p = theme::palette(ui);
        egui::Panel::top("header")
            .frame(Frame::NONE)
            .show(ui, |ui| header(ui, p));
        egui::Panel::left("sidebar")
            .exact_size(220.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(Frame::new().fill(p.panel).inner_margin(16.0))
            .show(ui, |ui| {
                self.sidebar(ui, p);
                // Right border, drawn on the outer edge of the panel's margin.
                let edge = ui.max_rect().expand(16.0);
                ui.painter().vline(
                    edge.right() - 0.5,
                    edge.y_range(),
                    Stroke::new(1.0, p.border),
                );
            });
        egui::CentralPanel::default()
            .frame(Frame::new().fill(p.bg).inner_margin(16.0))
            .show(ui, |ui| self.content(ui, p));
    }
}

/// Loads a todo.txt file. The path is canonicalized first, as `rtmapp` does; `path` is the
/// `PathBuf` from the file dialog, with no string or URI conversion in between.
fn load_library(path: &Path) -> Result<TodoLibrary, String> {
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut library = TodoLibrary::new(canonical.to_string_lossy().into_owned());
    library
        .load()
        .map_err(|e| format!("{}: {e}", canonical.display()))?;
    Ok(library)
}

fn header(ui: &mut Ui, p: &Palette) {
    Frame::new()
        .fill(p.header)
        .inner_margin(Margin::symmetric(20, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // CSS: h1 is 1.25rem with a 1.5 line height.
            ui.set_min_height(26.25);
            ui.label(
                RichText::new("Rusty Todo.txt Manager")
                    .font(theme::bold(17.5))
                    .color(Color32::WHITE),
            );
        });
}

fn empty_state(ui: &mut Ui, p: &Palette, text: &str) {
    ui.add_space(20.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(text).color(p.muted));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory whose name contains spaces, under the system temp dir.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rtmgui test {} {name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        dir
    }

    #[test]
    fn loads_items_from_a_path_with_spaces_and_dots() {
        let dir = temp_dir("loads");
        std::fs::write(
            dir.join("todo list.txt"),
            "(A) Pay bill +Home @online due:2026-11-01\nx 2026-10-20 2026-10-19 Done thing\nPlain item\n",
        )
        .unwrap();

        let library = load_library(&dir.join("sub").join("..").join("todo list.txt")).unwrap();

        assert_eq!(library.item_count(), 3);
        assert!(library.items[1].done);
        assert!(!library.file_name.contains(".."));
        assert!(library.file_name.ends_with("todo list.txt"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_file_is_an_error_that_names_the_path() {
        let dir = temp_dir("missing");
        let error = load_library(&dir.join("nope.txt")).unwrap_err();
        assert!(error.contains("nope.txt"), "{error}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn empty_file_gives_an_empty_library() {
        let dir = temp_dir("empty");
        std::fs::write(dir.join("todo.txt"), "").unwrap();
        assert_eq!(load_library(&dir.join("todo.txt")).unwrap().item_count(), 0);
        let _ = std::fs::remove_dir_all(dir);
    }
}
