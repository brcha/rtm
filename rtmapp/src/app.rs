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
use crate::sync::{self, Done, Found, Place, Question, Repo, RepoKey, Worker};
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
    /// The open files (RTM-66), one per entry of the saved session, in its order (RTM-127).
    files: Vec<OpenFile>,
    /// The path of the file shown (`OpenFile::path`); none when no file is open.
    active: Option<PathBuf>,
    /// The app line, above the file's: the config, a file that could not be created or opened,
    /// the quick-add input. Shown until the next successful change or config save.
    error: Option<String>,
    /// The quick-add input.
    new_item: String,
    /// The open edit dialog, if any.
    editing: Option<EditForm>,
    /// The open Settings dialog, if any.
    settings: Option<SettingsForm>,
    /// The file whose tab's close button was clicked, while the app asks before closing it.
    confirm_close: Option<PathBuf>,
    /// Settings changed `sync`: reconnect once no job is outstanding (`reconnect_when_idle`).
    reconnect: bool,
    /// The repositories the open files sync through (RTM-64), one worker each (RTM-126).
    repos: Vec<RepoSync>,
    /// The workers of repositories whose last open file was closed, until their last commit and
    /// sync are done; exit waits for them too.
    closing: Vec<Worker>,
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
    /// Its entry's path: canonical once loaded, as the config gave it until then. It names the
    /// file while it is not loaded (missing, failed, closed by a conflict or "Don't open").
    path: PathBuf,
    /// The tab's label: its entry's `name`, else the file name.
    name: String,
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
        let (files, active) = open_session(&mut config);
        let mut app = App {
            config,
            config_path,
            files,
            active,
            error: None,
            new_item: String::new(),
            editing: None,
            settings: None,
            confirm_close: None,
            reconnect: false,
            repos: Vec::new(),
            closing: Vec::new(),
            dialog: None,
            ctx: cc.egui_ctx.clone(),
        };
        app.connect_all();
        app
    }

    /// The file shown, unless no file is open.
    fn file(&self) -> Option<&OpenFile> {
        let active = self.active.as_deref()?;
        self.files.iter().find(|file| file.is(active))
    }

    fn file_mut(&mut self) -> Option<&mut OpenFile> {
        find_file(&mut self.files, self.active.as_deref())
    }

    /// The shown file's items, while it is loaded.
    fn library(&self) -> Option<&TodoLibrary> {
        self.file()?.library.as_ref()
    }

    /// The repository `file` syncs through.
    fn repo_of(&self, file: &OpenFile) -> Option<&RepoSync> {
        let key = file.repo.as_ref()?;
        self.repos.iter().find(|repo| &repo.key == key)
    }

    /// Starts sync for the shown file: its check on open runs on the repository's worker, and the
    /// file syncs once that passes (`checked`). A file it replaced has left its repository first.
    fn connect(&mut self) {
        let Some(file) = find_file(&mut self.files, self.active.as_deref()) else {
            return;
        };
        file.connect(&mut self.repos, &self.config, &self.ctx);
        self.prune();
    }

    /// Starts sync for every open file, as at startup and after Settings changed `sync`.
    fn connect_all(&mut self) {
        for file in &mut self.files {
            file.connect(&mut self.repos, &self.config, &self.ctx);
        }
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
        if self.library().is_none() {
            // A conflicted repository closed the shown file.
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
        let Some(file) = find_file(&mut self.files, self.active.as_deref()) else {
            return;
        };
        let Some((repo, question)) = file.asking.take() else {
            return;
        };
        if yes {
            let message = repo.commit_message(question);
            let path = file.loaded_path();
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

    /// Whether any repository has a job outstanding: Settings' reconnect waits for it.
    fn busy(&self) -> bool {
        self.repos.iter().any(|repo| repo.worker.busy())
    }

    /// Applies `change` to the list at once and writes it when the worker is idle.
    fn apply(&mut self, change: Change) {
        let Some(file) = self.file_mut() else {
            return;
        };
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
            // A failed write is in the file's line.
            let _ = file.flush(find_repo(&mut self.repos, file.repo.as_ref()));
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
        // No open file is left for their results.
        for worker in &mut self.closing {
            worker.poll();
        }
        self.closing.retain(Worker::busy);
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
                if self.library().is_none() {
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
        if let Some(file) = self.file_mut() {
            file.edited = None;
            file.deleting = None;
            file.stale = false;
        }
    }

    /// Opens the edit dialog on the item at `index`.
    fn open_editor(&mut self, index: usize) {
        let Some(file) = find_file(&mut self.files, self.active.as_deref()) else {
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
        let stale = self.file().is_some_and(|file| file.stale);
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
        let Some(found) = self.file().and_then(|file| file.choose_vcs.as_ref()) else {
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
            Some("") => {
                if let Some(file) = self.file_mut() {
                    file.choose_vcs = None;
                }
            }
            Some(name) => {
                let file = self.file().and_then(OpenFile::loaded_path);
                if let Some(file) = file {
                    self.config.set_vcs(&file.to_string_lossy(), name);
                }
                self.save_config();
                self.connect();
            }
        }
    }

    fn sync_question(&mut self, ctx: &egui::Context, p: &Palette) {
        let Some(file) = self.file() else {
            return;
        };
        let Some((repo, question)) = &file.asking else {
            return;
        };
        let name = file
            .path
            .file_name()
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
        // Matches the disabled menu entries.
        if self.dialog.is_some() {
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
        if self.dialog.is_some() {
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

    /// Applies the file dialog's result once no item is being edited: opening a file shows its
    /// tab. Until then the result waits in the channel; closing the edit dialog repaints.
    fn dialog_result(&mut self) {
        if self.editing.is_some() {
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

    /// Creates `path` empty and opens it. A file open in a tab is not replaced: its tab would keep
    /// the old items and write them back.
    fn create_file(&mut self, path: &Path) {
        if let Ok(canonical) = path.canonicalize()
            && self
                .files
                .iter()
                .any(|file| file.library.is_some() && file.is(&canonical))
        {
            self.error = Some(format!(
                "{} is open; close its tab before replacing it.",
                canonical.display()
            ));
            return;
        }
        match std::fs::File::create(path) {
            Ok(_) => self.open_file(path),
            Err(e) => self.error = Some(format!("{}: {e}", path.display())),
        }
    }

    /// Shows `path`: the tab of the file when it is open (loading it again when it is not loaded,
    /// as the file lines ask), otherwise a new tab, kept in the config. Its sync starts.
    fn open_file(&mut self, path: &Path) {
        let library = match load_library(path) {
            Ok(library) => library,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        let name = library.file_name.clone();
        let canonical = PathBuf::from(&name);
        let connect = match self
            .files
            .iter_mut()
            .find(|file| file.resolves_to(&canonical))
        {
            Some(file) if file.library.is_some() => false,
            Some(file) => {
                self.config.rename(&file.path.to_string_lossy(), &name);
                file.path = canonical.clone();
                file.library = Some(library);
                file.error = None;
                true
            }
            None => {
                self.config.add_file(&name);
                self.files.push(OpenFile::loaded(library, None));
                true
            }
        };
        self.active = Some(canonical);
        if connect {
            self.connect();
        }
        self.config.set_active(&name);
        self.save_config();
    }

    /// Closes the tab of the open file at `path`: its pending changes are written and committed
    /// first, and its entry leaves the config. Refused while the edit dialog is open, while its
    /// check on open runs or its repository has a job outstanding, and when the write fails.
    /// Closing the last open file of a repository syncs it once more, so its commits reach the
    /// remote.
    fn close(&mut self, path: &Path) {
        if !self
            .files
            .iter()
            .any(|file| file.is(path) && self.can_close(file))
        {
            return;
        }
        let Some(file) = self.files.iter_mut().find(|file| file.is(path)) else {
            return;
        };
        let mut repo = find_repo(&mut self.repos, file.repo.as_ref());
        if !file.flush(repo.as_deref_mut()) {
            return;
        }
        let key = file.repo.clone();
        let loaded = file.loaded_path();
        let others =
            |other: &OpenFile| !other.is(path) && other.repo.is_some() && other.repo == key;
        if let Some(repo) = repo
            && repo.worker.has_remote()
            && !self.files.iter().any(others)
            && let Some(loaded) = loaded
        {
            repo.worker.sync(vec![loaded]);
        }
        let Some(index) = self.files.iter().position(|file| file.is(path)) else {
            return;
        };
        let closed = self.files.remove(index);
        if self.active.as_deref() == Some(path) {
            // The tab to its right takes its place, else the one to its left.
            let next = self.files.get(index).or(self.files.last());
            self.active = next.map(|file| file.path.clone());
        }
        self.config.remove_file(&closed.path.to_string_lossy());
        if let Some(key) = &closed.repo
            && !self
                .files
                .iter()
                .any(|file| file.repo.as_ref() == Some(key))
            && let Some(at) = self.repos.iter().position(|repo| &repo.key == key)
        {
            let repo = self.repos.remove(at);
            if repo.worker.busy() {
                self.closing.push(repo.worker);
            }
        }
        if let Some(active) = &self.active {
            self.config.set_active(&active.to_string_lossy());
        }
        self.prune();
        self.save_config();
    }

    /// Whether `close` would close `file` now: not while the edit dialog is open, while its check
    /// on open runs or while its repository has a job outstanding.
    fn can_close(&self, file: &OpenFile) -> bool {
        self.editing.is_none()
            && !file.checking
            && self.repo_of(file).is_none_or(|repo| !repo.worker.busy())
    }

    /// Asks before closing the tab of `confirm_close`, so a stray click on its close button does
    /// not close it. Close tab waits, disabled, while `close` would refuse.
    fn close_question(&mut self, ctx: &egui::Context, p: &Palette) {
        let Some(path) = self.confirm_close.clone() else {
            return;
        };
        let Some(file) = self.files.iter().find(|file| file.is(&path)) else {
            self.confirm_close = None;
            return;
        };
        let closable = self.can_close(file);
        let mut answer = None;
        let cancel = theme::question(
            ctx,
            p,
            "close-question",
            &format!("Close {}?", file.name),
            "The tab closes and the app no longer opens the file at start. The file itself stays \
             where it is; Load File opens it again.",
            |ui| {
                if ui
                    .add_enabled_ui(closable, |ui| theme::primary_button(ui, "Close tab"))
                    .inner
                    .clicked()
                {
                    answer = Some(true);
                }
                if theme::secondary_button(ui, "Cancel").clicked() {
                    answer = Some(false);
                }
            },
        );
        if cancel {
            answer = Some(false);
        }
        if let Some(yes) = answer {
            self.confirm_close = None;
            if yes {
                self.close(&path);
            }
        }
    }

    /// Shows the open file at `path`, a click on its tab; it is shown at the next start too.
    fn switch(&mut self, path: &Path) {
        if self.editing.is_some() {
            return;
        }
        if !self.files.iter().any(|file| file.is(path)) {
            return;
        }
        self.active = Some(path.to_owned());
        self.config.set_active(&path.to_string_lossy());
        self.save_config();
    }

    /// Shows the first file (in tab order) with a question waiting, the Git-or-Pijul choice or the
    /// outside changes, unless a dialog is open or the shown file has one. Only `active` moves:
    /// the checks finish a few frames after startup, and nothing is written at startup.
    fn show_question(&mut self) {
        if self.modal_open() {
            return;
        }
        let waiting = |file: &OpenFile| file.choose_vcs.is_some() || file.asking.is_some();
        if let Some(file) = self.files.iter().find(|file| waiting(file)) {
            self.active = Some(file.path.clone());
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
    /// a change of `sync` reconnects once no repository has a job outstanding.
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
            self.connect_all();
        }
    }

    /// Whether a modal is open; the menu's shortcuts are inactive then.
    fn modal_open(&self) -> bool {
        self.editing.is_some()
            || self.settings.is_some()
            || self.confirm_close.is_some()
            || self
                .file()
                .is_some_and(|file| file.choose_vcs.is_some() || file.asking.is_some())
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
                let name = self
                    .file()
                    .map_or("No file loaded".into(), |file| file.path.to_string_lossy());
                // CSS: word-break: break-all, since a path has no spaces to wrap at.
                let mut job = LayoutJob::single_section(
                    name.into_owned(),
                    TextFormat::simple(egui::FontId::proportional(11.9), p.muted),
                );
                job.wrap.break_anywhere = true;
                ui.add(egui::Label::new(job));
            });
        self.filters(ui, p);
        let count = self.library().map_or(0, TodoLibrary::item_count);
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
        let item = self.library().and_then(|l| l.items.get(index));
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
        if self.library().is_none() {
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
        let active = self.active.as_deref();
        let file = self
            .files
            .iter()
            .find(|file| active.is_some_and(|path| file.is(path)));
        let library = file.and_then(|file| file.library.as_ref());
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
        let Some(file) = find_file(&mut self.files, self.active.as_deref()) else {
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
        let (Some(form), Some(library)) = (self.editing.take(), self.library()) else {
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

    /// One tab per open file, in order, then a `+` with New File and Load File.
    fn tab_strip(&self, ui: &mut Ui, p: &Palette, files_enabled: bool) -> TabStrip {
        let mut clicked = None;
        let mut shown = None;
        let strip = egui::ScrollArea::horizontal()
            .id_salt("tabs")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Clear of the page's rounded corner.
                    ui.add_space(6.0);
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for file in &self.files {
                        let active = self.file().is_some_and(|open| open.is(&file.path));
                        let tab = tab(ui, p, file, active, self.can_close(file));
                        clicked = clicked.take().or(tab.inner);
                        if active {
                            scroll_to_once(ui, &tab.response, &file.path);
                            shown = Some(tab.response.rect);
                        }
                    }
                    ui.add_space(6.0);
                    let button = theme::add_button(ui, p);
                    egui::Popup::menu(&button).show(|ui| {
                        let mut chosen = None;
                        file_entries(ui, files_enabled, &mut chosen);
                        if let Some(action) = chosen {
                            clicked = Some(TabAction::Menu(action));
                        }
                    });
                });
            });
        // A tab scrolled out of sight is not joined.
        let shown = shown.map(|rect| rect.intersect(strip.inner_rect));
        TabStrip {
            clicked,
            shown: shown.filter(|rect| rect.is_positive()),
        }
    }

    /// The tab strip above the page of the shown file: its lines, the add row and the list.
    fn tabs_and_page(&mut self, ui: &mut Ui, p: &Palette, files_enabled: bool) -> Option<Clicked> {
        let strip = self.tab_strip(ui, p, files_enabled);
        // The tabs stand on the page's top border.
        ui.add_space(-ui.spacing().item_spacing.y - 1.0);
        let row = Frame::new()
            .fill(p.panel)
            .stroke(Stroke::new(1.0, p.border))
            .corner_radius(6.0)
            .inner_margin(16.0)
            .show(ui, |ui| self.content(ui, p))
            .inner;
        if let Some(shown) = strip.shown {
            // The shown tab opens into the page: its part of the border is covered.
            let gap = egui::Rect::from_min_max(
                egui::pos2(shown.left() + 1.0, shown.bottom() - 1.0),
                egui::pos2(shown.right() - 1.0, shown.bottom() + 1.0),
            );
            ui.painter().rect_filled(gap, 0.0, p.panel);
        }
        strip.clicked.map(Clicked::Tab).or(row.map(Clicked::Row))
    }

    /// The app and file lines, the add row and the list. Returns what was clicked in the list.
    fn content(&mut self, ui: &mut Ui, p: &Palette) -> Option<RowAction> {
        let file_error = self.file().and_then(|file| file.error.as_ref());
        for error in [self.error.as_ref(), file_error].into_iter().flatten() {
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
                let Some(library) = self.library() else {
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

impl RepoSync {
    /// Queues a sync of the repository for those of `files` that sync through it.
    fn sync(&mut self, files: &[OpenFile], now: Instant) {
        let paths = files
            .iter()
            .filter(|file| file.repo.as_ref() == Some(&self.key))
            .filter_map(OpenFile::loaded_path)
            .collect();
        self.worker.sync(paths);
        self.last_sync = now;
    }
}

/// The open file at `path` among `files`.
fn find_file<'a>(files: &'a mut [OpenFile], path: Option<&Path>) -> Option<&'a mut OpenFile> {
    let path = path?;
    files.iter_mut().find(|file| file.is(path))
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
    /// The file of the session at `path`, not loaded yet; `name` is its entry's tab label.
    fn new(path: PathBuf, name: Option<String>) -> Self {
        let name = name.unwrap_or_else(|| {
            path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            )
        });
        OpenFile {
            path,
            name,
            ..OpenFile::default()
        }
    }

    /// A file loaded at its canonical path, labelled `name` or by its file name.
    fn loaded(library: TodoLibrary, name: Option<String>) -> Self {
        let mut file = OpenFile::new(PathBuf::from(&library.file_name), name);
        file.library = Some(library);
        file
    }

    /// The file's path, while it is loaded: only a loaded file is placed, checked or synced.
    fn loaded_path(&self) -> Option<PathBuf> {
        self.library.as_ref().map(|_| self.path.clone())
    }

    /// Whether this is the open file at `path`, loaded or not.
    fn is(&self, path: &Path) -> bool {
        self.path == path
    }

    /// Whether this is the open file at `canonical`, also when its entry names it another way
    /// (a file missing at startup is not resolved yet).
    fn resolves_to(&self, canonical: &Path) -> bool {
        self.is(canonical)
            || (self.library.is_none()
                && self.path.canonicalize().is_ok_and(|path| path == canonical))
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
        let Some(path) = self.loaded_path() else {
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
    /// are redone on what it brings. False when the write failed.
    fn flush(&mut self, repo: Option<&mut RepoSync>) -> bool {
        // During the check on open a write would look like an outside change.
        if self.pending.is_empty()
            || self.checking
            || repo.as_ref().is_some_and(|repo| repo.worker.busy())
        {
            return true;
        }
        let Some(library) = &mut self.library else {
            return true;
        };
        let changes = std::mem::take(&mut self.pending);
        // The Tauri app read the file again after a completion; kept.
        let reread = changes.iter().any(Change::completes);
        let saved = library
            .save()
            .and_then(|()| if reread { library.load() } else { Ok(()) });
        if let Err(e) = saved {
            self.error = Some(format!("{}: {e}", library.file_name));
            return false;
        }
        // While the outside-changes question is open, a commit would take those changes (or add
        // the untracked file) before the user answers; the write alone keeps the edit.
        if let Some(repo) = repo.filter(|_| self.asking.is_none()) {
            let path = PathBuf::from(&library.file_name);
            repo.worker.commit(path, commit_message(&changes));
        }
        true
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
        // A closed repository's commit gets as long as an open one's; its sync, the exit limit.
        let saving_deadline = deadline.max(Instant::now() + SAVE_TIMEOUT);
        for worker in &mut self.closing {
            while worker.busy() {
                let until = if worker.syncing() {
                    deadline
                } else {
                    saving_deadline
                };
                if worker
                    .next(until.saturating_duration_since(Instant::now()))
                    .is_none()
                {
                    break;
                }
            }
        }
    }

    /// Runs before each `ui`, and also while the window is hidden (when a repaint was requested).
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sync_results();
        self.reconnect_when_idle();
        self.show_question();
        self.dialog_result();
        self.tick(Instant::now());
    }

    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        let p = theme::palette(ui);
        // The status of the shown file's repository, or why the file does not sync.
        let status = self.file().and_then(|file| match self.repo_of(file) {
            _ if file.checking => Some("Checking…".to_owned()),
            Some(repo) if repo.worker.syncing() => Some("Syncing…".to_owned()),
            Some(repo) => repo.status.clone(),
            None => file.not_syncing.clone(),
        });
        // Sync now: shown when a repository has a remote, enabled unless all of those sync.
        let remotes: Vec<_> = self
            .repos
            .iter()
            .map(|r| &r.worker)
            .filter(|worker| worker.has_remote())
            .collect();
        let sync_button =
            (!remotes.is_empty()).then(|| !remotes.iter().all(|worker| worker.syncing()));
        // New File and Load File: disabled while a file dialog is open.
        let files_enabled = self.dialog.is_none();
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
            .show(ui, |ui| self.tabs_and_page(ui, p, files_enabled))
            .inner;
        match clicked {
            Some(Clicked::Tab(TabAction::Switch(path))) => self.switch(&path),
            Some(Clicked::Tab(TabAction::Close(path))) => self.confirm_close = Some(path),
            Some(Clicked::Tab(TabAction::Menu(action))) => self.menu_action(action, &ctx, frame),
            Some(Clicked::Row(RowAction::Toggle(index))) => self.toggle(index),
            Some(Clicked::Row(RowAction::Edit(index))) => self.open_editor(index),
            None => {}
        }
        self.vcs_question(&ui.ctx().clone(), p);
        self.sync_question(&ui.ctx().clone(), p);
        self.close_question(&ui.ctx().clone(), p);
        if let Some(form) = &mut self.editing {
            match edit::show(ui.ctx(), p, form) {
                Some(Outcome::Save) => self.save_edit(),
                Some(Outcome::Cancel) => self.close_editor(),
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

/// Loads every file of the saved session, in its order, and finds the one `file_name` names (else
/// the first). A file that fails to load keeps its place with the error in its line.
fn open_session(config: &mut AppConfig) -> (Vec<OpenFile>, Option<PathBuf>) {
    let mut files: Vec<OpenFile> = Vec::new();
    for entry in config.files.clone() {
        let file = match load_library(Path::new(&entry.path)) {
            // Entries are looked up by the resolved path from now on; written with the next
            // save, not now. Two entries for one file would open two tabs that overwrite
            // each other, so the second is dropped.
            Ok(library) => {
                config.rename(&entry.path, &library.file_name);
                if files
                    .iter()
                    .any(|file| file.is(Path::new(&library.file_name)))
                {
                    continue;
                }
                OpenFile::loaded(library, entry.name)
            }
            Err(e) => OpenFile {
                error: Some(e),
                ..OpenFile::new(PathBuf::from(&entry.path), entry.name)
            },
        };
        files.push(file);
    }
    let named = |file: &&OpenFile| {
        config
            .file_name
            .as_deref()
            .is_some_and(|name| file.is(Path::new(name)))
    };
    let active = files.iter().find(named).or(files.first());
    let active = active.map(|file| file.path.clone());
    (files, active)
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
        file_entries(ui, files_enabled, &mut chosen);
        ui.separator();
        menu_entry(ui, true, MenuAction::Settings, &mut chosen);
        ui.separator();
        menu_entry(ui, true, MenuAction::Quit, &mut chosen);
    });
    chosen
}

/// New File and Load File, in the menu and in the tab strip's `+` popup.
fn file_entries(ui: &mut Ui, enabled: bool, chosen: &mut Option<MenuAction>) {
    menu_entry(ui, enabled, MenuAction::NewFile, chosen);
    menu_entry(ui, enabled, MenuAction::LoadFile, chosen);
}

/// The menu entry of `action`, with its shortcut; sets `chosen` when clicked.
fn menu_entry(ui: &mut Ui, enabled: bool, action: MenuAction, chosen: &mut Option<MenuAction>) {
    let (text, shortcut) = match action {
        MenuAction::NewFile => ("New File…", &NEW_FILE),
        MenuAction::LoadFile => ("Load File…", &LOAD_FILE),
        MenuAction::Settings => ("Settings…", &SETTINGS),
        MenuAction::Quit => ("Quit", &QUIT),
    };
    let button = egui::Button::new(text).shortcut_text(shortcut_text(ui.ctx(), shortcut));
    if ui.add_enabled(enabled, button).clicked() {
        *chosen = Some(action);
    }
}

/// What was clicked in the tab strip: a tab, its close button, or an entry of the `+` popup.
enum TabAction {
    Switch(PathBuf),
    Close(PathBuf),
    Menu(MenuAction),
}

/// What the tab strip reports: what was clicked, and where the shown tab is, to join it to the
/// page below.
struct TabStrip {
    clicked: Option<TabAction>,
    shown: Option<egui::Rect>,
}

/// What was clicked in the content area: in the tab strip, or in the list.
enum Clicked {
    Tab(TabAction),
    Row(RowAction),
}

/// The tab of `file`, painted: its name (red while it has a file line), the full path on hover,
/// and a close button shown while the pointer is over the tab. The shown tab has the page's fill
/// and border, the others a faint fill, stronger under the pointer. Returns what was clicked, with
/// the tab's response.
fn tab(
    ui: &mut Ui,
    p: &Palette,
    file: &OpenFile,
    active: bool,
    closable: bool,
) -> egui::InnerResponse<Option<TabAction>> {
    let color = if file.error.is_some() {
        p.due
    } else if active {
        p.text
    } else {
        p.muted
    };
    let font = if active {
        theme::bold(12.6)
    } else {
        egui::FontId::proportional(12.6)
    };
    let galley = ui.painter().layout_no_wrap(file.name.clone(), font, color);
    // 12 px before the name, 6 px to the close button (16 px), 8 px after it.
    let size = egui::vec2(12.0 + galley.size().x + 6.0 + 16.0 + 8.0, 32.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Tab, true, &file.name));
    // egui reports `selected` as toggled; a tab is selected.
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_selected(active));
    // The load errors start with the path already.
    let path = file.path.display().to_string();
    let hover = match &file.error {
        Some(error) if error.contains(&path) => error.clone(),
        Some(error) => format!("{path}\n{error}"),
        None => path,
    };
    let response = response.on_hover_text(hover);
    let pointer = response.contains_pointer();
    let corners = egui::CornerRadius {
        nw: 6,
        ne: 6,
        sw: 0,
        se: 0,
    };
    if active {
        ui.painter().rect(
            rect,
            corners,
            p.panel,
            Stroke::new(1.0, p.border),
            egui::StrokeKind::Inside,
        );
    } else {
        // A faint fill makes each tab a unit; stronger under the pointer.
        let fill = p.border.gamma_multiply(if pointer { 0.7 } else { 0.35 });
        ui.painter().rect_filled(rect, corners, fill);
    }
    let text_at = egui::pos2(rect.left() + 12.0, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(text_at, galley, color);
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(2.0),
            4.0,
            Stroke::new(1.0, p.focus),
            egui::StrokeKind::Inside,
        );
    }
    let close_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 16.0, rect.center().y),
        egui::vec2(16.0, 16.0),
    );
    let name = format!("Close {}", file.name);
    // A child of its own, so the parent's cursor stays after the tab.
    let mut close_ui = ui.new_child(egui::UiBuilder::new().max_rect(close_rect));
    if !closable {
        close_ui.disable();
    }
    let close = theme::close_button(&mut close_ui, p, &name, pointer);
    let clicked = if close.clicked() {
        Some(TabAction::Close(file.path.clone()))
    } else if response.clicked() {
        Some(TabAction::Switch(file.path.clone()))
    } else {
        None
    };
    egui::InnerResponse::new(clicked, response)
}

/// Scrolls the tab strip to the shown tab when it becomes the shown one (at startup, on a click, on
/// opening a file), and leaves the strip where the user scrolled it otherwise.
fn scroll_to_once(ui: &Ui, tab: &egui::Response, path: &Path) {
    let id = egui::Id::unique("tabs-scrolled-to");
    let path = path.to_owned();
    if ui.data(|data| data.get_temp::<PathBuf>(id)).as_ref() != Some(&path) {
        tab.scroll_to_me(None);
        ui.data_mut(|data| data.insert_temp(id, path));
    }
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

#[cfg(test)]
#[path = "tests/app_tabs_tests.rs"]
mod tabs_tests;
