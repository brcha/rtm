use chrono::{Duration, Local};
use clap::{Parser, Subcommand, ValueEnum};
use rtmsync::{State, Vcs};
use std::io::Write;
use std::path::{Path, PathBuf};
use todotxt::TodoItem;
use todotxt::TodoLibrary;

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

        /// Filter by due date
        #[arg(value_enum)]
        filter: Option<Filter>,
    },
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

/// Pulls (when `repo` has a remote), then reloads `lib` so it holds what the pull brought.
fn pull_and_reload(repo: Option<&Repo>, lib: &mut TodoLibrary, file_name: &str) {
    if repo.is_some_and(Repo::sync)
        && let Err(e) = lib.load()
    {
        eprintln!("Error loading file '{}': {}", file_name, e);
        std::process::exit(1);
    }
}

/// The index of `target` in `lib` after a pull, or exits: the pull changed or removed it, and the
/// change the user asked for no longer applies to what they saw.
fn find_again(lib: &TodoLibrary, target: &TodoItem, taken: &[usize]) -> usize {
    let items = lib.list_items();
    if let Some(i) = (0..items.len()).find(|i| items[*i] == *target && !taken.contains(i)) {
        return i;
    }
    eprintln!("The sync changed or removed '{target}'; nothing was changed.");
    if let Some(fresh) = target
        .uuid
        .and_then(|uuid| items.iter().find(|item| item.uuid == Some(uuid)))
    {
        eprintln!("It now reads: {fresh}");
    }
    std::process::exit(1);
}

#[derive(ValueEnum, Clone, PartialEq)]
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

fn main() {
    let cli = Cli::parse();
    let file_name = cli
        .file
        .unwrap_or_else(|| std::env::var("TODOTXT").unwrap_or_else(|_| "todo.txt".to_string()));

    let mut lib = TodoLibrary::new(file_name.clone());
    let repo = Repo::find(&file_name, cli.no_sync, cli.vcs, cli.commit_changes);
    let repo = repo.as_ref();

    match cli.command {
        Commands::List { completed, filter } => {
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
                            Some((start, end)) => {
                                item.due.map_or(false, |d| d >= start && d <= end)
                            }
                        }
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
                            && date_filter.map_or(true, |(start, end)| {
                                item.due.map_or(false, |d| d >= start && d <= end)
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
                    for i in 0..items.len() {
                        if items[i].uuid == Some(uuid) && !items[i].done {
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

            if !yes {
                print!("Delete? [y/N] ");
                std::io::stdout().flush().ok();
                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer).ok();
                let answer = answer.trim().to_lowercase();
                if answer != "y" && answer != "yes" {
                    println!("Nothing deleted.");
                    return;
                }
            }

            if let Err(e) = lib.save() {
                eprintln!("Error saving file: {}", e);
                std::process::exit(1);
            }
            println!("Deleted {} item(s) from '{}'", removed.len() + 1, file_name);
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
    }
}
