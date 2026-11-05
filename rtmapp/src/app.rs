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
use crate::settings::{self, SettingsForm};
use crate::sync::{self, Done, Found, Question, Repo, Worker};
use crate::theme::{self, Palette};

/// How long exit waits for a job that changes not yet in the file depend on: a sync's pull and
/// push, 60 s each at most, and a little more.
const SAVE_TIMEOUT: Duration = Duration::from_secs(125);

/// What to do with the path a file dialog returned: `App::open_file` or `App::create_file`.
type OnPath = fn(&mut App, &Path);

/// The menu's shortcuts (RTM-134); `COMMAND` is Cmd on macOS and Ctrl elsewhere.
const NEW_FILE: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::N);
const LOAD_FILE: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::O);
const SETTINGS: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Comma);
const QUIT: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Q);

pub struct App {
    config: AppConfig,
    config_path: PathBuf,
    /// The open files (RTM-66); one until tabs come (RTM-127).
    files: Vec<OpenFile>,
    /// The index in `files` of the file shown.
    active: usize,
    /// The app line, above the file's: the config, a file that could not be created or opened,
    /// the quick-add input. Shown until the next successful change or config save.
    error: Option<String>,
    /// The quick-add input.
    new_item: String,
    /// The open edit dialog, if any.
    editing: Option<EditForm>,
    /// The open Settings dialog, if any.
    settings: Option<SettingsForm>,
    /// Settings changed `sync`: reconnect once the worker is idle (`reconnect_when_idle`).
    reconnect: bool,
    /// Sync of the open file (RTM-64), when it syncs.
    worker: Option<Worker>,
    /// The sync line in the header: a failed pull or push, or why sync is off.
    sync_status: Option<String>,
    /// When the last sync was asked for; the next one is due `sync_interval_minutes` later.
    last_sync: Instant,
    /// The open file dialog: its result, and what to do with a chosen path.
    dialog: Option<(mpsc::Receiver<Option<PathBuf>>, OnPath)>,
    ctx: egui::Context,
}

/// What belongs to one open todo.txt (RTM-125).
#[derive(Default)]
struct OpenFile {
    library: Option<TodoLibrary>,
    /// The file line, below the app's: what happened to this file (a failed load at startup, a
    /// conflict, outside changes, a failed write, commit or reload, changes a pull dropped). Shown until the next successful
    /// change or until a file is opened in its place.
    error: Option<String>,
    /// Changes applied to `library` but not yet written: the file is written only while the worker
    /// is idle (RTM-20), and these are redone on the file a pull brings.
    pending: Vec<Change>,
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
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        App::with_config(cc, config::config_path())
    }

    /// `new` with the config at `config_path`: the UI tests start the app on a temp config, so
    /// they never read or write the user's.
    fn with_config(cc: &eframe::CreationContext<'_>, config_path: PathBuf) -> Self {
        theme::install(&cc.egui_ctx);
        let mut config = AppConfig::load(&config_path);
        let mut error = None;
        let library = match config.file_name.clone().map(PathBuf::from) {
            Some(path) if path.exists() => match load_library(&path) {
                Ok(library) => {
                    // The file's entry is looked up by the resolved path from now on; written
                    // with the next save, not now.
                    config.rename_active(&library.file_name);
                    Some(library)
                }
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
            files: vec![OpenFile {
                library,
                error,
                ..OpenFile::default()
            }],
            active: 0,
            error: None,
            new_item: String::new(),
            editing: None,
            settings: None,
            reconnect: false,
            worker: None,
            sync_status: None,
            last_sync: Instant::now(),
            dialog: None,
            ctx: cc.egui_ctx.clone(),
        };
        app.connect();
        app
    }

    /// The file shown.
    fn file(&self) -> &OpenFile {
        &self.files[self.active]
    }

    fn file_mut(&mut self) -> &mut OpenFile {
        &mut self.files[self.active]
    }

    /// Starts sync for the loaded file, if it syncs, and syncs once. Stops it for a previous file.
    fn connect(&mut self) {
        self.worker = None;
        self.sync_status = None;
        let file = self.file_mut();
        file.choose_vcs = None;
        file.asking = None;
        let Some(library) = &self.file().library else {
            return;
        };
        match sync::find(Path::new(&library.file_name), &self.config) {
            Found::NoSync => {}
            Found::Repo(repo) => {
                self.worker = Some(Worker::start(repo, self.ctx.clone()));
                self.sync_now(Instant::now());
            }
            Found::Ask(repo, question) => self.file_mut().asking = Some((repo, question)),
            Found::Ambiguous(found) => self.file_mut().choose_vcs = Some(found),
            Found::Conflicted => self.conflicted(),
            Found::Unavailable(e) => self.sync_status = Some(format!("Not syncing: {e}")),
        }
    }

    /// Closes the file: its repository has conflicts, which a reload would read as items. Pending
    /// changes cannot be written into it.
    fn conflicted(&mut self) {
        let open = self.file_mut();
        let file = open.library.take().map(|library| library.file_name);
        let lost = std::mem::take(&mut open.pending).len();
        self.worker = None;
        self.editing = None;
        let lost = match lost {
            0 => String::new(),
            1 => " One change made during the sync was not saved.".to_owned(),
            n => format!(" {n} changes made during the sync were not saved."),
        };
        self.file_mut().error = Some(format!(
            "The repository holding {} has conflicts or a stopped rebase or merge; resolve them by \
             hand, then load the file again.{lost}",
            file.unwrap_or_default()
        ));
    }

    /// The answer to `asking`: yes commits the outside changes or adds the file, then syncs; no
    /// closes a modified file, and opens an untracked one without sync (asked again next time).
    fn answer(&mut self, yes: bool) {
        let Some((repo, question)) = self.file_mut().asking.take() else {
            return;
        };
        if yes {
            let message = repo.commit_message(question);
            let mut worker = Worker::start(repo, self.ctx.clone());
            worker.commit(message);
            self.worker = Some(worker);
            self.sync_now(Instant::now());
        } else if question == Question::Modified {
            let file = self
                .file_mut()
                .library
                .take()
                .map(|library| library.file_name);
            self.editing = None;
            self.file_mut().error = Some(format!(
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
        let file = &mut self.files[self.active];
        let Some(library) = &mut file.library else {
            return;
        };
        if change.redo(library) {
            file.error = None;
            file.pending.push(change);
            self.error = None;
            self.flush();
        }
    }

    /// Writes the pending changes and queues their commit, unless a job is outstanding: it may be
    /// a pull about to change the file, and the changes are redone on what it brings.
    fn flush(&mut self) {
        if self.file().pending.is_empty() || self.busy() {
            return;
        }
        let file = &mut self.files[self.active];
        let Some(library) = &mut file.library else {
            return;
        };
        let changes = std::mem::take(&mut file.pending);
        // The Tauri app read the file again after a completion; kept.
        let reread = changes.iter().any(Change::completes);
        let saved = library
            .save()
            .and_then(|()| if reread { library.load() } else { Ok(()) });
        if let Err(e) = saved {
            file.error = Some(format!("{}: {e}", library.file_name));
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
            Done::NotCommitted(e) => self.file_mut().error = Some(format!("Not committed: {e}")),
            Done::Conflict if self.file().library.is_some() => self.conflicted(),
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
            let saving = !self.files[self.active].pending.is_empty() || !worker.syncing();
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
        let file = &mut self.files[self.active];
        let Some(library) = &mut file.library else {
            return;
        };
        if let Err(e) = library.load() {
            file.error = Some(format!("{}: {e}", library.file_name));
            return;
        }
        let mut dropped = Vec::new();
        file.pending.retain(|change| {
            let redone = change.redo(library);
            if !redone {
                dropped.push(change.describe());
            }
            redone
        });
        if !dropped.is_empty() {
            file.error = Some(format!(
                "A sync changed or removed the item of a change made meanwhile, which was not \
                 applied: {}",
                dropped.join("; ")
            ));
        }
        let (Some(form), Some(original)) = (&mut self.editing, &file.edited) else {
            return;
        };
        let Some(index) = library.items.iter().position(|item| item == original) else {
            file.stale = true;
            return;
        };
        form.index = index;
        if form.confirm_delete.is_some()
            && let Some(removing) = &file.deleting
            && library.clone().remove_item(index).as_ref() != Some(removing)
        {
            file.stale = true;
        }
    }

    /// Opens the edit dialog on the item at `index`.
    fn open_editor(&mut self, index: usize) {
        let file = &mut self.files[self.active];
        let Some(item) = file.library.as_ref().and_then(|l| l.items.get(index)) else {
            return;
        };
        self.editing = Some(EditForm::new(index, item));
        file.edited = Some(item.clone());
        file.deleting = None;
        file.stale = false;
    }

    /// Refuses Save or Delete when a pull changed the dialog's item; true when refused.
    fn refuse_stale(&mut self) -> bool {
        let stale = self.file().stale;
        if stale && let Some(form) = &mut self.editing {
            form.error = Some(
                "A sync changed this item, or what deleting it would remove; close the dialog and \
                 open the item again."
                    .to_owned(),
            );
        }
        stale
    }

    /// The Git-or-Pijul question, while it is open. The answer is saved in the config.
    fn vcs_question(&mut self, ctx: &egui::Context, p: &Palette) {
        let Some(found) = &self.file().choose_vcs else {
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
            Some("") => self.file_mut().choose_vcs = None,
            Some(name) => {
                if let Some(library) = &self.files[self.active].library {
                    self.config.set_vcs(&library.file_name, name);
                }
                self.save_config();
                self.connect();
            }
        }
    }

    fn sync_question(&mut self, ctx: &egui::Context, p: &Palette) {
        let Some((repo, question)) = &self.file().asking else {
            return;
        };
        let name = self
            .file()
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
                self.config.set_active(&library.file_name);
                let file = self.file_mut();
                file.library = Some(library);
                file.error = None;
                self.save_config();
                self.connect();
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Writes the config; replaces the app line with the result.
    fn save_config(&mut self) {
        self.error = self
            .config
            .save(&self.config_path)
            .err()
            .map(|e| format!("Could not save the config: {e}"));
    }

    /// Saves the Settings dialog's fields. A new interval takes effect at once (`tick` reads it);
    /// a change of `sync` reconnects once the worker is idle.
    fn save_settings(&mut self) {
        let Some(form) = self.settings.take() else {
            return;
        };
        let sync = self.config.sync;
        form.apply(&mut self.config);
        self.save_config();
        if self.config.sync != sync {
            self.reconnect = true;
        }
    }

    /// Starts or stops sync after Settings changed `sync`, once no job is outstanding: a running
    /// job's result (a pull that changed the file, a conflict) must not be lost, and the pending
    /// changes have been written by then.
    fn reconnect_when_idle(&mut self) {
        if self.reconnect && !self.busy() {
            self.reconnect = false;
            self.connect();
        }
    }

    /// Whether a modal is open; the menu's shortcuts are inactive then.
    fn modal_open(&self) -> bool {
        let file = self.file();
        self.editing.is_some()
            || self.settings.is_some()
            || file.choose_vcs.is_some()
            || file.asking.is_some()
    }

    /// The menu entry the header's menu or a shortcut chose.
    fn menu_action(&mut self, action: MenuAction, ctx: &egui::Context, frame: &eframe::Frame) {
        match action {
            MenuAction::NewFile => self.new_file(frame),
            MenuAction::LoadFile => self.pick_file(frame),
            MenuAction::Settings => self.settings = Some(SettingsForm::new(&self.config)),
            // Through the normal close, so `on_exit` writes, commits and syncs.
            MenuAction::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    /// The menu entry whose shortcut was pressed, unless a modal is open.
    fn shortcut(&self, ctx: &egui::Context) -> Option<MenuAction> {
        if self.modal_open() {
            return None;
        }
        ctx.input_mut(|i| {
            [
                (NEW_FILE, MenuAction::NewFile),
                (LOAD_FILE, MenuAction::LoadFile),
                (SETTINGS, MenuAction::Settings),
                (QUIT, MenuAction::Quit),
            ]
            .into_iter()
            .find(|(shortcut, _)| i.consume_shortcut(shortcut))
            .map(|(_, action)| action)
        })
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
        let count = self
            .file()
            .library
            .as_ref()
            .map_or(0, TodoLibrary::item_count);
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
        let item = self
            .file()
            .library
            .as_ref()
            .and_then(|l| l.items.get(index));
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
        if self.file().library.is_none() {
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
        let (Some(form), Some(library)) = (&mut self.editing, &self.files[self.active].library)
        else {
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
        self.file_mut().edited = None;
        self.apply(Change::Edit { original, edited });
    }

    /// Shows the delete confirmation with the number of sub-items the delete would take, counted by
    /// removing from a copy so it is exactly what `delete_item` will remove.
    fn ask_delete(&mut self) {
        let (Some(form), Some(library)) = (&mut self.editing, &self.files[self.active].library)
        else {
            self.editing = None;
            return;
        };
        match library.clone().remove_item(form.index) {
            Some(removed) => {
                form.confirm_delete = Some(removed.len() - 1);
                self.files[self.active].deleting = Some(removed);
            }
            None => self.editing = None,
        }
    }

    /// Deletes the edited item and its sub-items, like the other mutations.
    fn delete_item(&mut self) {
        if self.refuse_stale() {
            return;
        }
        let (Some(form), Some(library)) = (self.editing.take(), &self.file().library) else {
            return;
        };
        let original = library.items.get(form.index).cloned();
        let removing = library.clone().remove_item(form.index);
        self.file_mut().edited = None;
        if let (Some(original), Some(removing)) = (original, removing) {
            self.apply(Change::Delete { original, removing });
        }
    }

    /// The app and file lines, the add row and the list. Returns what was clicked in the list.
    fn content(&mut self, ui: &mut Ui, p: &Palette) -> Option<RowAction> {
        for error in [&self.error, &self.file().error].into_iter().flatten() {
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
                let Some(library) = &self.file().library else {
                    empty_state(
                        ui,
                        p,
                        "No file loaded. Use New File or Load File in the menu at the left of the \
                         title bar to create or select a todo.txt file.",
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
        self.reconnect_when_idle();
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
        // New File and Load File: disabled during any job and while a file dialog is open.
        let files_enabled = !self.busy() && self.dialog.is_none();
        let clicked = egui::Panel::top("header")
            .frame(Frame::NONE)
            .show(ui, |ui| {
                header(ui, p, status.as_deref(), sync_button, files_enabled)
            })
            .inner;
        let ctx = ui.ctx().clone();
        match clicked.or_else(|| self.shortcut(&ctx).map(HeaderAction::Menu)) {
            Some(HeaderAction::Sync) => self.sync_now(Instant::now()),
            Some(HeaderAction::Menu(action)) => self.menu_action(action, &ctx, frame),
            None => {}
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
        if let Some(form) = &mut self.settings {
            match settings::show(ui.ctx(), p, form) {
                Some(settings::Outcome::Save) => self.save_settings(),
                Some(settings::Outcome::Cancel) => self.settings = None,
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

/// What was clicked in the title bar.
enum HeaderAction {
    Sync,
    Menu(MenuAction),
}

/// The entries of the title bar's menu (RTM-134), also reached by their shortcuts.
#[derive(Clone, Copy)]
enum MenuAction {
    NewFile,
    LoadFile,
    Settings,
    Quit,
}

/// The title bar: the menu, the title, the sync status and, when `sync_button` is set, a Sync now
/// button enabled as it says. `files_enabled` enables the menu's New File and Load File.
fn header(
    ui: &mut Ui,
    p: &Palette,
    status: Option<&str>,
    sync_button: Option<bool>,
    files_enabled: bool,
) -> Option<HeaderAction> {
    let mut clicked = None;
    Frame::new()
        .fill(p.header)
        .inner_margin(Margin::symmetric(20, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // CSS: h1 is 1.25rem with a 1.5 line height.
            ui.set_min_height(26.25);
            ui.horizontal(|ui| {
                if let Some(action) = menu(ui, files_enabled) {
                    clicked = Some(HeaderAction::Menu(action));
                }
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
                        if ui.add_enabled(enabled, button).clicked() {
                            clicked = Some(HeaderAction::Sync);
                        }
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

/// The menu button and its popup: New File, Load File, Settings and Quit, with their shortcuts.
fn menu(ui: &mut Ui, files_enabled: bool) -> Option<MenuAction> {
    let button = theme::menu_button(ui);
    let mut chosen = None;
    egui::Popup::menu(&button).show(|ui| {
        let mut entry = |ui: &mut Ui, enabled: bool, text: &str, shortcut, action| {
            let button = egui::Button::new(text).shortcut_text(shortcut_text(ui.ctx(), shortcut));
            if ui.add_enabled(enabled, button).clicked() {
                chosen = Some(action);
            }
        };
        entry(
            ui,
            files_enabled,
            "New File…",
            &NEW_FILE,
            MenuAction::NewFile,
        );
        entry(
            ui,
            files_enabled,
            "Load File…",
            &LOAD_FILE,
            MenuAction::LoadFile,
        );
        ui.separator();
        entry(ui, true, "Settings…", &SETTINGS, MenuAction::Settings);
        ui.separator();
        entry(ui, true, "Quit", &QUIT, MenuAction::Quit);
    });
    chosen
}

/// `shortcut` as the menu shows it. egui spells out a key after modifier names ("Ctrl+Comma");
/// its symbol reads better ("Ctrl+,").
fn shortcut_text(ctx: &egui::Context, shortcut: &egui::KeyboardShortcut) -> String {
    let key = shortcut.logical_key;
    ctx.format_shortcut(shortcut)
        .replace(key.name(), key.symbol_or_name())
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

#[cfg(test)]
#[path = "tests/app_settings_tests.rs"]
mod settings_tests;

#[cfg(test)]
#[path = "tests/app_ui_tests.rs"]
mod ui_tests;

#[cfg(test)]
#[path = "tests/app_error_tests.rs"]
mod error_tests;
