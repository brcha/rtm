use chrono::{Duration, Local};
use clap::{Parser, Subcommand, ValueEnum};
use rtmsync::{State, Vcs};
use std::io::Write;
use std::path::{Path, PathBuf};
use todotxt::TodoItem;
use todotxt::TodoLibrary;
use todotxt::WriteLock;
use todotxt::{TodoContext, TodoProject};

#[derive(Parser)]
#[command(name = "rtmcli")]
#[command(about = "Rusty Todo.txt Manager CLI")]
struct Cli {
    /// Todo.txt file name
    #[arg(short = 'f', long)]
    file: Option<String>,

    /// Do not commit, pull or push, even in a repository
    #[arg(long, global = true, env = "RTM_NO_SYNC",
          value_parser = clap::builder::BoolishValueParser::new())]
    no_sync: bool,

    /// Version control system to use when the file's directory belongs to several
    #[arg(long, global = true, env = "RTM_VCS", value_enum)]
    vcs: Option<VcsName>,

    /// Commit changes to the file made outside rtm, or add an untracked file, before syncing
    #[arg(long, global = true)]
    commit_changes: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List items
    List {
        /// List completed items
        #[arg(short, long)]
        completed: bool,

        /// Filters, in any order: one due date filter (today, week, all, overdue, future),
        /// +project and @context. An item must match all of them
        #[arg(value_name = "FILTER")]
        filters: Vec<String>,
    },
    /// List the projects in the file with their number of open items
    Projects,
    /// List the contexts in the file with their number of open items
    Contexts,
    /// Add a new item
    Add {
        /// Todo description in Todo.txt format
        description: String,
    },
    /// Complete items by filter or identifier
    Complete {
        /// First arg: filter or identifier
        arg1: String,

        /// Second arg: optional index or identifier
        arg2: Option<String>,
    },
    /// Delete an item and its sub-items
    Delete {
        /// Item number as shown by `list` (or `list -c` with -c), or a UUID
        id: String,

        /// The number counts completed items
        #[arg(short, long)]
        completed: bool,

        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },
    /// Pull and push the repository holding the file
    Sync,
    /// Check whether a newer release of rtm exists (the only command that uses the network)
    CheckUpdate {
        /// Release channel: beta is also offered beta releases
        #[arg(long, value_enum, default_value_t = UpdateChannel::Stable)]
        channel: UpdateChannel,
    },
}

#[derive(ValueEnum, Clone, Copy, PartialEq)]
enum VcsName {
    Git,
    Pijul,
}

impl VcsName {
    fn vcs(self) -> Vcs {
        match self {
            VcsName::Git => Vcs::Git,
            VcsName::Pijul => Vcs::Pijul,
        }
    }
}

#[derive(ValueEnum, Clone, Copy, PartialEq)]
enum UpdateChannel {
    Stable,
    Beta,
}

#[cfg(feature = "update-check")]
impl From<UpdateChannel> for rtmupdate::Channel {
    fn from(channel: UpdateChannel) -> Self {
        match channel {
            UpdateChannel::Stable => rtmupdate::Channel::Stable,
            UpdateChannel::Beta => rtmupdate::Channel::Beta,
        }
    }
}

/// Prints whether a newer release exists and exits: 0 with an answer, 1 when the check failed.
#[cfg(feature = "update-check")]
fn check_update(channel: UpdateChannel) -> ! {
    let version = env!("CARGO_PKG_VERSION");
    match rtmupdate::check(version, channel.into()) {
        Ok(Some(release)) => println!("rtm {} is available: {}", release.version, release.url),
        Ok(None) => println!("rtmcli {version} is up to date"),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
    std::process::exit(0);
}

/// Builds without the update check (the Nix package) keep the command, to say how to update.
#[cfg(not(feature = "update-check"))]
fn check_update(_channel: UpdateChannel) -> ! {
    println!(
        "This build of rtmcli has no update check; update it through the package manager that \
         installed it (for Nix, the flake input)."
    );
    std::process::exit(0);
}

/// The repository a todo.txt is synced through: every change is committed, and pushed when the
/// repository has a remote.
struct Repo {
    vcs: Vcs,
    file: PathBuf,
    remote: bool,
}

impl Repo {
    /// The repository holding `file`, when it should be synced: sync is not turned off, the file
    /// is tracked, and its directory belongs to one VCS (or `choice` picks one). With
    /// `commit_changes`, changes made outside rtm are committed first, and an untracked file is
    /// added. Exits on an ambiguous directory, a mismatching `choice`, a conflicted repository,
    /// and outside changes without `commit_changes`.
    fn find(
        file: &str,
        no_sync: bool,
        choice: Option<VcsName>,
        commit_changes: bool,
    ) -> Option<Repo> {
        if no_sync {
            return None;
        }
        let file = PathBuf::from(file);
        let found = rtmsync::detect(&file);
        let vcs = match (found.as_slice(), choice.map(VcsName::vcs)) {
            ([], _) => return None,
            ([only], None) => *only,
            (found, Some(chosen)) if found.contains(&chosen) => chosen,
            ([only], Some(chosen)) => {
                eprintln!(
                    "'{}' is in a {only:?} repository, not {chosen:?}",
                    file.display()
                );
                std::process::exit(2);
            }
            (found, _) => {
                eprintln!(
                    "'{}' is in a directory used by {found:?}; choose one with --vcs or RTM_VCS",
                    file.display()
                );
                std::process::exit(2);
            }
        };
        let state = if file.exists() {
            vcs.state(&file)
        } else {
            // A missing file's state cannot be told reliably; it keeps the behaviour from before
            // the check: synced when tracked, silently not otherwise.
            vcs.is_tracked(&file).map(|tracked| {
                if tracked {
                    State::Clean
                } else {
                    State::Ignored
                }
            })
        };
        let state = match state {
            Ok(State::Ignored) => return None,
            Ok(State::Untracked) if !commit_changes => {
                eprintln!(
                    "'{}' is not tracked by {vcs:?}; run with --commit-changes to add it and sync, \
                     or ignore it in the repository to silence this",
                    file.display()
                );
                return None;
            }
            Ok(state) => state,
            Err(e) => {
                eprintln!("Not syncing: {e}");
                return None;
            }
        };
        match vcs.is_conflicted(&file) {
            Ok(false) => {}
            Ok(true) => conflicted(&file),
            Err(e) => {
                eprintln!("Not syncing: {e}");
                return None;
            }
        }
        let message = match state {
            State::Modified if !commit_changes => {
                eprintln!(
                    "'{}' has changes not committed by rtm (made by hand, or with --no-sync); \
                     commit or revert them, or run with --commit-changes",
                    file.display()
                );
                std::process::exit(1);
            }
            State::Modified => Some("rtm: commit outside changes".to_owned()),
            State::Untracked => Some(format!(
                "rtm: track {}",
                file.file_name().unwrap_or_default().to_string_lossy()
            )),
            State::Clean | State::Ignored => None,
        };
        if let Some(message) = message
            && let Err(e) = vcs.commit(&file, &message)
        {
            eprintln!("Not committed: {e}");
            std::process::exit(1);
        }
        let remote = vcs.has_remote(&file).unwrap_or(false);
        Some(Repo { vcs, file, remote })
    }

    /// Pulls and pushes when there is a remote. Returns whether it synced; a failure other than
    /// a conflict is only a warning, since the file is still saved and committed locally.
    fn sync(&self) -> bool {
        if !self.remote {
            return false;
        }
        match self.vcs.sync(&self.file) {
            Ok(()) => true,
            Err(rtmsync::Error::Conflict) => conflicted(&self.file),
            Err(e) => {
                eprintln!("Not synced: {e}");
                false
            }
        }
    }

    /// Commits the saved file with `message`, then syncs (pulls and pushes) when there is a remote.
    fn commit_and_sync(&self, message: &str) {
        if let Err(e) = self.vcs.commit(&self.file, message) {
            eprintln!("Not committed: {e}");
            return;
        }
        self.sync();
    }
}

/// Exits: the repository has conflicts the user resolves by hand.
fn conflicted(file: &Path) -> ! {
    eprintln!(
        "The repository holding '{}' has conflicts or a stopped rebase or merge; resolve them by \
         hand, then run rtmcli again",
        file.display()
    );
    std::process::exit(1);
}

/// The write lock rtmapp and the MCP server share (RTM-142, RTM-158), on the file's repository, or
/// on the file outside one: held while this command reads, pulls, changes and commits it, so no other
/// rtm process writes the file meanwhile. Waits for it. A file whose directory does not exist has
/// nothing to protect (the command reports it); any other failure exits when the command writes or
/// pulls (`needed`), and is only a warning for a plain read.
fn lock(file: &str, needed: bool) -> Option<WriteLock> {
    let target = rtmsync::lock_target(Path::new(file));
    match WriteLock::acquire(&target) {
        Ok(lock) => Some(lock),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) if needed => {
            eprintln!("Cannot lock '{}': {e}", target.display());
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Warning: cannot lock '{}': {e}", target.display());
            None
        }
    }
}

/// Pulls (when `repo` has a remote), then reloads `lib` so it holds what the pull brought.
fn pull_and_reload(repo: Option<&Repo>, lib: &mut TodoLibrary, file_name: &str) {
    if repo.is_some_and(Repo::sync)
        && let Err(e) = lib.load()
    {
        eprintln!("Error loading file '{}': {}", file_name, e);
        std::process::exit(1);
    }
}

/// The index of `target` in `lib` after a pull or a question, or exits: the pull or another program
/// changed or removed it meanwhile, and the change the user asked for no longer applies to what
/// they saw.
fn find_again(lib: &TodoLibrary, target: &TodoItem, taken: &[usize]) -> usize {
    let items = lib.list_items();
    if let Some(i) = (0..items.len()).find(|i| items[*i] == *target && !taken.contains(i)) {
        return i;
    }
    eprintln!("'{target}' was changed or removed meanwhile; nothing was changed.");
    if let Some(fresh) = target
        .uuid
        .and_then(|uuid| items.iter().find(|item| item.uuid == Some(uuid)))
    {
        eprintln!("It now reads: {fresh}");
    }
    std::process::exit(1);
}

#[derive(Clone, PartialEq)]
enum Filter {
    Today,
    Week,
    All,
    Overdue,
    Future,
}

fn parse_filter(s: &str) -> Option<Filter> {
    match s {
        "today" => Some(Filter::Today),
        "week" => Some(Filter::Week),
        "all" => Some(Filter::All),
        "overdue" => Some(Filter::Overdue),
        "future" => Some(Filter::Future),
        _ => None,
    }
}

/// Sorts `list`'s terms into its due date filter, projects and contexts, or exits 2 on an unknown
/// term or a second date filter.
fn list_filters(terms: &[String]) -> (Option<Filter>, Vec<TodoProject>, Vec<TodoContext>) {
    let mut filter: Option<(&str, Filter)> = None;
    let mut projects = Vec::new();
    let mut contexts = Vec::new();
    for term in terms {
        if let Some(date) = parse_filter(term) {
            if let Some((first, _)) = filter {
                eprintln!("Error: only one due date filter, got '{first}' and '{term}'");
                std::process::exit(2);
            }
            filter = Some((term, date));
        } else if let Ok(project) = term.parse() {
            projects.push(project);
        } else if let Ok(context) = term.parse() {
            contexts.push(context);
        } else {
            eprintln!(
                "Error: unknown filter '{term}': use today, week, all, overdue, future, \
                 +project or @context"
            );
            std::process::exit(2);
        }
    }
    (filter.map(|(_, date)| date), projects, contexts)
}

fn main() {
    let cli = Cli::parse();
    // Before the file and its repository are looked at: the check reads no file and syncs nothing.
    if let Commands::CheckUpdate { channel } = cli.command {
        check_update(channel);
    }
    // Before the repository is looked at, so a usage error never commits, syncs or hints first.
    let list_filters = match &cli.command {
        Commands::List { filters, .. } => Some(list_filters(filters)),
        _ => None,
    };
    let file_name = cli
        .file
        .unwrap_or_else(|| std::env::var("TODOTXT").unwrap_or_else(|_| "todo.txt".to_string()));

    let mut lib = TodoLibrary::new(file_name.clone());
    // Before the repository check, which may commit and reads the file's state: another rtm
    // process between its save and its commit would look like an outside change. A read that
    // cannot pull (sync off, or no repository) only needs it to see a whole file.
    let writes = matches!(
        cli.command,
        Commands::Add { .. } | Commands::Complete { .. } | Commands::Delete { .. } | Commands::Sync
    );
    let may_pull = !cli.no_sync && rtmsync::root(Path::new(&file_name)).is_some();
    let mut held = lock(&file_name, writes || may_pull);
    let repo = Repo::find(&file_name, cli.no_sync, cli.vcs, cli.commit_changes);
    let repo = repo.as_ref();

    match cli.command {
        Commands::List { completed, .. } => {
            let (filter, projects, contexts) = list_filters.expect("parsed for list above");
            if let Some(repo) = repo {
                repo.sync();
            }
            if let Err(e) = lib.load() {
                eprintln!("Error loading file '{}': {}", file_name, e);
                std::process::exit(1);
            }

            let today = Local::now().date_naive();
            let date_filter = match filter {
                Some(Filter::Today) => Some((today, today)),
                Some(Filter::Week) => Some((today, today + Duration::days(6))),
                Some(Filter::All) => None,
                Some(Filter::Overdue) => Some((chrono::NaiveDate::MIN, today - Duration::days(1))),
                Some(Filter::Future) => Some((today + Duration::days(1), chrono::NaiveDate::MAX)),
                None => None,
            };

            let filtered_items: Vec<_> = lib
                .list_items()
                .iter()
                .filter(|item| {
                    item.done == completed
                        && match date_filter {
                            None => true,
                            Some((start, end)) => item.due.is_some_and(|d| d >= start && d <= end),
                        }
                        && item.matches(&projects, &contexts)
                })
                .collect();

            println!("Items in '{}':", file_name);
            for (i, item) in filtered_items.iter().enumerate() {
                println!("{}. {}", i + 1, item);
            }

            if filtered_items.is_empty() {
                println!("No items found.");
            }
        }
        Commands::Projects | Commands::Contexts => {
            if let Some(repo) = repo {
                repo.sync();
            }
            if let Err(e) = lib.load() {
                eprintln!("Error loading file '{}': {}", file_name, e);
                std::process::exit(1);
            }
            let (kind, tags): (&str, Vec<(String, usize)>) =
                if matches!(cli.command, Commands::Projects) {
                    let tags = lib.projects().into_iter();
                    ("Projects", tags.map(|(p, n)| (p.to_string(), n)).collect())
                } else {
                    let tags = lib.contexts().into_iter();
                    ("Contexts", tags.map(|(c, n)| (c.to_string(), n)).collect())
                };
            println!("{} in '{}':", kind, file_name);
            for (tag, open) in &tags {
                println!("{tag} ({open})");
            }
            if tags.is_empty() {
                println!("No {} found.", kind.to_lowercase());
            }
        }
        Commands::Add { description } => {
            if let Some(repo) = repo {
                repo.sync();
            }
            // Try to load existing items to preserve them
            let _ = lib.load(); // Ignore error if file doesn't exist
            let item: TodoItem = description.parse().unwrap_or_else(|e| {
                eprintln!("Error parsing todo: {:?}", e);
                std::process::exit(1);
            });
            lib.add_item(item);
            if let Err(e) = lib.save() {
                eprintln!("Error saving file: {}", e);
                std::process::exit(1);
            }
            println!("Added item to '{}'", file_name);
            if let Some(repo) = repo {
                repo.commit_and_sync(&format!("rtm: add {description}"));
            }
        }
        Commands::Complete { arg1, arg2 } => {
            if let Err(e) = lib.load() {
                eprintln!("Error loading file '{}': {}", file_name, e);
                std::process::exit(1);
            }

            let today = Local::now().date_naive();
            let items = lib.list_items();
            let mut indices_to_complete = Vec::new();

            let arg1_ref = &arg1;
            if let Some(filter_name) = parse_filter(arg1_ref) {
                let date_filter = match filter_name {
                    Filter::Today => Some((today, today)),
                    Filter::Week => Some((today, today + Duration::days(6))),
                    Filter::All => None,
                    Filter::Overdue => Some((chrono::NaiveDate::MIN, today - Duration::days(1))),
                    Filter::Future => Some((today + Duration::days(1), chrono::NaiveDate::MAX)),
                };

                let filtered_indices: Vec<usize> = items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| {
                        !item.done
                            && date_filter.is_none_or(|(start, end)| {
                                item.due.is_some_and(|d| d >= start && d <= end)
                            })
                    })
                    .map(|(i, _)| i)
                    .collect();

                if let Some(ref arg2) = arg2 {
                    if let Ok(index) = arg2.parse::<usize>() {
                        if index > 0 && index <= filtered_indices.len() {
                            indices_to_complete.push(filtered_indices[index - 1]);
                        } else {
                            eprintln!("Invalid index for filter: {}", index);
                            std::process::exit(1);
                        }
                    } else if let Ok(uuid) = arg2.parse::<uuid::Uuid>() {
                        let mut found = false;
                        for &i in &filtered_indices {
                            if items[i].uuid == Some(uuid) {
                                indices_to_complete.push(i);
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            eprintln!("UUID {} not found in filtered list", uuid);
                            std::process::exit(1);
                        }
                    } else {
                        eprintln!("Invalid second arg: {}", arg2);
                        std::process::exit(1);
                    }
                } else {
                    // No second arg, complete all filtered
                    indices_to_complete = filtered_indices;
                }
            } else {
                // arg1 is identifier
                if let Ok(index) = arg1_ref.parse::<usize>() {
                    let uncompleted: Vec<usize> =
                        (0..items.len()).filter(|&i| !items[i].done).collect();
                    if index > 0 && index <= uncompleted.len() {
                        indices_to_complete.push(uncompleted[index - 1]);
                    } else {
                        eprintln!("Invalid absolute index: {}", index);
                        std::process::exit(1);
                    }
                } else if let Ok(uuid) = arg1_ref.parse::<uuid::Uuid>() {
                    let mut found = false;
                    for (i, item) in items.iter().enumerate() {
                        if item.uuid == Some(uuid) && !item.done {
                            indices_to_complete.push(i);
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        eprintln!("UUID {} not found or already completed", uuid);
                        std::process::exit(1);
                    }
                } else {
                    eprintln!("Invalid arg: {}", arg1);
                    std::process::exit(1);
                }
            }

            // Resolved above against the file before the pull; the same items are looked up again
            // after it, so a pull that renumbered items cannot redirect the change.
            let targets: Vec<TodoItem> = indices_to_complete
                .iter()
                .map(|&i| items[i].clone())
                .collect();
            pull_and_reload(repo, &mut lib, &file_name);
            let mut indices_to_complete = Vec::new();
            for target in &targets {
                let i = find_again(&lib, target, &indices_to_complete);
                indices_to_complete.push(i);
            }

            for &i in &indices_to_complete {
                lib.complete_item(i).unwrap();
            }

            if !indices_to_complete.is_empty() {
                if let Err(e) = lib.save() {
                    eprintln!("Error saving file: {}", e);
                    std::process::exit(1);
                }
                println!(
                    "Completed {} item(s) in '{}'",
                    indices_to_complete.len(),
                    file_name
                );
                let message = match targets.as_slice() {
                    [one] => format!("rtm: complete {one}"),
                    many => format!("rtm: complete {} items", many.len()),
                };
                if let Some(repo) = repo {
                    repo.commit_and_sync(&message);
                }
            } else {
                println!("No items matched the criteria");
            }
        }
        Commands::Delete { id, completed, yes } => {
            if let Err(e) = lib.load() {
                eprintln!("Error loading file '{}': {}", file_name, e);
                std::process::exit(1);
            }

            let items = lib.list_items();
            let index = if let Ok(n) = id.parse::<usize>() {
                let rows: Vec<usize> = (0..items.len())
                    .filter(|&i| items[i].done == completed)
                    .collect();
                if n == 0 || n > rows.len() {
                    eprintln!("Invalid index: {}", n);
                    std::process::exit(1);
                }
                rows[n - 1]
            } else if let Ok(uuid) = id.parse::<uuid::Uuid>() {
                items
                    .iter()
                    .position(|item| item.uuid == Some(uuid))
                    .unwrap_or_else(|| {
                        eprintln!("UUID {} not found", uuid);
                        std::process::exit(1);
                    })
            } else {
                eprintln!("Invalid arg: {}", id);
                std::process::exit(1);
            };

            // Resolved against the file before the pull, then looked up again after it.
            let target = items[index].clone();
            pull_and_reload(repo, &mut lib, &file_name);
            let index = find_again(&lib, &target, &[]);

            // Removed in memory only; the file is written after confirmation.
            let mut removed = lib.remove_item(index).unwrap();
            // remove_item returns file order, where a sub-item can precede its parent.
            let pos = removed.iter().position(|item| *item == target).unwrap();
            removed.remove(pos);
            println!("Deleting from '{}':", file_name);
            println!("{}", target);
            for item in &removed {
                println!("  {}", item);
            }

            // After the question the repository is checked again: an editor may have left changes,
            // or a pull a conflict, while it waited.
            let mut rechecked = None;
            let asked = !yes;
            if !yes {
                // No other rtm process waits on a person: the lock is released while asking, and the
                // file is read again afterwards.
                drop(held.take());
                print!("Delete? [y/N] ");
                std::io::stdout().flush().ok();
                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer).ok();
                let answer = answer.trim().to_lowercase();
                if answer != "y" && answer != "yes" {
                    println!("Nothing deleted.");
                    return;
                }
                held = lock(&file_name, true);
                rechecked = Repo::find(&file_name, cli.no_sync, cli.vcs, cli.commit_changes);
                if let Err(e) = lib.load() {
                    eprintln!("Error loading file '{}': {}", file_name, e);
                    std::process::exit(1);
                }
                let index = find_again(&lib, &target, &[]);
                let mut again = lib.remove_item(index).unwrap();
                again.remove(again.iter().position(|item| *item == target).unwrap());
                if again != removed {
                    eprintln!(
                        "The sub-items of '{target}' were changed meanwhile; nothing was deleted."
                    );
                    std::process::exit(1);
                }
            }

            if let Err(e) = lib.save() {
                eprintln!("Error saving file: {}", e);
                std::process::exit(1);
            }
            println!("Deleted {} item(s) from '{}'", removed.len() + 1, file_name);
            let repo = if asked { rechecked.as_ref() } else { repo };
            if let Some(repo) = repo {
                repo.commit_and_sync(&format!("rtm: delete {target}"));
            }
        }
        Commands::Sync => match repo {
            Some(repo) if !repo.remote => println!("'{}' has no remote to sync with", file_name),
            Some(repo) => {
                // The pull and push are this command's whole job, so failing them is an error.
                if !repo.sync() {
                    std::process::exit(1);
                }
                println!("Synced '{}'", file_name);
            }
            None => println!(
                "'{}' is not synced (not in a repository, not tracked, or sync is off)",
                file_name
            ),
        },
        Commands::CheckUpdate { .. } => unreachable!("handled before the file is resolved"),
    }
    // Held until the command is done, its commit and push included.
    drop(held);
}
