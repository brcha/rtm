//! The edit dialog: the item's todo.txt line, kept in sync with the fields below it in both
//! directions. Invalid dates or recurrence and tag names with whitespace are refused, and a typed
//! `+` or `@` in front of a tag name is dropped. A done item is shown read-only. Delete and Move to
//! each ask inline first.

use std::path::PathBuf;

use chrono::NaiveDate;
use eframe::egui::{self, Align, Color32, Key, Layout, Modal, RichText, Stroke, Ui};
use todotxt::todo_recurrence::TodoRecurrence;
use todotxt::{TodoContext, TodoItem, TodoPriority, TodoProject};

use crate::calendar;
use crate::theme::{self, Palette};

const DATE_FORMAT: &str = "%Y-%m-%d";

/// The fields of the dialog, as text, for the item at `index` in the file.
pub struct EditForm {
    pub index: usize,
    /// The item's line as in the file, less its `uuid:` and `sub:` tags, which link sub-items and
    /// are put back on Save.
    pub line: String,
    pub priority: Option<u8>,
    pub due: String,
    pub threshold: String,
    pub recurrence: String,
    pub projects: String,
    pub contexts: String,
    /// A done item: shown, but not edited (uncomplete it to edit it).
    pub read_only: bool,
    /// Why the last Save was refused; shown above the buttons.
    pub error: Option<String>,
    /// While the delete confirmation is shown: how many sub-items go with the item.
    pub confirm_delete: Option<usize>,
    /// The other open files the item can be moved to; Move to is hidden without any.
    pub targets: Vec<MoveTarget>,
    /// While the move confirmation is shown: where to, and how many sub-items go with the item.
    pub confirm_move: Option<ConfirmMove>,
    /// The line and the fields as of the last `sync`, to tell which of them changed since.
    synced: Snapshot,
}

/// The line and the fields as text.
#[derive(Clone, Default, PartialEq)]
struct Snapshot {
    line: String,
    priority: Option<u8>,
    due: String,
    threshold: String,
    recurrence: String,
    projects: String,
    contexts: String,
}

impl Snapshot {
    /// The fields of `item`, without the line.
    fn of(item: &TodoItem) -> Self {
        let date = |d: Option<NaiveDate>| d.map(|d| d.format(DATE_FORMAT).to_string());
        Snapshot {
            line: String::new(),
            priority: item.priority.priority,
            due: date(item.due).unwrap_or_default(),
            threshold: date(item.threshold).unwrap_or_default(),
            recurrence: item
                .recurrence
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
            projects: join(item.projects.iter().map(|p| p.name.as_str())),
            contexts: join(item.contexts.iter().map(|c| c.name.as_str())),
        }
    }
}

/// An open file the edit dialog's item can be moved to.
#[derive(Clone)]
pub struct MoveTarget {
    pub path: PathBuf,
    /// Its tab's label.
    pub name: String,
}

/// The move confirmation: the target, and how many sub-items go with the item.
pub struct ConfirmMove {
    pub target: MoveTarget,
    pub sub_items: usize,
}

pub enum Outcome {
    Save,
    Cancel,
    /// The trash button: ask before deleting.
    AskDelete,
    Delete,
    /// A file picked under Move to: ask before moving there.
    AskMove(MoveTarget),
    Move,
}

impl EditForm {
    pub fn new(index: usize, item: &TodoItem) -> Self {
        let shown = TodoItem {
            uuid: None,
            sub: None,
            ..item.clone()
        };
        let synced = Snapshot {
            line: shown.to_string(),
            ..Snapshot::of(item)
        };
        let s = synced.clone();
        EditForm {
            index,
            line: s.line,
            priority: s.priority,
            due: s.due,
            threshold: s.threshold,
            recurrence: s.recurrence,
            projects: s.projects,
            contexts: s.contexts,
            read_only: item.done,
            error: None,
            confirm_delete: None,
            targets: Vec::new(),
            confirm_move: None,
            synced,
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            line: self.line.clone(),
            priority: self.priority,
            due: self.due.clone(),
            threshold: self.threshold.clone(),
            recurrence: self.recurrence.clone(),
            projects: self.projects.clone(),
            contexts: self.contexts.clone(),
        }
    }

    /// Carries the changes made since the last call from the line to the fields and back. A field
    /// changed since is written into the line, unless it does not parse yet (`2026-1` while a date
    /// is typed), which leaves its part of the line alone. When the line was edited, the fields
    /// not changed since are filled from it. Run once a frame and before Save.
    pub fn sync(&mut self) {
        let was = self.synced.clone();
        if self.snapshot() == was {
            return;
        }
        let Ok(mut item) = self.line.parse::<TodoItem>() else {
            return;
        };
        let mut written = false;
        if self.priority != was.priority {
            item.priority = TodoPriority {
                priority: self.priority,
            };
            written = true;
        }
        if self.due != was.due
            && let Ok(due) = date(&self.due, "")
        {
            item.due = due;
            written = true;
        }
        if self.threshold != was.threshold
            && let Ok(threshold) = date(&self.threshold, "")
        {
            item.threshold = threshold;
            written = true;
        }
        if self.recurrence != was.recurrence
            && let Ok(recurrence) = recurrence(&self.recurrence)
        {
            item.recurrence = recurrence;
            written = true;
        }
        if self.projects != was.projects
            && let Ok(projects) = names(&self.projects, '+', "")
        {
            item.projects = projects
                .into_iter()
                .map(|name| TodoProject { name })
                .collect();
            written = true;
        }
        if self.contexts != was.contexts
            && let Ok(contexts) = names(&self.contexts, '@', "")
        {
            item.contexts = contexts
                .into_iter()
                .map(|name| TodoContext { name })
                .collect();
            written = true;
        }
        if self.line != was.line {
            let read = Snapshot::of(&item);
            if self.priority == was.priority {
                self.priority = read.priority;
            }
            if self.due == was.due {
                self.due = read.due;
            }
            if self.threshold == was.threshold {
                self.threshold = read.threshold;
            }
            if self.recurrence == was.recurrence {
                self.recurrence = read.recurrence;
            }
            if self.projects == was.projects {
                self.projects = read.projects;
            }
            if self.contexts == was.contexts {
                self.contexts = read.contexts;
            }
        }
        if written {
            self.line = item.to_string();
        }
        self.synced = self.snapshot();
    }

    /// Syncs, validates the line and the fields, then writes the line to `item`, with the item's
    /// `uuid:` and `sub:` tags put back. On an error `item` is left untouched; so is a done item,
    /// which the dialog does not edit, and an item the line leaves as it was.
    pub fn apply(&mut self, item: &mut TodoItem) -> Result<(), String> {
        if self.read_only {
            return Ok(());
        }
        self.sync();
        let line = self.line.trim();
        TodoItem::check_one_line(line).map_err(|e| format!("Line: {e}"))?;
        let mut new: TodoItem = line.parse().map_err(|e| format!("Line: {e}"))?;
        if new.description.is_empty() {
            return Err("Line: a description is required".to_owned());
        }
        if new.done {
            return Err(
                "Line: complete the item with its checkbox, not with a leading \"x\"".to_owned(),
            );
        }
        if new.uuid.is_some() || new.sub.is_some() {
            return Err("Line: uuid: and sub: tags link sub-items and cannot be typed".to_owned());
        }
        // The line holds every field that parses (`sync`); these are the ones that do not.
        date(&self.due, "Due date")?;
        date(&self.threshold, "Threshold")?;
        recurrence(&self.recurrence)?;
        names(&self.projects, '+', "Projects")?;
        names(&self.contexts, '@', "Contexts")?;
        new.uuid = item.uuid;
        new.sub = item.sub;
        new.check_dates().map_err(|e| format!("Dates: {e}"))?;
        read_back(&new)?;
        if new != *item {
            *item = new;
        }
        Ok(())
    }
}

/// Refuses an item whose line would read back as a different item.
fn read_back(new: &TodoItem) -> Result<(), String> {
    if new.to_string().parse::<TodoItem>().ok().as_ref() == Some(new) {
        Ok(())
    } else {
        Err("Line: reads back as a different item".to_owned())
    }
}

fn join<'a>(names: impl Iterator<Item = &'a str>) -> String {
    names.collect::<Vec<_>>().join(", ")
}

/// An empty field means no date.
fn date(text: &str, field: &str) -> Result<Option<NaiveDate>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    todotxt::strict_date(text)
        .map(Some)
        .ok_or_else(|| format!("{field}: expected YYYY-MM-DD"))
}

/// An empty field means no recurrence.
fn recurrence(text: &str) -> Result<Option<TodoRecurrence>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    text.parse()
        .map(Some)
        .map_err(|_| "Recurrence: expected e.g. 1d, +2w, 1m".to_owned())
}

/// Comma-separated names: trimmed, one leading `sigil` (`+` or `@`) dropped, empties skipped. A name
/// with whitespace would split into several words when the line is read back, so it is refused.
fn names(text: &str, sigil: char, field: &str) -> Result<Vec<String>, String> {
    text.split(',')
        .map(|name| {
            let name = name.trim();
            name.strip_prefix(sigil).unwrap_or(name)
        })
        .filter(|name| !name.is_empty())
        .map(|name| {
            if name.contains(char::is_whitespace) {
                Err(format!("{field}: names cannot contain spaces"))
            } else {
                Ok(name.to_owned())
            }
        })
        .collect()
}

/// The priority choices: none, then A to Z, with labels on A to C.
fn priority_label(priority: Option<u8>) -> String {
    match priority {
        None => "None".to_owned(),
        Some(n) => {
            let letter = char::from(b'A' + n.min(25));
            match n {
                0 => format!("({letter}) High"),
                1 => format!("({letter}) Medium"),
                2 => format!("({letter}) Low"),
                _ => format!("({letter})"),
            }
        }
    }
}

fn move_question(question: &ConfirmMove) -> String {
    let name = &question.target.name;
    match question.sub_items {
        0 => format!("Move this item to {name}?"),
        1 => format!("Move this item and 1 sub-item to {name}?"),
        n => format!("Move this item and {n} sub-items to {name}?"),
    }
}

fn delete_question(sub_items: usize) -> String {
    match sub_items {
        0 => "Delete this item?".to_owned(),
        1 => "Delete this item and 1 sub-item?".to_owned(),
        n => format!("Delete this item and {n} sub-items?"),
    }
}

/// The modal. Returns `Save` on the Save button or Enter in a field, `Cancel` on the Cancel
/// button, Escape or a click outside the dialog,
/// `AskDelete` on the trash button and `Delete` once the deletion is confirmed, `AskMove` on a file
/// picked under Move to and `Move` once the move is confirmed.
pub fn show(ctx: &egui::Context, p: &Palette, form: &mut EditForm) -> Option<Outcome> {
    let frame = theme::dialog_frame(p);
    let mut outcome = None;
    let modal = Modal::new(egui::Id::unique("edit-dialog"))
        .frame(frame)
        .backdrop_color(Color32::from_black_alpha(128))
        .show(ctx, |ui| {
            ui.set_width(452.0);
            // shortcut: no CSS max-height: 90vh scrolling. A ScrollArea inside the modal sizes itself
            // from the modal's previous size and stays too short; the dialog is about 560 px tall and
            // the window opens at 650 px. Revisit if the form grows or small windows matter.
            outcome = form_ui(ui, p, form);
            form.sync();
        });
    // Escape or a click on the backdrop.
    if modal.should_close() {
        outcome = Some(Outcome::Cancel);
    }
    outcome
}

fn form_ui(ui: &mut Ui, p: &Palette, form: &mut EditForm) -> Option<Outcome> {
    ui.spacing_mut().item_spacing = egui::vec2(16.0, 16.0);
    ui.label(
        RichText::new("Edit Item")
            .font(theme::bold(17.5))
            .color(p.text),
    );
    ui.add_space(4.0);
    let editable = !form.read_only;
    // A done item's fields are shown disabled; Enter in them does not save.
    let submit = ui
        .add_enabled_ui(editable, |ui| {
            let mut submit = false;
            submit |= field(ui, p, "Line", &mut form.line, "");
            ui.columns(2, |c| {
                group(&mut c[0], "Priority", |ui, label| {
                    priority_combo(ui, p, &mut form.priority, label)
                });
                submit |= date_field(&mut c[1], p, "Due Date", &mut form.due, "Select due date");
            });
            ui.columns(2, |c| {
                submit |= field(
                    &mut c[0],
                    p,
                    "Recurrence",
                    &mut form.recurrence,
                    "e.g., 1d, 1w, 1m",
                );
                submit |= date_field(
                    &mut c[1],
                    p,
                    "Threshold",
                    &mut form.threshold,
                    "Select threshold",
                );
            });
            submit |= field(
                ui,
                p,
                "Projects (comma separated)",
                &mut form.projects,
                "e.g., Work, Personal",
            );
            submit |= field(
                ui,
                p,
                "Contexts (comma separated)",
                &mut form.contexts,
                "e.g., home, office",
            );
            submit
        })
        .inner
        && editable;
    if let Some(error) = &form.error {
        ui.label(RichText::new(error).color(p.due));
    }
    if let Some(sub_items) = form.confirm_delete {
        ui.label(RichText::new(delete_question(sub_items)).color(p.text));
    }
    if let Some(question) = &form.confirm_move {
        ui.label(RichText::new(move_question(question)).color(p.text));
    }
    ui.add_space(4.0);
    let mut outcome = None;
    // Top-aligned: a centred cross axis would stretch the row over all the height below it.
    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if editable && theme::success_button(ui, "Save").clicked() {
            outcome = Some(Outcome::Save);
        }
        if theme::secondary_button(ui, "Cancel").clicked() {
            outcome = Some(Outcome::Cancel);
        }
        // The rest of the row, from the far left: the trash button and Move to, or the answers to
        // their questions.
        ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
            if form.confirm_move.is_some() {
                // As for Delete: the second click of a double-click must not confirm.
                let confirm = theme::primary_button(ui, "Move");
                if confirm.clicked() && !confirm.double_clicked() {
                    outcome = Some(Outcome::Move);
                }
                if theme::secondary_button(ui, "Keep").clicked() {
                    form.confirm_move = None;
                }
            } else if form.confirm_delete.is_some() {
                // The question line shifts Delete under where the trash button was, so the second
                // click of a double-click on the trash button must not confirm.
                let delete = theme::danger_button(ui, "Delete");
                if delete.clicked() && !delete.double_clicked() {
                    outcome = Some(Outcome::Delete);
                }
                if theme::secondary_button(ui, "Keep").clicked() {
                    form.confirm_delete = None;
                }
            } else {
                if theme::trash_button(ui, p).clicked() {
                    outcome = Some(Outcome::AskDelete);
                }
                if let Some(target) = move_to(ui, &form.targets) {
                    outcome = Some(Outcome::AskMove(target));
                }
            }
        });
    });
    if submit {
        outcome = Some(Outcome::Save);
    }
    outcome
}

/// The Move to button and its list of the other open files; nothing without any. Returns the file
/// picked.
fn move_to(ui: &mut Ui, targets: &[MoveTarget]) -> Option<MoveTarget> {
    if targets.is_empty() {
        return None;
    }
    let button = theme::secondary_button(ui, "Move to…");
    let mut picked = None;
    let shown = egui::Popup::menu(&button).show(|ui| {
        for target in targets {
            if ui.button(&target.name).clicked() {
                picked = Some(target.clone());
            }
        }
    });
    // A click in the dialog moves its layer to the top; as a sublayer the list stays above it.
    if let Some(shown) = &shown {
        ui.ctx()
            .set_sublayer(ui.layer_id(), shown.response.layer_id);
    }
    picked
}

/// A label with its control 6 px below; `add` gets the label's id, to name the control by it.
/// Labels are regular (see "Look" in `rtmapp/AGENTS.md`).
fn group<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui, egui::Id) -> R) -> R {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        let label = ui.label(RichText::new(label).size(12.6));
        add(ui, label.id)
    })
    .inner
}

/// A labelled text field. Returns whether Enter was pressed in it (the form submits on Enter).
fn field(ui: &mut Ui, p: &Palette, label: &str, text: &mut String, hint: &str) -> bool {
    group(ui, label, |ui, label| {
        // 9 px, so the fields are as tall as the priority select.
        let response = theme::text_input(ui, p, text, hint, 9, p.focus).labelled_by(label);
        response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))
    })
}

/// A labelled date field with the calendar popup. Returns whether Enter was pressed in it.
fn date_field(ui: &mut Ui, p: &Palette, label: &str, text: &mut String, hint: &str) -> bool {
    group(ui, label, |ui, label| {
        let response = calendar::date_field(ui, p, text, hint).labelled_by(label);
        response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))
    })
}

/// A `ComboBox` with the look of the text inputs, named by the label `label`.
fn priority_combo(ui: &mut Ui, p: &Palette, priority: &mut Option<u8>, label: egui::Id) {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(12.0, 9.0);
        let widgets = &mut ui.visuals_mut().widgets;
        for state in [
            &mut widgets.inactive,
            &mut widgets.hovered,
            &mut widgets.active,
            &mut widgets.open,
        ] {
            state.weak_bg_fill = p.input_bg;
            state.bg_fill = p.input_bg;
            state.bg_stroke = Stroke::new(1.0, p.input_border);
            state.corner_radius = 6.0.into();
            state.expansion = 0.0;
        }
        let width = ui.available_width();
        egui::ComboBox::from_id_salt("edit-priority")
            .width(width)
            .selected_text(RichText::new(priority_label(*priority)).size(13.3))
            .show_ui(ui, |ui| {
                for choice in std::iter::once(None).chain((0..26).map(Some)) {
                    ui.selectable_value(priority, choice, priority_label(choice));
                }
            })
            .response
            .labelled_by(label);
    });
}

#[cfg(test)]
#[path = "tests/edit_form_tests.rs"]
mod form_tests;
