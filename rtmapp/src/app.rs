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
use crate::sync::{self, Done, Found, Place, Question, Repo, RepoKey, Worker};
use crate::theme::{self, Palette};

/// How long exit waits for a job that changes not yet in the file depend on: a sync's pull and
/// push, 60 s each at most, and a little more.
const SAVE_TIMEOUT: Duration = Duration::from_secs(125);

/// What to do with the path a file dialog returned: `App::open_file` or `App::create_file`.
type OnPath = fn(&mut App, &Path);

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
    /// The repositories the open files sync through (RTM-64), one worker each (RTM-126).
    repos: Vec<RepoSync>,
    /// The open file dialog: its result, and what to do with a chosen path.
    dialog: Option<(mpsc::Receiver<Option<PathBuf>>, OnPath)>,
    ctx: egui::Context,
}

/// One repository's sync: the worker that runs all its git or pijul calls, for every open file
/// in it.
struct RepoSync {
    key: RepoKey,
    worker: Worker,
    /// The sync line in the header: a failed pull or push.
    status: Option<String>,
    /// When the last sync was asked for; the next one is due `sync_interval_minutes` later.
    last_sync: Instant,
}

/// What belongs to one open todo.txt (RTM-125).
#[derive(Default)]
struct OpenFile {
    library: Option<TodoLibrary>,
    /// The repository it syncs through, a key of `App::repos`.
    repo: Option<RepoKey>,
    /// Why it does not sync, for the header: a check that failed (for example `git` missing).
    not_syncing: Option<String>,
    /// Its check on open is running on the repository's worker; its changes wait for it.
    checking: bool,
    /// The file line, below the app's: what happened to this file (a failed load at startup, a
    /// conflict, outside changes, a failed write, commit or reload, changes a pull dropped). Shown
    /// until the next successful change or until a file is opened in its place.
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
            repos: Vec::new(),
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

    /// The repository `file` syncs through.
    fn repo_of(&self, file: &OpenFile) -> Option<&RepoSync> {
        let key = file.repo.as_ref()?;
        self.repos.iter().find(|repo| &repo.key == key)
    }

    /// Starts sync for the shown file: its check on open runs on the repository's worker, and the
    /// file syncs once that passes (`checked`). A file it replaced has left its repository first.
    fn connect(&mut self) {
        let Some(file) = self.files.get_mut(self.active) else {
            return;
        };
        file.connect(&mut self.repos, &self.config, &self.ctx);
        self.prune();
    }

    /// The result of the check on open of the open file at `path`.
    fn checked(&mut self, path: &Path, found: Found) {
        let Some(file) = self
            .files
            .iter_mut()
            .find(|file| file.checking && file.is(path))
        else {
            return;
        };
        file.checking = false;
        match found {
            Found::Repo(repo) => {
                // A sync queued before the file joined did not list it; read what it pulled.
                file.reload(self.editing.as_mut());
                if let Some(joined) = find_repo(&mut self.repos, file.repo.as_ref()) {
                    joined.worker.found_remote(repo.remote());
                }
                self.sync_repo(&repo.key(), Instant::now());
            }
            Found::Ask(repo, question) => {
                file.reload(self.editing.as_mut());
                file.asking = Some((repo, question));
            }
            Found::NoSync => file.repo = None,
            Found::Conflicted => file.close_conflicted(),
            Found::Unavailable(e) => {
                file.repo = None;
                file.not_syncing = Some(format!("Not syncing: {e}"));
            }
        }
        self.prune();
        if self.file().library.is_none() {
            // A conflicted repository closed the file.
            self.close_editor();
        }
    }

    /// Stops the repositories no open file syncs through any more (their workers end once their
    /// queues are done).
    fn prune(&mut self) {
        let files = &self.files;
        self.repos.retain(|repo| {
            files
                .iter()
                .any(|file| file.repo.as_ref() == Some(&repo.key))
        });
    }

    /// Queues a sync of the repository `key` for its open files.
    fn sync_repo(&mut self, key: &RepoKey, now: Instant) {
        if let Some(repo) = find_repo(&mut self.repos, Some(key)) {
            repo.sync(&self.files, now);
        }
    }

    /// The answer to `asking`: yes commits the outside changes or adds the file, then syncs; no
    /// closes a modified file, and opens an untracked one without sync (asked again next time).
    fn answer(&mut self, yes: bool) {
        let Some(file) = self.files.get_mut(self.active) else {
            return;
        };
        let Some((repo, question)) = file.asking.take() else {
            return;
        };
        if yes {
            let message = repo.commit_message(question);
            let path = file.path();
            if let Some(joined) = find_repo(&mut self.repos, file.repo.as_ref()) {
                joined.worker.found_remote(repo.remote());
                if let Some(path) = path {
                    joined.worker.commit(path, message);
                }
            }
            self.sync_repo(&repo.key(), Instant::now());
        } else if question == Question::Modified {
            file.close_modified();
            self.close_editor();
        } else {
            // Opened without sync, asked again next time.
            file.repo = None;
        }
        self.prune();
    }

    /// Whether any repository has a job outstanding: New File and Load File wait for it.
    fn busy(&self) -> bool {
        self.repos.iter().any(|repo| repo.worker.busy())
    }

    /// Applies `change` to the list at once and writes it when the worker is idle.
    fn apply(&mut self, change: Change) {
        let file = self.file_mut();
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

    /// Writes every open file's pending changes whose repository is idle (see `OpenFile::flush`).
    fn flush(&mut self) {
        for file in &mut self.files {
            file.flush(find_repo(&mut self.repos, file.repo.as_ref()));
        }
    }

    /// Syncs every repository, as Sync now and exit do.
    fn sync_now(&mut self, now: Instant) {
        for repo in &mut self.repos {
            repo.sync(&self.files, now);
        }
    }

    /// Syncs each repository when the interval has passed since its last sync, and asks for a
    /// repaint when the next one is due, so the timer runs in a hidden or minimized window too.
    fn tick(&mut self, now: Instant) {
        let minutes = self.config.sync_interval_minutes;
        if minutes == 0 {
            return;
        }
        let interval = Duration::from_secs(minutes.saturating_mul(60));
        let mut wait = None::<Duration>;
        for repo in self
            .repos
            .iter_mut()
            .filter(|repo| repo.worker.has_remote())
        {
            let next = match repo.last_sync.checked_add(interval) {
                None => continue,
                Some(due) if now < due => due - now,
                Some(_) => {
                    repo.sync(&self.files, now);
                    interval
                }
            };
            wait = Some(wait.map_or(next, |wait| wait.min(next)));
        }
        if let Some(wait) = wait {
            self.ctx.request_repaint_after(wait);
        }
    }

    /// Applies the workers' finished jobs, then writes the changes that waited for them.
    fn sync_results(&mut self) {
        let results: Vec<_> = self
            .repos
            .iter_mut()
            .flat_map(|repo| {
                let key = repo.key.clone();
                repo.worker
                    .poll()
                    .into_iter()
                    .map(move |done| (key.clone(), done))
            })
            .collect();
        for (key, done) in results {
            self.handle(&key, done);
        }
        self.flush();
    }

    /// One job's result for the repository `key`: status, a reload of each file a pull changed,
    /// closing the repository's files on a conflict.
    fn handle(&mut self, key: &RepoKey, done: Done) {
        let Some(repo) = find_repo(&mut self.repos, Some(key)) else {
            // The repository stopped (a conflict, or its last file left) after the job.
            return;
        };
        match done {
            Done::Checked { file, found } => self.checked(&file, found),
            Done::Committed => {}
            Done::Synced { changed } => {
                repo.status = None;
                self.reload(&changed);
            }
            Done::NotSynced { error, changed } => {
                repo.status = Some(format!("Not synced: {error}"));
                // A pull that got through before the push failed.
                self.reload(&changed);
            }
            Done::NotCommitted { file, error } => {
                if let Some(file) = self.files.iter_mut().find(|open| open.is(&file)) {
                    file.error = Some(format!("Not committed: {error}"));
                }
            }
            Done::Conflict => {
                for file in self
                    .files
                    .iter_mut()
                    .filter(|file| file.repo.as_ref() == Some(key))
                {
                    file.close_conflicted();
                }
                self.prune();
                if self.file().library.is_none() {
                    self.close_editor();
                }
            }
        }
    }

    /// Reads the open files at `changed` again after a pull changed them (see `OpenFile::reload`).
    fn reload(&mut self, changed: &[PathBuf]) {
        let changed = |file: &&mut OpenFile| changed.iter().any(|path| file.is(path));
        for file in self.files.iter_mut().filter(changed) {
            file.reload(self.editing.as_mut());
        }
    }

    /// Handles the outstanding jobs' results until none is left or `deadline` passes, writing the
    /// changes that waited for them. A job that changes are still waiting behind gets as long as a
    /// sync's network timeout, and so does a commit: those changes are not in the file yet. The
    /// repositories' workers run at once, so they are waited for in turn within one limit.
    fn finish_jobs(&mut self, deadline: Instant) {
        let saving_deadline = deadline.max(Instant::now() + SAVE_TIMEOUT);
        let keys: Vec<_> = self.repos.iter().map(|repo| repo.key.clone()).collect();
        for key in keys {
            while let Some(repo) = find_repo(&mut self.repos, Some(&key))
                && repo.worker.busy()
            {
                let waiting = self
                    .files
                    .iter()
                    .any(|file| file.repo.as_ref() == Some(&key) && !file.pending.is_empty());
                let until = if waiting || !repo.worker.syncing() {
                    saving_deadline
                } else {
                    deadline
                };
                let Some(done) = repo
                    .worker
                    .next(until.saturating_duration_since(Instant::now()))
                else {
                    break;
                };
                self.handle(&key, done);
                self.flush();
            }
        }
    }

    /// Closes the edit dialog and forgets its item, so a later reload of the file does not touch
    /// a dialog opened on another item.
    fn close_editor(&mut self) {
        self.editing = None;
        let file = self.file_mut();
        file.edited = None;
        file.deleting = None;
        file.stale = false;
    }

    /// Opens the edit dialog on the item at `index`.
    fn open_editor(&mut self, index: usize) {
        let Some(file) = self.files.get_mut(self.active) else {
            return;
        };
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
                let file = self.file().library.as_ref().map(|l| l.file_name.clone());
                if let Some(file) = file {
                    self.config.set_vcs(&file, name);
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
        let library = self
            .files
            .get(self.active)
            .and_then(|file| file.library.as_ref());
        let (Some(form), Some(library)) = (&mut self.editing, library) else {
            self.close_editor();
            return;
        };
        let Some(original) = library.items.get(form.index).cloned() else {
            self.close_editor();
            return;
        };
        let mut edited = original.clone();
        if let Err(e) = form.apply(&mut edited) {
            form.error = Some(e);
            return;
        }
        self.close_editor();
        self.apply(Change::Edit { original, edited });
    }

    /// Shows the delete confirmation with the number of sub-items the delete would take, counted by
    /// removing from a copy so it is exactly what `delete_item` will remove.
    fn ask_delete(&mut self) {
        let Some(file) = self.files.get_mut(self.active) else {
            return;
        };
        let (Some(form), Some(library)) = (&mut self.editing, &file.library) else {
            self.close_editor();
            return;
        };
        match library.clone().remove_item(form.index) {
            Some(removed) => {
                form.confirm_delete = Some(removed.len() - 1);
                file.deleting = Some(removed);
            }
            None => self.close_editor(),
        }
    }

    /// Deletes the edited item and its sub-items, like the other mutations.
    fn delete_item(&mut self) {
        if self.refuse_stale() {
            return;
        }
        let (Some(form), Some(library)) = (self.editing.take(), &self.file().library) else {
            self.close_editor();
            return;
        };
        let original = library.items.get(form.index).cloned();
        let removing = library.clone().remove_item(form.index);
        self.close_editor();
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

impl RepoSync {
    /// Queues a sync of the repository for those of `files` that sync through it.
    fn sync(&mut self, files: &[OpenFile], now: Instant) {
        let paths = files
            .iter()
            .filter(|file| file.repo.as_ref() == Some(&self.key))
            .filter_map(OpenFile::path)
            .collect();
        self.worker.sync(paths);
        self.last_sync = now;
    }
}

/// The repository `key` among `repos`.
fn find_repo<'a>(repos: &'a mut [RepoSync], key: Option<&RepoKey>) -> Option<&'a mut RepoSync> {
    let key = key?;
    repos.iter_mut().find(|repo| &repo.key == key)
}

/// The sync of repository `key`, started with a worker when no open file uses it yet.
fn ensure_repo<'a>(
    repos: &'a mut Vec<RepoSync>,
    key: &RepoKey,
    ctx: &egui::Context,
) -> Option<&'a mut RepoSync> {
    if !repos.iter().any(|known| &known.key == key) {
        repos.push(RepoSync {
            key: key.clone(),
            worker: Worker::start(key.vcs, ctx.clone()),
            status: None,
            last_sync: Instant::now(),
        });
    }
    find_repo(repos, Some(key))
}

impl OpenFile {
    /// The file's path, while it is loaded.
    fn path(&self) -> Option<PathBuf> {
        self.library
            .as_ref()
            .map(|library| PathBuf::from(&library.file_name))
    }

    /// Whether this is the loaded file at `path`.
    fn is(&self, path: &Path) -> bool {
        self.library
            .as_ref()
            .is_some_and(|library| Path::new(&library.file_name) == path)
    }

    /// Finds where the file syncs and, in a repository, assigns it there and queues its check on
    /// open on the repository's worker, so syncs list it from now on. The file has left any
    /// repository it was in.
    fn connect(&mut self, repos: &mut Vec<RepoSync>, config: &AppConfig, ctx: &egui::Context) {
        self.repo = None;
        self.not_syncing = None;
        self.choose_vcs = None;
        self.asking = None;
        self.checking = false;
        let Some(path) = self.path() else {
            return;
        };
        match sync::place(&path, config) {
            Place::NoSync => {}
            Place::Ambiguous(found) => self.choose_vcs = Some(found),
            Place::Repo(key) => {
                if let Some(repo) = ensure_repo(repos, &key, ctx) {
                    repo.worker.check(key.root.clone(), path);
                    self.repo = Some(key);
                    self.checking = true;
                }
            }
        }
    }

    /// Closes the file: its repository has conflicts, which a reload would read as items. Pending
    /// changes cannot be written into it. It stays among the open files, with the reason.
    fn close_conflicted(&mut self) {
        self.repo = None;
        self.checking = false;
        // A file waiting for the outside-changes answer is in the repository's syncs too.
        self.asking = None;
        let file = self.library.take().map(|library| library.file_name);
        let lost = match std::mem::take(&mut self.pending).len() {
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

    /// Closes the file after the user declined to commit its outside changes.
    fn close_modified(&mut self) {
        self.repo = None;
        let file = self.library.take().map(|library| library.file_name);
        self.error = Some(format!(
            "{} has changes rtm did not commit; commit or revert them by hand, then load the file \
             again.",
            file.unwrap_or_default()
        ));
    }

    /// Writes the pending changes and queues their commit on `repo`, the file's repository,
    /// unless it has a job outstanding: it may be a pull about to change the file, and the changes
    /// are redone on what it brings.
    fn flush(&mut self, repo: Option<&mut RepoSync>) {
        // During the check on open a write would look like an outside change.
        if self.pending.is_empty()
            || self.checking
            || repo.as_ref().is_some_and(|repo| repo.worker.busy())
        {
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
        // While the outside-changes question is open, a commit would take those changes (or add
        // the untracked file) before the user answers; the write alone keeps the edit.
        if let Some(repo) = repo.filter(|_| self.asking.is_none()) {
            let path = PathBuf::from(&library.file_name);
            repo.worker.commit(path, commit_message(&changes));
        }
    }

    /// Reads the file again after a pull changed it, redoes the pending changes on it, and, when
    /// the edit dialog is open on one of its items, finds that item in the result for `form`.
    fn reload(&mut self, form: Option<&mut EditForm>) {
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
        // `edited` is set only while the dialog is open on this file's item.
        let (Some(form), Some(original)) = (form, &self.edited) else {
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
}

impl eframe::App for App {
    /// Writes and commits the pending changes and syncs once more, within a few seconds.
    fn on_exit(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        self.finish_jobs(deadline);
        self.sync_now(Instant::now());
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
        // The status of the shown file's repository, or why the file does not sync.
        let status = match self.repo_of(self.file()) {
            _ if self.file().checking => Some("Checking…".to_owned()),
            Some(repo) if repo.worker.syncing() => Some("Syncing…".to_owned()),
            Some(repo) => repo.status.clone(),
            None => self.file().not_syncing.clone(),
        };
        // Sync now: shown when a repository has a remote, enabled unless all of those sync.
        let remotes: Vec<_> = self
            .repos
            .iter()
            .map(|r| &r.worker)
            .filter(|worker| worker.has_remote())
            .collect();
        let sync_button =
            (!remotes.is_empty()).then(|| !remotes.iter().all(|worker| worker.syncing()));
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
                Some(Outcome::Cancel) => self.close_editor(),
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

#[cfg(test)]
#[path = "tests/app_ui_tests.rs"]
mod ui_tests;

#[cfg(test)]
#[path = "tests/app_error_tests.rs"]
mod error_tests;
