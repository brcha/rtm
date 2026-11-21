//! The edit dialog. Invalid dates or recurrence and tag names with whitespace are refused, and a
//! typed `+` or `@` in front of a tag name is dropped. A description that the file would read back
//! as a different item is refused. Delete and Move to each ask inline first.

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
    pub description: String,
    pub priority: Option<u8>,
    pub due: String,
    pub threshold: String,
    pub recurrence: String,
    pub projects: String,
    pub contexts: String,
    /// Why the last Save was refused; shown above the buttons.
    pub error: Option<String>,
    /// While the delete confirmation is shown: how many sub-items go with the item.
    pub confirm_delete: Option<usize>,
    /// The other open files the item can be moved to; Move to is hidden without any.
    pub targets: Vec<MoveTarget>,
    /// While the move confirmation is shown: where to, and how many sub-items go with the item.
    pub confirm_move: Option<ConfirmMove>,
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
        let date = |d: Option<NaiveDate>| d.map(|d| d.format(DATE_FORMAT).to_string());
        EditForm {
            index,
            description: item.description.clone(),
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
            error: None,
            confirm_delete: None,
            targets: Vec::new(),
            confirm_move: None,
        }
    }

    /// Validates every field, then writes them to `item`. On an error `item` is left untouched.
    /// Fields the dialog does not show (`done`, the completion and creation dates, `uuid`, `sub`) are
    /// kept.
    pub fn apply(&self, item: &mut TodoItem) -> Result<(), String> {
        let description = self.description.trim();
        if description.is_empty() {
            return Err("Description is required".to_owned());
        }
        let due = date(&self.due, "Due date")?;
        let threshold = date(&self.threshold, "Threshold")?;
        let recurrence = recurrence(&self.recurrence)?;
        let projects = names(&self.projects, '+', "Projects")?;
        let contexts = names(&self.contexts, '@', "Contexts")?;

        let mut new = item.clone();
        // Single spaces, as the line reads back.
        new.description = description.split_whitespace().collect::<Vec<_>>().join(" ");
        new.priority = TodoPriority {
            priority: self.priority,
        };
        new.due = due;
        new.threshold = threshold;
        new.recurrence = recurrence;
        new.projects = projects
            .into_iter()
            .map(|name| TodoProject { name })
            .collect();
        new.contexts = contexts
            .into_iter()
            .map(|name| TodoContext { name })
            .collect();
        new.check_dates().map_err(|e| format!("Dates: {e}"))?;
        read_back(&new)?;
        *item = new;
        Ok(())
    }
}

/// Refuses an item whose line reads back as a different item: text typed in the description
/// that the file reads as a tag, a completion mark, a priority or a date.
fn read_back(new: &TodoItem) -> Result<(), String> {
    let back = new.to_string().parse::<TodoItem>().ok();
    if back.as_ref() == Some(new) {
        return Ok(());
    }
    let field = back.and_then(|back| {
        if back.projects != new.projects {
            Some("projects in the Projects field")
        } else if back.contexts != new.contexts {
            Some("contexts in the Contexts field")
        } else if back.due != new.due {
            Some("the due date in the Due Date field")
        } else if back.threshold != new.threshold {
            Some("the threshold in the Threshold field")
        } else if back.recurrence != new.recurrence {
            Some("the recurrence in the Recurrence field")
        } else {
            None
        }
    });
    Err(match field {
        Some(field) => format!("Description: put {field}"),
        None => "Description: reads back as a different item; remove the leading \"x\", \
                 priority or date, or the key:value tag"
            .to_owned(),
    })
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
        });
    // Escape or a click on the backdrop.
    if modal.should_close() {
        outcome = Some(Outcome::Cancel);
    }
    outcome
}

fn form_ui(ui: &mut Ui, p: &Palette, form: &mut EditForm) -> Option<Outcome> {
    let mut submit = false;
    ui.spacing_mut().item_spacing = egui::vec2(16.0, 16.0);
    ui.label(
        RichText::new("Edit Item")
            .font(theme::bold(17.5))
            .color(p.text),
    );
    ui.add_space(4.0);
    submit |= field(ui, p, "Description", &mut form.description, "");
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
        if theme::success_button(ui, "Save").clicked() {
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
