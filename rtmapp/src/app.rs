use std::path::{Path, PathBuf};

use eframe::egui::{
    self, Align, Color32, Frame, Layout, Margin, RichText, Stroke, Ui,
    text::{LayoutJob, TextFormat},
};
use todotxt::{TodoItem, TodoLibrary};

use crate::config::{self, AppConfig};
use crate::edit::{self, EditForm, Outcome};
use crate::list::{self, RowAction};
use crate::theme::{self, Palette};

pub struct App {
    config: AppConfig,
    config_path: PathBuf,
    library: Option<TodoLibrary>,
    /// Shown above the list until the next successful action.
    error: Option<String>,
    /// The quick-add input.
    new_item: String,
    /// The open edit dialog, if any.
    editing: Option<EditForm>,
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
            new_item: String::new(),
            editing: None,
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

    /// The four view settings. A change is saved at once, as in the Tauri app.
    fn filters(&mut self, ui: &mut Ui, p: &Palette) {
        let mut changed = false;
        ui.scope(|ui| {
            // CSS: 16 px boxes, 8 px gaps, 0.9rem labels.
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.spacing_mut().icon_width = 16.0;
            ui.spacing_mut().icon_spacing = 8.0;
            // egui's light theme draws the box with no border, nearly invisible on white; give it
            // the grey outline of a native checkbox.
            let widgets = &mut ui.visuals_mut().widgets;
            for state in [&mut widgets.inactive, &mut widgets.hovered] {
                state.bg_fill = p.input_bg;
                state.bg_stroke = Stroke::new(1.0, p.done_text);
            }
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
        self.filters(ui, p);
        let count = self.library.as_ref().map_or(0, TodoLibrary::item_count);
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.label(
                RichText::new(format!("Total items: {count}"))
                    .size(11.9)
                    .color(p.muted),
            );
        });
    }

    /// Completes or uncompletes the item at file index `index` and saves at once, as the Tauri app
    /// did, including its re-read of the file after a successful completion.
    fn toggle(&mut self, index: usize) {
        let Some(library) = &mut self.library else {
            return;
        };
        let Some(item) = library.items.get(index) else {
            return;
        };
        let result = if item.done {
            library.uncomplete_item(index);
            library.save()
        } else {
            // Appends the next occurrence of a recurring item.
            library.complete_item(index);
            library.save().and_then(|()| library.load())
        };
        self.error = result.err().map(|e| format!("{}: {e}", library.file_name));
    }

    /// Parses the quick-add text, appends the item and saves at once, as the Tauri app did. The
    /// input is cleared only when the item was saved; blank input does nothing.
    fn add_item(&mut self) {
        let text = self.new_item.trim();
        if text.is_empty() {
            return;
        }
        let Some(library) = &mut self.library else {
            self.error = Some("Please load a file first".to_owned());
            return;
        };
        let item = match text.parse::<TodoItem>() {
            Ok(item) => item,
            Err(e) => {
                self.error = Some(format!("Could not parse the item: {e}"));
                return;
            }
        };
        // Sets the creation date.
        library.add_item(item);
        match library.save() {
            Ok(()) => {
                self.error = None;
                self.new_item.clear();
            }
            Err(e) => self.error = Some(format!("{}: {e}", library.file_name)),
        }
    }

    /// CSS `.add-item`: the input fills the width, "Add" sits to its right, 16 px above the list.
    fn add_row(&mut self, ui: &mut Ui, p: &Palette) {
        let mut submit = false;
        // Top-aligned: a centred cross axis would centre the row in all the space below it.
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            // Lines the button up with the input, as the CSS flex row does.
            ui.spacing_mut().interact_size.y = 38.0;
            submit |= theme::success_button(ui, "Add").clicked();
            // The CSS has no dark focus colour for this input, so it stays blue in both themes.
            let input = theme::text_input(
                ui,
                p,
                &mut self.new_item,
                "New item (e.g., (A) Task due:2024-01-15 +Project @context)",
                10,
                theme::BLUE,
            );
            // Enter ends the edit and drops focus; give it back for the next item.
            if input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submit = true;
                input.request_focus();
            }
        });
        ui.add_space(16.0 - ui.spacing().item_spacing.y);
        if submit {
            self.add_item();
        }
    }

    /// Applies the edit dialog to its item and saves at once, as the Tauri app's `update_item` did.
    /// An invalid field keeps the dialog open with the reason; nothing is written.
    fn save_edit(&mut self) {
        let (Some(form), Some(library)) = (&mut self.editing, &mut self.library) else {
            self.editing = None;
            return;
        };
        let Some(item) = library.items.get_mut(form.index) else {
            self.editing = None;
            return;
        };
        if let Err(e) = form.apply(item) {
            form.error = Some(e);
            return;
        }
        self.error = library
            .save()
            .err()
            .map(|e| format!("{}: {e}", library.file_name));
        self.editing = None;
    }

    /// The error line, the add row and the list. Returns what was clicked in the list.
    fn content(&mut self, ui: &mut Ui, p: &Palette) -> Option<RowAction> {
        if let Some(error) = &self.error {
            ui.label(RichText::new(error).color(p.due));
            ui.add_space(8.0);
        }
        self.add_row(ui, p);
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
                    return None;
                };
                let today = chrono::Local::now().date_naive();
                let visible = list::visible(&library.items, &self.config, today);
                if visible.is_empty() {
                    // Also when every item is filtered out, as in the Tauri app.
                    empty_state(ui, p, "No items found.");
                    None
                } else {
                    list::show(ui, p, &library.items, &visible)
                }
            })
            .inner
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
        let clicked = egui::CentralPanel::default()
            .frame(Frame::new().fill(p.bg).inner_margin(16.0))
            .show(ui, |ui| self.content(ui, p))
            .inner;
        match clicked {
            Some(RowAction::Toggle(index)) => self.toggle(index),
            Some(RowAction::Edit(index)) => {
                self.editing = self.library.as_ref().and_then(|library| {
                    library
                        .items
                        .get(index)
                        .map(|item| EditForm::new(index, item))
                });
            }
            None => {}
        }
        if let Some(form) = &mut self.editing {
            match edit::show(ui.ctx(), p, form) {
                Some(Outcome::Save) => self.save_edit(),
                Some(Outcome::Cancel) => self.editing = None,
                None => {}
            }
        }
    }
}

/// Loads a todo.txt file. The path is canonicalized first, as the Tauri app did; `path` is the
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
        let dir = std::env::temp_dir().join(format!("rtmapp test {} {name}", std::process::id()));
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

    /// An app on a temp todo.txt with `contents`. The config path is in the temp dir too, so a
    /// test can never write the user's real config.
    fn app_with(dir: &Path, contents: &str) -> App {
        let file = dir.join("todo.txt");
        std::fs::write(&file, contents).unwrap();
        App {
            config: AppConfig::default(),
            config_path: dir.join("config.toml"),
            library: Some(load_library(&file).unwrap()),
            error: None,
            new_item: String::new(),
            editing: None,
        }
    }

    fn file_lines(dir: &Path) -> Vec<String> {
        std::fs::read_to_string(dir.join("todo.txt"))
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn completing_a_recurring_item_saves_it_and_the_next_occurrence() {
        let dir = temp_dir("complete");
        // Strict recurrence, so the next due date follows the old one, not today.
        let mut app = app_with(&dir, "Pay rent due:2026-11-01 rec:+1w\n");

        app.toggle(0);

        assert_eq!(app.error, None);
        let lines = file_lines(&dir);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].starts_with("x ") && lines[0].contains("due:2026-11-01"));
        assert!(!lines[1].starts_with("x ") && lines[1].contains("due:2026-11-08"));
        // Re-read after completing, as in the Tauri app.
        assert_eq!(app.library.as_ref().unwrap().item_count(), 2);
        assert!(!dir.join("config.toml").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn uncompleting_clears_the_completion_date_on_disk() {
        let dir = temp_dir("uncomplete");
        let mut app = app_with(&dir, "x 2026-10-20 2026-10-19 Done thing\n");

        app.toggle(0);

        assert_eq!(app.error, None);
        let lines = file_lines(&dir);
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].starts_with("x ") && !lines[0].contains("2026-10-20"));
        assert!(lines[0].contains("2026-10-19 Done thing"), "{}", lines[0]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn toggling_an_index_past_the_end_changes_nothing() {
        let dir = temp_dir("past-end");
        let mut app = app_with(&dir, "Only item\n");

        app.toggle(1);

        assert_eq!(app.error, None);
        assert_eq!(file_lines(&dir), ["Only item"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn adding_saves_the_trimmed_item_with_a_creation_date() {
        let dir = temp_dir("add");
        let mut app = app_with(&dir, "First\n");
        app.new_item = "  (B) Second +Proj due:2026-11-02  ".to_owned();

        app.add_item();

        assert_eq!(app.error, None);
        assert_eq!(app.new_item, "");
        let today = chrono::Local::now().date_naive().format("%Y-%m-%d");
        assert_eq!(
            file_lines(&dir),
            [
                "First".to_owned(),
                format!("(B) {today} Second +Proj due:2026-11-02")
            ]
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn blank_input_adds_nothing() {
        let dir = temp_dir("blank");
        let mut app = app_with(&dir, "First\n");
        app.new_item = "   ".to_owned();

        app.add_item();

        assert_eq!(app.error, None);
        assert_eq!(
            std::fs::read_to_string(dir.join("todo.txt")).unwrap(),
            "First\n"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn adding_without_a_file_is_an_error_and_writes_nothing() {
        let dir = temp_dir("no-file");
        let mut app = App {
            config: AppConfig::default(),
            config_path: dir.join("config.toml"),
            library: None,
            error: None,
            new_item: "Task".to_owned(),
            editing: None,
        };

        app.add_item();

        assert_eq!(app.error.as_deref(), Some("Please load a file first"));
        assert_eq!(app.new_item, "Task");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1); // only "sub"
        let _ = std::fs::remove_dir_all(dir);
    }

    fn open_editor(app: &mut App, index: usize) -> &mut EditForm {
        let item = &app.library.as_ref().unwrap().items[index];
        app.editing.insert(EditForm::new(index, item))
    }

    #[test]
    fn saving_an_edit_writes_the_item_and_keeps_the_others() {
        let dir = temp_dir("edit");
        let mut app = app_with(&dir, "First\n(D) Second +Home\nThird\n");
        let form = open_editor(&mut app, 1);
        form.description = "Second, edited".into();
        form.due = "2026-11-02".into();

        app.save_edit();

        assert_eq!(app.error, None);
        assert!(app.editing.is_none());
        assert_eq!(
            file_lines(&dir),
            ["First", "(D) Second, edited +Home due:2026-11-02", "Third"]
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_invalid_edit_keeps_the_dialog_and_writes_nothing() {
        let dir = temp_dir("edit-invalid");
        let mut app = app_with(&dir, "First\n");
        open_editor(&mut app, 0).due = "tomorrow".into();

        app.save_edit();

        let form = app.editing.as_ref().expect("dialog stays open");
        assert_eq!(form.error.as_deref(), Some("Due date: expected YYYY-MM-DD"));
        assert_eq!(
            std::fs::read_to_string(dir.join("todo.txt")).unwrap(),
            "First\n"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
