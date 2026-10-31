use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, TryRecvError};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Color32, Frame, Layout, Margin, RichText, Stroke, Ui,
    text::{LayoutJob, TextFormat},
};
use todotxt::{TodoItem, TodoLibrary};

use crate::config::{self, AppConfig};
use crate::edit::{self, EditForm, Outcome};
use crate::list::{self, RowAction};
use crate::sync::{self, Done, Found, Question, Repo, Worker};
use crate::theme::{self, Palette};

/// How long exit waits for a job that changes not yet in the file depend on: a sync's pull and
/// push, 60 s each at most, and a little more.
const SAVE_TIMEOUT: Duration = Duration::from_secs(125);

/// What to do with the path a file dialog returned: `App::open_file` or `App::create_file`.
type OnPath = fn(&mut App, &Path);

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
    /// The question about the file's outside changes or untracked state, asked before it syncs.
    asking: Option<(Repo, Question)>,
    /// The edit dialog's item as it was when the dialog opened, and what deleting it would remove,
    /// so a pull that changed either is noticed.
    edited: Option<TodoItem>,
    deleting: Option<Vec<TodoItem>>,
    /// A pull changed the edit dialog's item, or what deleting it would remove; Save and Delete
    /// are refused.
    stale: bool,
    /// The open file dialog: its result, and what to do with a chosen path.
    dialog: Option<(mpsc::Receiver<Option<PathBuf>>, OnPath)>,
    ctx: egui::Context,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        App::with_config(cc, config::config_path())
    }

    /// `new` with the config at `config_path`: the UI tests start the app on a temp config, so
    /// they never read or write the user's.
    fn with_config(cc: &eframe::CreationContext<'_>, config_path: PathBuf) -> Self {
        theme::install(&cc.egui_ctx);
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
            asking: None,
            edited: None,
            deleting: None,
            stale: false,
            dialog: None,
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
        self.asking = None;
        let Some(library) = &self.library else {
            return;
        };
        match sync::find(Path::new(&library.file_name), &self.config) {
            Found::NoSync => {}
            Found::Repo(repo) => {
                self.worker = Some(Worker::start(repo, self.ctx.clone()));
                self.sync_now(Instant::now());
            }
            Found::Ask(repo, question) => self.asking = Some((repo, question)),
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

    /// The answer to `asking`: yes commits the outside changes or adds the file, then syncs; no
    /// closes a modified file, and opens an untracked one without sync (asked again next time).
    fn answer(&mut self, yes: bool) {
        let Some((repo, question)) = self.asking.take() else {
            return;
        };
        if yes {
            let message = repo.commit_message(question);
            let mut worker = Worker::start(repo, self.ctx.clone());
            worker.commit(message);
            self.worker = Some(worker);
            self.sync_now(Instant::now());
        } else if question == Question::Modified {
            let file = self.library.take().map(|library| library.file_name);
            self.editing = None;
            self.error = Some(format!(
                "{} has changes rtm did not commit; commit or revert them by hand, then load the \
                 file again.",
                file.unwrap_or_default()
            ));
        }
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

    fn sync_question(&mut self, ctx: &egui::Context, p: &Palette) {
        let Some((repo, question)) = &self.asking else {
            return;
        };
        let name = self
            .library
            .as_ref()
            .and_then(|library| Path::new(&library.file_name).file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let vcs = match repo.vcs() {
            rtmsync::Vcs::Git => "Git",
            rtmsync::Vcs::Pijul => "Pijul",
        };
        let (text, yes, no) = match question {
            Question::Modified => (
                format!("{name} has changes that rtm did not commit."),
                "Commit them",
                "Don't open",
            ),
            Question::Untracked => (
                format!("{name} is not in the {vcs} repository yet."),
                "Add it",
                "Not now",
            ),
        };
        let mut answer = None;
        egui::Modal::new(egui::Id::unique("sync-question")).show(ctx, |ui| {
            ui.label(RichText::new(text).color(p.text));
            ui.horizontal(|ui| {
                if theme::primary_button(ui, yes).clicked() {
                    answer = Some(true);
                }
                if theme::secondary_button(ui, no).clicked() {
                    answer = Some(false);
                }
            });
        });
        if let Some(yes) = answer {
            self.answer(yes);
        }
    }

    /// Asks for a file and loads it once the dialog returns (see `dialog_result`).
    fn pick_file(&mut self, frame: &eframe::Frame) {
        // Matches the disabled buttons; `dialog_result` is what waits for an idle worker before
        // the file is switched.
        if self.busy() || self.dialog.is_some() {
            return;
        }
        let dialog = rfd::AsyncFileDialog::new()
            .add_filter("Text Files", &["txt"])
            .set_parent(frame)
            .pick_file();
        self.show_dialog(dialog, App::open_file);
    }

    /// Asks where to create a todo.txt, then creates it empty and opens it. The dialog has asked
    /// before returning an existing file, so that file is replaced.
    fn new_file(&mut self, frame: &eframe::Frame) {
        if self.busy() || self.dialog.is_some() {
            return;
        }
        let dialog = rfd::AsyncFileDialog::new()
            .add_filter("Text Files", &["txt"])
            .set_file_name("todo.txt")
            .set_parent(frame)
            .save_file();
        self.show_dialog(dialog, App::create_file);
    }

    /// Waits for `dialog` on its own thread, so the window keeps repainting while it is open. The
    /// future is created on the UI thread, where macOS has to run the panel.
    fn show_dialog(
        &mut self,
        dialog: impl Future<Output = Option<rfd::FileHandle>> + Send + 'static,
        then: OnPath,
    ) {
        let (send, receive) = mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let path = pollster::block_on(dialog).map(|file| file.path().to_owned());
            let _ = send.send(path);
            ctx.request_repaint();
        });
        self.dialog = Some((receive, then));
    }

    /// Applies the file dialog's result once nothing depends on the open file: no job is running
    /// (its pending changes are written then) and no item is being edited. Until then the result
    /// waits in the channel; the worker's results and closing the edit dialog repaint.
    fn dialog_result(&mut self) {
        if self.busy() || self.editing.is_some() {
            return;
        }
        let Some((receive, then)) = &self.dialog else {
            return;
        };
        let then = *then;
        match receive.try_recv() {
            Err(TryRecvError::Empty) => {}
            // A cancel, a failure (rfd logs it) or a dialog thread that died.
            Ok(None) | Err(TryRecvError::Disconnected) => self.dialog = None,
            Ok(Some(path)) => {
                self.dialog = None;
                then(self, &path);
            }
        }
    }

    fn create_file(&mut self, path: &Path) {
        match std::fs::File::create(path) {
            Ok(_) => self.open_file(path),
            Err(e) => self.error = Some(format!("{}: {e}", path.display())),
        }
    }

    /// Makes `path` the active file, saves it in the config and starts its sync.
    fn open_file(&mut self, path: &Path) {
        match load_library(path) {
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

    fn sidebar(&mut self, ui: &mut Ui, p: &Palette, frame: &eframe::Frame) {
        ui.spacing_mut().item_spacing.y = 16.0;
        let busy = self.busy() || self.dialog.is_some();
        let (new, load) = ui
            .add_enabled_ui(!busy, |ui| {
                ui.horizontal(|ui| {
                    let new = theme::secondary_button(ui, "New File").clicked();
                    (new, theme::primary_button(ui, "Load File").clicked())
                })
                .inner
            })
            .inner;
        if new {
            self.new_file(frame);
        }
        if load {
            self.pick_file(frame);
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
                        "No file loaded. Use 'New File' to create a todo.txt file, or 'Load File' \
                         to select one.",
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
        self.dialog_result();
        self.tick(Instant::now());
    }

    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
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
                self.sidebar(ui, p, frame);
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
        self.sync_question(&ui.ctx().clone(), p);
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
#[path = "tests/app_support_tests.rs"]
mod support_tests;

#[cfg(test)]
#[path = "tests/app_file_tests.rs"]
mod file_tests;

#[cfg(test)]
#[path = "tests/app_edit_tests.rs"]
mod edit_tests;

#[cfg(test)]
#[path = "tests/app_sync_tests.rs"]
mod sync_tests;
