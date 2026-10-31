use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Color32, Frame, Layout, Margin, RichText, Stroke, Ui,
    text::{LayoutJob, TextFormat},
};
use todotxt::{TodoItem, TodoLibrary};

use crate::config::{self, AppConfig};
use crate::edit::{self, EditForm, Outcome};
use crate::list::{self, RowAction};
use crate::sync::{self, Done, Found, Worker};
use crate::theme::{self, Palette};

/// How long exit waits for a job that changes not yet in the file depend on: a sync's pull and
/// push, 60 s each at most, and a little more.
const SAVE_TIMEOUT: Duration = Duration::from_secs(125);

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
    /// Sync of the open file (RTM-64), when it syncs.
    worker: Option<Worker>,
    /// Changes applied to `library` but not yet written: the file is written only while the worker
    /// is idle (RTM-20), and these are redone on the file a pull brings.
    pending: Vec<Change>,
    /// The sync line in the header: a failed pull or push, or why sync is off.
    sync_status: Option<String>,
    /// When the last sync was asked for; the next one is due `sync_interval_minutes` later.
    last_sync: Instant,
    /// The VCSs to choose from when the file's directory belongs to several.
    choose_vcs: Option<Vec<rtmsync::Vcs>>,
    /// The edit dialog's item as it was when the dialog opened, and what deleting it would remove,
    /// so a pull that changed either is noticed.
    edited: Option<TodoItem>,
    deleting: Option<Vec<TodoItem>>,
    /// A pull changed the edit dialog's item, or what deleting it would remove; Save and Delete
    /// are refused.
    stale: bool,
    ctx: egui::Context,
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
        let mut app = App {
            config,
            config_path,
            library,
            error,
            new_item: String::new(),
            editing: None,
            worker: None,
            pending: Vec::new(),
            sync_status: None,
            last_sync: Instant::now(),
            choose_vcs: None,
            edited: None,
            deleting: None,
            stale: false,
            ctx: cc.egui_ctx.clone(),
        };
        app.connect();
        app
    }

    /// Starts sync for the loaded file, if it syncs, and syncs once. Stops it for a previous file.
    fn connect(&mut self) {
        self.worker = None;
        self.sync_status = None;
        self.choose_vcs = None;
        let Some(library) = &self.library else {
            return;
        };
        match sync::find(Path::new(&library.file_name), &self.config) {
            Found::NoSync => {}
            Found::Repo(repo) => {
                self.worker = Some(Worker::start(repo, self.ctx.clone()));
                self.sync_now(Instant::now());
            }
            Found::Ambiguous(found) => self.choose_vcs = Some(found),
            Found::Conflicted => self.conflicted(),
            Found::Unavailable(e) => self.sync_status = Some(format!("Not syncing: {e}")),
        }
    }

    /// Closes the file: its repository has conflicts, which a reload would read as items. Pending
    /// changes cannot be written into it.
    fn conflicted(&mut self) {
        let file = self.library.take().map(|library| library.file_name);
        self.worker = None;
        self.editing = None;
        let lost = std::mem::take(&mut self.pending).len();
        let lost = match lost {
            0 => String::new(),
            1 => " One change made during the sync was not saved.".to_owned(),
            n => format!(" {n} changes made during the sync were not saved."),
        };
        self.error = Some(format!(
            "The repository holding {} has conflicts or a stopped rebase or merge; resolve them by \
             hand, then load the file again.{lost}",
            file.unwrap_or_default()
        ));
    }

    fn busy(&self) -> bool {
        self.worker.as_ref().is_some_and(Worker::busy)
    }

    /// Applies `change` to the list at once and writes it when the worker is idle.
    fn apply(&mut self, change: Change) {
        let Some(library) = &mut self.library else {
            return;
        };
        if change.redo(library) {
            self.error = None;
            self.pending.push(change);
            self.flush();
        }
    }

    /// Writes the pending changes and queues their commit, unless a job is outstanding: it may be
    /// a pull about to change the file, and the changes are redone on what it brings.
    fn flush(&mut self) {
        if self.pending.is_empty() || self.busy() {
            return;
        }
        let Some(library) = &mut self.library else {
            return;
        };
        let changes = std::mem::take(&mut self.pending);
        // The Tauri app read the file again after a completion; kept.
        let reread = changes.iter().any(Change::completes);
        let saved = library
            .save()
            .and_then(|()| if reread { library.load() } else { Ok(()) });
        if let Err(e) = saved {
            self.error = Some(format!("{}: {e}", library.file_name));
            return;
        }
        if let Some(worker) = &mut self.worker {
            worker.commit(commit_message(&changes));
        }
    }

    fn sync_now(&mut self, now: Instant) {
        if let Some(worker) = &mut self.worker {
            worker.sync();
            self.last_sync = now;
        }
    }

    /// Syncs when the interval has passed since the last sync, and asks for a repaint when the
    /// next one is due, so the timer runs in a hidden or minimized window too.
    fn tick(&mut self, now: Instant) {
        let minutes = self.config.sync_interval_minutes;
        if minutes == 0 || !self.worker.as_ref().is_some_and(Worker::has_remote) {
            return;
        }
        let interval = Duration::from_secs(minutes.saturating_mul(60));
        let wait = match self.last_sync.checked_add(interval) {
            None => return,
            Some(due) if now < due => due - now,
            Some(_) => {
                self.sync_now(now);
                interval
            }
        };
        self.ctx.request_repaint_after(wait);
    }

    /// Applies the worker's finished jobs, then writes the changes that waited for them.
    fn sync_results(&mut self) {
        let Some(worker) = &mut self.worker else {
            return;
        };
        for done in worker.poll() {
            self.handle(done);
        }
        self.flush();
    }

    /// One job's result: status, a reload after a pull that changed the file, closing the file on
    /// a conflict.
    fn handle(&mut self, done: Done) {
        match done {
            Done::Committed => {}
            Done::Synced { changed } => {
                self.sync_status = None;
                if changed {
                    self.reload();
                }
            }
            Done::NotSynced { error, changed } => {
                self.sync_status = Some(format!("Not synced: {error}"));
                // A pull that got through before the push failed.
                if changed {
                    self.reload();
                }
            }
            Done::NotCommitted(e) => self.error = Some(format!("Not committed: {e}")),
            Done::Conflict if self.library.is_some() => self.conflicted(),
            Done::Conflict => {}
        }
    }

    /// Handles the outstanding jobs' results until none is left or `deadline` passes, writing the
    /// changes that waited for them. A job that changes are still waiting behind gets as long as a
    /// sync's network timeout, and so does a commit: those changes are not in the file yet.
    fn finish_jobs(&mut self, deadline: Instant) {
        while let Some(worker) = &mut self.worker
            && worker.busy()
        {
            let saving = !self.pending.is_empty() || !worker.syncing();
            let until = if saving {
                deadline.max(Instant::now() + SAVE_TIMEOUT)
            } else {
                deadline
            };
            let Some(done) = worker.next(until.saturating_duration_since(Instant::now())) else {
                return;
            };
            self.handle(done);
            self.flush();
        }
    }

    /// Reads the file again after a pull changed it, redoes the pending changes on it, and finds
    /// the edit dialog's item in the result.
    fn reload(&mut self) {
        let Some(library) = &mut self.library else {
            return;
        };
        if let Err(e) = library.load() {
            self.error = Some(format!("{}: {e}", library.file_name));
            return;
        }
        let mut dropped = Vec::new();
        self.pending.retain(|change| {
            let redone = change.redo(library);
            if !redone {
                dropped.push(change.describe());
            }
            redone
        });
        if !dropped.is_empty() {
            self.error = Some(format!(
                "A sync changed or removed the item of a change made meanwhile, which was not \
                 applied: {}",
                dropped.join("; ")
            ));
        }
        let (Some(form), Some(original)) = (&mut self.editing, &self.edited) else {
            return;
        };
        let Some(index) = library.items.iter().position(|item| item == original) else {
            self.stale = true;
            return;
        };
        form.index = index;
        if form.confirm_delete.is_some()
            && let Some(removing) = &self.deleting
            && library.clone().remove_item(index).as_ref() != Some(removing)
        {
            self.stale = true;
        }
    }

    /// Opens the edit dialog on the item at `index`.
    fn open_editor(&mut self, index: usize) {
        let Some(item) = self.library.as_ref().and_then(|l| l.items.get(index)) else {
            return;
        };
        self.editing = Some(EditForm::new(index, item));
        self.edited = Some(item.clone());
        self.deleting = None;
        self.stale = false;
    }

    /// Refuses Save or Delete when a pull changed the dialog's item; true when refused.
    fn refuse_stale(&mut self) -> bool {
        if self.stale
            && let Some(form) = &mut self.editing
        {
            form.error = Some(
                "A sync changed this item, or what deleting it would remove; close the dialog and \
                 open the item again."
                    .to_owned(),
            );
        }
        self.stale
    }

    /// The Git-or-Pijul question, while it is open. The answer is saved in the config.
    fn vcs_question(&mut self, ctx: &egui::Context, p: &Palette) {
        let Some(found) = &self.choose_vcs else {
            return;
        };
        let mut answer = None;
        egui::Modal::new(egui::Id::unique("choose-vcs")).show(ctx, |ui| {
            ui.label(
                RichText::new("This folder holds both a Git and a Pijul repository.").color(p.text),
            );
            ui.label(RichText::new("Sync the file through:").color(p.text));
            ui.horizontal(|ui| {
                for vcs in found {
                    let name = match vcs {
                        rtmsync::Vcs::Git => "Git",
                        rtmsync::Vcs::Pijul => "Pijul",
                    };
                    if theme::primary_button(ui, name).clicked() {
                        answer = Some(name.to_ascii_lowercase());
                    }
                }
                if theme::secondary_button(ui, "Don't sync").clicked() {
                    answer = Some(String::new());
                }
            });
        });
        match answer.as_deref() {
            None => {}
            Some("") => self.choose_vcs = None,
            Some(name) => {
                self.config.vcs = Some(name.to_owned());
                self.save_config();
                self.connect();
            }
        }
    }

    /// Asks for a file and loads it. The dialog blocks the UI thread while it is open.
    fn pick_file(&mut self) {
        // A sync may be writing the file; the widgets are disabled too, but this is the guarantee.
        if self.busy() {
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Text Files", &["txt"])
            .pick_file()
        else {
            return;
        };
        match load_library(&path) {
            Ok(library) => {
                // The saved Git-or-Pijul choice belonged to the previous file.
                if self.config.file_name.as_deref() != Some(library.file_name.as_str()) {
                    self.config.vcs = None;
                }
                self.config.file_name = Some(library.file_name.clone());
                self.library = Some(library);
                self.save_config();
                self.connect();
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
        let busy = self.busy();
        if ui
            .add_enabled_ui(!busy, |ui| theme::primary_button(ui, "Load File"))
            .inner
            .clicked()
        {
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

    /// Completes or uncompletes the item at file index `index`, as the Tauri app did.
    fn toggle(&mut self, index: usize) {
        let item = self.library.as_ref().and_then(|l| l.items.get(index));
        if let Some(item) = item.cloned() {
            self.apply(Change::Toggle(item));
        }
    }

    /// Parses the quick-add text and appends the item, as the Tauri app did. The input is cleared
    /// only when the item was added; blank input does nothing.
    fn add_item(&mut self) {
        let text = self.new_item.trim();
        if text.is_empty() {
            return;
        }
        if self.library.is_none() {
            self.error = Some("Please load a file first".to_owned());
            return;
        }
        match text.parse::<TodoItem>() {
            Ok(item) => {
                self.apply(Change::Add(item));
                self.new_item.clear();
            }
            Err(e) => self.error = Some(format!("Could not parse the item: {e}")),
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

    /// Applies the edit dialog to its item, as the Tauri app's `update_item` did. An invalid field
    /// keeps the dialog open with the reason; nothing changes.
    fn save_edit(&mut self) {
        if self.refuse_stale() {
            return;
        }
        let (Some(form), Some(library)) = (&mut self.editing, &self.library) else {
            self.editing = None;
            return;
        };
        let Some(original) = library.items.get(form.index).cloned() else {
            self.editing = None;
            return;
        };
        let mut edited = original.clone();
        if let Err(e) = form.apply(&mut edited) {
            form.error = Some(e);
            return;
        }
        self.editing = None;
        self.edited = None;
        self.apply(Change::Edit { original, edited });
    }

    /// Shows the delete confirmation with the number of sub-items the delete would take, counted by
    /// removing from a copy so it is exactly what `delete_item` will remove.
    fn ask_delete(&mut self) {
        let (Some(form), Some(library)) = (&mut self.editing, &self.library) else {
            self.editing = None;
            return;
        };
        match library.clone().remove_item(form.index) {
            Some(removed) => {
                form.confirm_delete = Some(removed.len() - 1);
                self.deleting = Some(removed);
            }
            None => self.editing = None,
        }
    }

    /// Deletes the edited item and its sub-items, like the other mutations.
    fn delete_item(&mut self) {
        if self.refuse_stale() {
            return;
        }
        let (Some(form), Some(library)) = (self.editing.take(), &self.library) else {
            return;
        };
        self.edited = None;
        let Some(original) = library.items.get(form.index).cloned() else {
            return;
        };
        if let Some(removing) = library.clone().remove_item(form.index) {
            self.apply(Change::Delete { original, removing });
        }
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
    /// Writes and commits the pending changes and syncs once more, within a few seconds.
    fn on_exit(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        self.finish_jobs(deadline);
        if let Some(worker) = &mut self.worker {
            worker.sync();
        }
        self.finish_jobs(deadline);
    }

    /// Runs before each `ui`, and also while the window is hidden (when a repaint was requested).
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sync_results();
        self.tick(Instant::now());
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let p = theme::palette(ui);
        let syncing = self.worker.as_ref().is_some_and(Worker::syncing);
        let status = if syncing {
            Some("Syncing…".to_owned())
        } else {
            self.sync_status.clone()
        };
        // Sync now: enabled unless a sync is already running.
        let sync_button = self
            .worker
            .as_ref()
            .filter(|worker| worker.has_remote())
            .map(|_| !syncing);
        let sync_clicked = egui::Panel::top("header")
            .frame(Frame::NONE)
            .show(ui, |ui| header(ui, p, status.as_deref(), sync_button))
            .inner;
        if sync_clicked {
            self.sync_now(Instant::now());
        }
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
            Some(RowAction::Edit(index)) => self.open_editor(index),
            None => {}
        }
        self.vcs_question(&ui.ctx().clone(), p);
        if let Some(form) = &mut self.editing {
            match edit::show(ui.ctx(), p, form) {
                Some(Outcome::Save) => self.save_edit(),
                Some(Outcome::Cancel) => self.editing = None,
                Some(Outcome::AskDelete) => self.ask_delete(),
                Some(Outcome::Delete) => self.delete_item(),
                None => {}
            }
        }
    }
}

/// A change to the list, kept until it is written, so it can be redone on the file a pull brought.
/// Items are found by equality, so a change whose item the pull changed or removed is not redone.
#[derive(Clone)]
enum Change {
    Add(TodoItem),
    /// Completes the item, or uncompletes it when it is done.
    Toggle(TodoItem),
    Edit {
        original: TodoItem,
        edited: TodoItem,
    },
    /// `removing`: the item and its sub-items, as the confirmation counted them.
    Delete {
        original: TodoItem,
        removing: Vec<TodoItem>,
    },
}

impl Change {
    /// Applies the change to `library`; false when its item is not there as it was.
    fn redo(&self, library: &mut TodoLibrary) -> bool {
        let find = |library: &TodoLibrary, item| library.items.iter().position(|i| i == item);
        match self {
            // Sets the creation date when the item has none.
            Change::Add(item) => library.add_item(item.clone()),
            Change::Toggle(item) => {
                let Some(index) = find(library, item) else {
                    return false;
                };
                if item.done {
                    library.uncomplete_item(index);
                } else {
                    // Appends the next occurrence of a recurring item.
                    library.complete_item(index);
                }
            }
            Change::Edit { original, edited } => {
                let Some(index) = find(library, original) else {
                    return false;
                };
                library.items[index] = edited.clone();
            }
            Change::Delete { original, removing } => {
                let Some(index) = find(library, original) else {
                    return false;
                };
                if library.clone().remove_item(index).as_ref() != Some(removing) {
                    return false;
                }
                library.remove_item(index);
            }
        }
        true
    }

    fn completes(&self) -> bool {
        matches!(self, Change::Toggle(item) if !item.done)
    }

    /// The commit message: what was done and the item's description, without dates or tags.
    fn describe(&self) -> String {
        match self {
            Change::Add(item) => format!("rtm: add {}", item.description),
            Change::Toggle(item) if item.done => format!("rtm: uncomplete {}", item.description),
            Change::Toggle(item) => format!("rtm: complete {}", item.description),
            Change::Edit { original, .. } => format!("rtm: edit {}", original.description),
            Change::Delete { original, .. } => format!("rtm: delete {}", original.description),
        }
    }
}

/// One change's own message, or a count with one line per change.
fn commit_message(changes: &[Change]) -> String {
    match changes {
        [change] => change.describe(),
        changes => {
            let lines: Vec<_> = changes.iter().map(Change::describe).collect();
            format!("rtm: {} changes\n\n{}", changes.len(), lines.join("\n"))
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

/// The title bar, with the sync status and, when `sync_button` is set, a Sync now button enabled
/// as it says. Returns whether the button was clicked.
fn header(ui: &mut Ui, p: &Palette, status: Option<&str>, sync_button: Option<bool>) -> bool {
    let mut clicked = false;
    Frame::new()
        .fill(p.header)
        .inner_margin(Margin::symmetric(20, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // CSS: h1 is 1.25rem with a 1.5 line height.
            ui.set_min_height(26.25);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Rusty Todo.txt Manager")
                        .font(theme::bold(17.5))
                        .color(Color32::WHITE),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(enabled) = sync_button {
                        let button = egui::Button::new(
                            RichText::new("Sync now").size(12.6).color(Color32::WHITE),
                        )
                        .frame(false);
                        clicked = ui.add_enabled(enabled, button).clicked();
                    }
                    if let Some(status) = status {
                        // git's errors run to several lines; the first fits, the rest on hover.
                        let first = status.lines().next().unwrap_or_default();
                        ui.label(RichText::new(first).size(12.6).color(Color32::WHITE))
                            .on_hover_text(status);
                    }
                });
            });
        });
    clicked
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
    fn a_commit_message_names_only_the_description() {
        let item: TodoItem = "(A) 2026-10-30 Pay bill +Home @online due:2026-11-01 rec:w"
            .parse()
            .unwrap();
        assert_eq!(Change::Toggle(item).describe(), "rtm: complete Pay bill");
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
        bare_app(dir, Some(load_library(&file).unwrap()))
    }

    /// An app on `library` with its config in `dir`, built directly: no window, no sync started.
    fn bare_app(dir: &Path, library: Option<TodoLibrary>) -> App {
        App {
            config: AppConfig::default(),
            config_path: dir.join("config.toml"),
            library,
            error: None,
            new_item: String::new(),
            editing: None,
            worker: None,
            pending: Vec::new(),
            sync_status: None,
            last_sync: Instant::now(),
            choose_vcs: None,
            edited: None,
            deleting: None,
            stale: false,
            ctx: egui::Context::default(),
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
        let mut app = bare_app(&dir, None);
        app.new_item = "Task".to_owned();

        app.add_item();

        assert_eq!(app.error.as_deref(), Some("Please load a file first"));
        assert_eq!(app.new_item, "Task");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1); // only "sub"
        let _ = std::fs::remove_dir_all(dir);
    }

    fn open_editor(app: &mut App, index: usize) -> &mut EditForm {
        app.open_editor(index);
        app.editing.as_mut().unwrap()
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

    /// A parent (index 2) with a sub-item before it and one after it, between two plain items.
    const LINKED: &str = "First\n\
        Child A sub:5a1f3c2e-8d4b-4e6f-9a7c-1b2d3e4f5a6b\n\
        Parent uuid:5a1f3c2e-8d4b-4e6f-9a7c-1b2d3e4f5a6b\n\
        Child B sub:5a1f3c2e-8d4b-4e6f-9a7c-1b2d3e4f5a6b\n\
        Last\n";

    #[test]
    fn asking_to_delete_counts_the_sub_items() {
        let dir = temp_dir("ask-delete");
        let mut app = app_with(&dir, LINKED);
        // load() drops a line whose uuid does not parse; make sure none was.
        assert_eq!(app.library.as_ref().unwrap().item_count(), 5);

        open_editor(&mut app, 2);
        app.ask_delete();
        assert_eq!(app.editing.as_ref().unwrap().confirm_delete, Some(2));

        open_editor(&mut app, 0);
        app.ask_delete();
        assert_eq!(app.editing.as_ref().unwrap().confirm_delete, Some(0));
        // Counting changes nothing.
        assert_eq!(app.library.as_ref().unwrap().item_count(), 5);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn deleting_removes_the_item_and_its_sub_items() {
        let dir = temp_dir("delete");
        let mut app = app_with(&dir, LINKED);
        assert_eq!(app.library.as_ref().unwrap().item_count(), 5);
        open_editor(&mut app, 2);

        app.delete_item();

        assert_eq!(app.error, None);
        assert!(app.editing.is_none());
        assert_eq!(file_lines(&dir), ["First", "Last"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn deleting_a_plain_item_removes_only_it() {
        let dir = temp_dir("delete-plain");
        let mut app = app_with(&dir, "First\nSecond\nThird\n");
        open_editor(&mut app, 1);

        app.delete_item();

        assert_eq!(app.error, None);
        assert!(app.editing.is_none());
        assert_eq!(file_lines(&dir), ["First", "Third"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    // Sync (RTM-64, RTM-20): scratch git remotes and clones on local paths, no network.

    fn git(dir: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    /// Local config that keeps the user's global identity, signing, hooks and rerere out.
    fn isolate(repo: &Path, scratch: &Path) {
        let hooks = scratch.join("no hooks");
        git(repo, &["config", "user.name", "rtm test"]);
        git(repo, &["config", "user.email", "rtm@example.com"]);
        git(repo, &["config", "commit.gpgsign", "false"]);
        git(repo, &["config", "core.hooksPath", hooks.to_str().unwrap()]);
        git(repo, &["config", "rerere.enabled", "false"]);
    }

    /// A bare `remote` and clones `a` and `b` sharing a tracked `todo.txt` holding `lines`.
    fn remote_and_clones(name: &str, lines: &str) -> PathBuf {
        let dir = temp_dir(name);
        std::fs::create_dir_all(dir.join("no hooks")).unwrap();
        git(&dir, &["init", "--quiet", "--bare", "remote"]);
        isolate(&dir.join("remote"), &dir);
        let hooks = format!("core.hooksPath={}", dir.join("no hooks").to_str().unwrap());
        for clone in ["a", "b"] {
            git(&dir, &["clone", "--quiet", "-c", &hooks, "remote", clone]);
            isolate(&dir.join(clone), &dir);
            if clone == "a" {
                std::fs::write(dir.join("a/todo.txt"), lines).unwrap();
                git(&dir.join("a"), &["add", "todo.txt"]);
                git(&dir.join("a"), &["commit", "--quiet", "-m", "start"]);
                git(&dir.join("a"), &["push", "--quiet", "-u", "origin", "HEAD"]);
            }
        }
        git(&dir.join("b"), &["pull", "--quiet"]);
        dir
    }

    fn push_from_b(dir: &Path, lines: &str) {
        std::fs::write(dir.join("b/todo.txt"), lines).unwrap();
        git(&dir.join("b"), &["commit", "--quiet", "-am", "from b"]);
        git(&dir.join("b"), &["push", "--quiet"]);
    }

    /// Lets the worker finish its jobs and applies their results, as the frames would.
    fn wait(app: &mut App) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            app.sync_results();
            if !app.busy() {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the worker never finished"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// An app on `file`, connected as at startup, with its config in `dir`.
    fn synced_app(dir: &Path, file: &Path) -> App {
        let mut app = bare_app(dir, Some(load_library(file).unwrap()));
        app.connect();
        wait(&mut app);
        app
    }

    /// Queues a sync, as the timer or Sync now does.
    fn sync(app: &mut App) {
        app.sync_now(Instant::now());
    }

    fn subjects(repo: &Path, count: usize) -> String {
        git(repo, &["log", &format!("-{count}"), "--format=%s"])
    }

    fn descriptions(app: &App) -> Vec<String> {
        let library = app.library.as_ref().unwrap();
        library
            .items
            .iter()
            .map(|item| item.description.clone())
            .collect()
    }

    #[test]
    fn opening_syncs_and_a_change_is_committed_at_once_and_pushed_by_the_next_sync() {
        let dir = remote_and_clones("sync push", "Alpha\n");
        push_from_b(&dir, "Alpha\nFrom b\n");
        let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
        assert_eq!(descriptions(&app), ["Alpha", "From b"]);

        app.new_item = "Call mom".to_owned();
        app.add_item();
        // Saved in the same frame, committed by the worker, not pushed.
        assert!(
            file_lines(&dir.join("a"))
                .iter()
                .any(|l| l.ends_with("Call mom"))
        );
        wait(&mut app);
        assert_eq!(subjects(&dir.join("a"), 1), "rtm: add Call mom");
        assert_eq!(subjects(&dir.join("remote"), 1), "from b");

        sync(&mut app);
        wait(&mut app);
        assert_eq!(subjects(&dir.join("remote"), 1), "rtm: add Call mom");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_change_made_while_a_job_runs_shows_at_once_and_is_written_after_it() {
        let dir = remote_and_clones("sync busy", "Alpha\n");
        let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
        sync(&mut app);
        // Outstanding until its result is picked up, however fast the job itself is.
        app.toggle(0);
        app.new_item = "Call mom".to_owned();
        app.add_item();
        assert_eq!(descriptions(&app), ["Alpha", "Call mom"]);
        assert!(app.library.as_ref().unwrap().items[0].done);
        assert_eq!(
            std::fs::read_to_string(dir.join("a/todo.txt")).unwrap(),
            "Alpha\n"
        );
        wait(&mut app);
        let lines = file_lines(&dir.join("a"));
        assert!(
            lines[0].starts_with("x ") && lines[1].ends_with("Call mom"),
            "{lines:?}"
        );
        assert_eq!(
            git(&dir.join("a"), &["log", "-1", "--format=%B"]),
            "rtm: 2 changes\n\nrtm: complete Alpha\nrtm: add Call mom"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_pending_change_is_redone_on_what_the_pull_brought() {
        let dir = remote_and_clones("sync replay", "Alpha\nBravo\n");
        let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
        push_from_b(&dir, "First\nAlpha\nBravo\n");
        sync(&mut app);
        app.toggle(1);
        wait(&mut app);
        assert_eq!(descriptions(&app), ["First", "Alpha", "Bravo"]);
        let lines = file_lines(&dir.join("a"));
        assert!(
            lines[2].starts_with("x ") && lines[2].ends_with("Bravo"),
            "{lines:?}"
        );
        assert_eq!(subjects(&dir.join("a"), 1), "rtm: complete Bravo");
        assert_eq!(app.error, None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_pending_change_whose_item_the_pull_changed_is_dropped_with_a_note() {
        let dir = remote_and_clones("sync dropped", "Alpha\nBravo\n");
        let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
        push_from_b(&dir, "Alpha\nBravo, renamed\n");
        sync(&mut app);
        app.toggle(1);
        wait(&mut app);
        assert_eq!(descriptions(&app), ["Alpha", "Bravo, renamed"]);
        assert!(!app.library.as_ref().unwrap().items[1].done);
        assert!(
            app.error
                .as_deref()
                .unwrap()
                .contains("rtm: complete Bravo")
        );
        assert_eq!(subjects(&dir.join("a"), 1), "from b");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_pull_whose_push_fails_is_still_taken_in() {
        let dir = remote_and_clones("sync push fails", "Alpha\nBravo\nCharlie\n");
        let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
        let missing = dir.join("missing");
        git(
            &dir.join("a"),
            &[
                "remote",
                "set-url",
                "--push",
                "origin",
                missing.to_str().unwrap(),
            ],
        );
        push_from_b(&dir, "From b\nAlpha\nBravo\nCharlie\n");
        app.new_item = "Call mom".to_owned();
        app.add_item();
        wait(&mut app);
        sync(&mut app);
        wait(&mut app);
        assert!(
            app.sync_status
                .as_deref()
                .is_some_and(|s| s.starts_with("Not synced")),
            "{:?}",
            app.sync_status
        );
        assert_eq!(
            descriptions(&app),
            ["From b", "Alpha", "Bravo", "Charlie", "Call mom"]
        );

        app.new_item = "Again".to_owned();
        app.add_item();
        wait(&mut app);
        assert_eq!(file_lines(&dir.join("a"))[0], "From b");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_edit_is_refused_when_a_pull_changed_its_item() {
        let dir = remote_and_clones("sync stale", "Alpha\nBravo\n");
        let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
        open_editor(&mut app, 1).description = "Bravo, edited here".to_owned();

        push_from_b(&dir, "Alpha\nBravo, edited there\n");
        sync(&mut app);
        wait(&mut app);
        app.save_edit();

        let form = app.editing.as_ref().expect("the dialog stays open");
        assert!(form.error.as_deref().unwrap().contains("changed"));
        assert_eq!(descriptions(&app), ["Alpha", "Bravo, edited there"]);
        assert_eq!(subjects(&dir.join("a"), 1), "from b");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_edit_follows_its_item_when_a_pull_renumbers() {
        let dir = remote_and_clones("sync renumber", "Alpha\nBravo\n");
        let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
        open_editor(&mut app, 1).description = "Bravo, edited".to_owned();

        push_from_b(&dir, "First\nAlpha\nBravo\n");
        sync(&mut app);
        wait(&mut app);
        app.save_edit();
        wait(&mut app);

        assert!(app.editing.is_none());
        assert_eq!(descriptions(&app), ["First", "Alpha", "Bravo, edited"]);
        assert_eq!(subjects(&dir.join("a"), 1), "rtm: edit Bravo");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn exit_commits_and_pushes() {
        let dir = remote_and_clones("sync exit", "Alpha\n");
        let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
        app.new_item = "Call mom".to_owned();
        app.add_item();
        eframe::App::on_exit(&mut app);
        assert!(!app.busy());
        assert_eq!(subjects(&dir.join("remote"), 1), "rtm: add Call mom");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn exit_during_a_pull_redoes_pending_changes_on_it() {
        let dir = remote_and_clones("sync exit pull", "Alpha\n");
        let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
        push_from_b(&dir, "Alpha\nFrom b\n");
        sync(&mut app);
        app.new_item = "Call mom".to_owned();
        app.add_item();
        eframe::App::on_exit(&mut app);
        assert!(!app.busy());
        let lines = file_lines(&dir.join("a"));
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert_eq!(lines[1], "From b");
        assert_eq!(
            subjects(&dir.join("remote"), 2),
            "rtm: add Call mom\nfrom b"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_conflict_found_by_a_sync_closes_the_file() {
        let dir = remote_and_clones("sync job conflict", "Alpha\n");
        let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
        // b changes the line before the app has pulled; the app then changes it too.
        push_from_b(&dir, "Alpha from b\n");
        open_editor(&mut app, 0).description = "Alpha from a".to_owned();
        app.save_edit();
        wait(&mut app);
        sync(&mut app);
        app.new_item = "Lost".to_owned();
        app.add_item();
        wait(&mut app);

        assert!(app.library.is_none());
        let error = app.error.as_deref().unwrap();
        assert!(error.contains("resolve them by hand"), "{error}");
        assert!(error.contains("One change"), "{error}");
        assert!(app.worker.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_sync_is_due_once_the_interval_has_passed_and_never_with_0() {
        let dir = remote_and_clones("sync timer", "Alpha\n");
        let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
        let opened = app.last_sync;
        let minutes = |n: u64| Duration::from_secs(n * 60);

        app.tick(opened + minutes(9));
        assert!(!app.worker.as_ref().unwrap().syncing());
        app.tick(opened + minutes(10));
        assert!(app.worker.as_ref().unwrap().syncing());
        assert_eq!(app.last_sync, opened + minutes(10));
        wait(&mut app);

        app.config.sync_interval_minutes = 0;
        app.tick(opened + minutes(60 * 24));
        assert!(!app.worker.as_ref().unwrap().syncing());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sync_off_neither_commits_nor_pulls() {
        let dir = remote_and_clones("sync off", "Alpha\n");
        let mut app = bare_app(&dir, Some(load_library(&dir.join("a/todo.txt")).unwrap()));
        app.config.sync = false;
        app.connect();
        assert!(app.worker.is_none());
        app.new_item = "Call mom".to_owned();
        app.add_item();
        assert_eq!(git(&dir.join("a"), &["log", "-1", "--format=%s"]), "start");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_untracked_file_does_not_sync() {
        let dir = remote_and_clones("sync untracked", "Alpha\n");
        std::fs::write(dir.join("a/other.txt"), "Other\n").unwrap();
        let app = synced_app(&dir, &dir.join("a/other.txt"));
        assert!(app.worker.is_none());
        assert!(app.library.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_conflicted_repository_is_not_opened() {
        let dir = remote_and_clones("sync conflict", "Alpha\n");
        push_from_b(&dir, "Alpha from b\n");
        std::fs::write(dir.join("a/todo.txt"), "Alpha from a\n").unwrap();
        git(&dir.join("a"), &["commit", "--quiet", "-am", "from a"]);
        let pull = std::process::Command::new("git")
            .current_dir(dir.join("a"))
            .args(["pull", "--quiet", "--rebase"])
            .output()
            .unwrap();
        assert!(!pull.status.success());

        let app = synced_app(&dir, &dir.join("a/todo.txt"));
        assert!(app.library.is_none());
        assert!(
            app.error
                .as_deref()
                .unwrap()
                .contains("resolve them by hand")
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
