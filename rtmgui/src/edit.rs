//! The edit dialog: the form behind it (`EditForm`) and the modal (`show`), laid out like `rtmapp`'s
//! `#edit-dialog`. Saving follows `rtmapp`'s `update_item`, with deliberate differences: the priority
//! can be any letter A to Z, invalid dates or recurrence are refused instead of being silently
//! dropped, tag names with whitespace are refused instead of being written as typed, and a typed `+`
//! or `@` in front of a tag name is dropped. The date fields have a calendar popup (`calendar`).

use chrono::NaiveDate;
use eframe::egui::{self, Align, Color32, Frame, Key, Layout, Modal, RichText, Stroke, Ui};
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
}

pub enum Outcome {
    Save,
    Cancel,
}

impl EditForm {
    /// Fills the form from `item`, as `openEditDialog` does.
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

        item.description = description.to_owned();
        item.priority = TodoPriority {
            priority: self.priority,
        };
        item.due = due;
        item.threshold = threshold;
        item.recurrence = recurrence;
        item.projects = projects
            .into_iter()
            .map(|name| TodoProject { name })
            .collect();
        item.contexts = contexts
            .into_iter()
            .map(|name| TodoContext { name })
            .collect();
        Ok(())
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
    NaiveDate::parse_from_str(text, DATE_FORMAT)
        .map(Some)
        .map_err(|_| format!("{field}: expected YYYY-MM-DD"))
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

/// The priority choices: none, then A to Z, with the `rtmapp` labels on A to C.
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

/// The modal. Returns `Save` on the Save button or Enter in a field, and `Cancel` on the Cancel
/// button, Escape or a click outside the dialog (`rtmapp` closes only on its buttons).
pub fn show(ctx: &egui::Context, p: &Palette, form: &mut EditForm) -> Option<Outcome> {
    // CSS: 500 px wide with 24 px padding, 8 px corners and a soft shadow; dark adds a border.
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
    // CSS: 16 px between the groups, and between the two columns of a `.form-row`.
    ui.spacing_mut().item_spacing = egui::vec2(16.0, 16.0);
    ui.label(
        RichText::new("Edit Item")
            .font(theme::bold(17.5))
            .color(p.text),
    );
    ui.add_space(4.0);
    submit |= field(ui, p, "Description", &mut form.description, "");
    ui.columns(2, |c| {
        group(&mut c[0], "Priority", |ui| {
            priority_combo(ui, p, &mut form.priority)
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
    });
    if submit {
        outcome = Some(Outcome::Save);
    }
    outcome
}

/// A label with its control 6 px below. Labels are regular: the CSS weight 500 renders regular
/// under `font-synthesis: none` (see the spike's findings).
fn group<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.label(RichText::new(label).size(12.6));
        add(ui)
    })
    .inner
}

/// A labelled text field. Returns whether Enter was pressed in it (the form submits on Enter).
fn field(ui: &mut Ui, p: &Palette, label: &str, text: &mut String, hint: &str) -> bool {
    group(ui, label, |ui| {
        // 9 px rather than the CSS 8 px, so the fields are as tall as the priority select.
        let response = theme::text_input(ui, p, text, hint, 9, p.focus);
        response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))
    })
}

/// A labelled date field with the calendar popup. Returns whether Enter was pressed in it.
fn date_field(ui: &mut Ui, p: &Palette, label: &str, text: &mut String, hint: &str) -> bool {
    group(ui, label, |ui| {
        let response = calendar::date_field(ui, p, text, hint);
        response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))
    })
}

/// A `ComboBox` with the look of the text inputs, as the spike's `form_control` does.
fn priority_combo(ui: &mut Ui, p: &Palette, priority: &mut Option<u8>) {
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
            });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(line: &str) -> TodoItem {
        line.parse().unwrap()
    }

    fn form(line: &str) -> EditForm {
        EditForm::new(0, &item(line))
    }

    const FULL: &str =
        "x 2026-10-25 2026-10-20 Pay bill pri:D +Home @online due:2026-11-01 rec:+2w t:2026-10-30";

    #[test]
    fn saving_an_unchanged_form_keeps_the_item() {
        let mut target = item(FULL);
        form(FULL).apply(&mut target).unwrap();
        assert_eq!(target, item(FULL));
        assert_eq!(target.priority.priority, Some(3));
    }

    #[test]
    fn fields_are_filled_like_rtmapp() {
        let f = form("(B) Task +Work +Home @office due:2026-11-01 rec:1w t:2026-10-30");
        assert_eq!(f.description, "Task");
        assert_eq!(f.priority, Some(1));
        assert_eq!(f.due, "2026-11-01");
        assert_eq!(f.threshold, "2026-10-30");
        assert_eq!(f.recurrence, "w");
        assert_eq!(f.projects, "Work, Home");
        assert_eq!(f.contexts, "office");
    }

    #[test]
    fn every_field_is_written_and_the_rest_kept() {
        let mut target = item(FULL);
        let mut f = form(FULL);
        f.description = "  New text ".into();
        f.priority = Some(0);
        f.due = "2026-12-24".into();
        f.threshold = String::new();
        f.recurrence = " 1m ".into();
        f.projects = "+Work, , Garden ".into();
        f.contexts = "@home,phone".into();

        f.apply(&mut target).unwrap();

        assert_eq!(target.description, "New text");
        assert_eq!(target.priority.priority, Some(0));
        assert_eq!(target.due, NaiveDate::from_ymd_opt(2026, 12, 24));
        assert_eq!(target.threshold, None);
        assert_eq!(target.recurrence, "1m".parse().ok());
        let projects: Vec<_> = target.projects.iter().map(|p| p.name.as_str()).collect();
        let contexts: Vec<_> = target.contexts.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(projects, ["Work", "Garden"]);
        assert_eq!(contexts, ["home", "phone"]);
        // Not in the dialog: kept.
        let original = item(FULL);
        assert!(target.done);
        assert_eq!(target.completion_date, original.completion_date);
        assert_eq!(target.creation_date, original.creation_date);
    }

    #[test]
    fn empty_fields_clear() {
        let mut target = item(FULL);
        let mut f = form(FULL);
        f.priority = None;
        (f.due, f.threshold, f.recurrence) = (String::new(), " ".into(), String::new());
        (f.projects, f.contexts) = (String::new(), " , ".into());
        f.apply(&mut target).unwrap();
        assert_eq!(target.priority.priority, None);
        assert_eq!((target.due, target.threshold), (None, None));
        assert!(target.recurrence.is_none());
        assert!(target.projects.is_empty() && target.contexts.is_empty());
    }

    /// Each invalid field is refused with its own message and leaves the item untouched.
    #[test]
    fn invalid_input_is_refused() {
        let cases: [(fn(&mut EditForm), &str); 9] = [
            (|f| f.description = "  ".into(), "Description is required"),
            (
                |f| f.due = "2026-13-01".into(),
                "Due date: expected YYYY-MM-DD",
            ),
            (
                |f| f.threshold = "soon".into(),
                "Threshold: expected YYYY-MM-DD",
            ),
            (|f| f.recurrence = "weekly".into(), "Recurrence:"),
            (|f| f.recurrence = "+".into(), "Recurrence:"),
            (|f| f.recurrence = "1é".into(), "Recurrence:"),
            (|f| f.recurrence = "é".into(), "Recurrence:"),
            (
                |f| f.projects = "Work, My Project".into(),
                "Projects: names cannot contain spaces",
            ),
            (
                |f| f.contexts = "@at home".into(),
                "Contexts: names cannot contain spaces",
            ),
        ];
        for (change, message) in cases {
            let mut target = item(FULL);
            let mut f = form(FULL);
            change(&mut f);
            let error = f.apply(&mut target).unwrap_err();
            assert!(error.starts_with(message), "{error}");
            assert_eq!(target, item(FULL), "{message}");
        }
    }

    #[test]
    fn priority_labels() {
        assert_eq!(priority_label(None), "None");
        assert_eq!(priority_label(Some(0)), "(A) High");
        assert_eq!(priority_label(Some(2)), "(C) Low");
        assert_eq!(priority_label(Some(3)), "(D)");
        assert_eq!(priority_label(Some(25)), "(Z)");
    }
}
