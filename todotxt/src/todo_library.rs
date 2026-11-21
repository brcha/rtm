use crate::todo_context::TodoContext;
use crate::todo_item::TodoItem;
use crate::todo_project::TodoProject;
use crate::todo_recurrence::TodoRecurrenceUnit;
use chrono::{Datelike, Duration, Local, Months, NaiveDate, Weekday};
use std::collections::{BTreeMap, HashSet};
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct TodoLibrary {
    pub file_name: String,
    pub items: Vec<TodoItem>,
    final_newline: bool,
}

impl TodoLibrary {
    pub fn new(file_name: String) -> Self {
        TodoLibrary {
            file_name,
            items: Vec::new(),
            final_newline: true,
        }
    }

    pub fn load(&mut self) -> Result<(), std::io::Error> {
        refuse_symlink(Path::new(&self.file_name))?;
        let content = std::fs::read_to_string(&self.file_name)?;
        // `lines` also drops the `\r` of CRLF line endings, so they are saved as LF.
        self.items = content
            .lines()
            .filter_map(|line| line.parse::<TodoItem>().ok())
            .collect();
        self.final_newline = content.is_empty() || content.ends_with('\n');
        Ok(())
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let mut content = self
            .items
            .iter()
            .map(|item| item.to_string())
            .collect::<Vec<String>>()
            .join("\n");
        if self.final_newline && !self.items.is_empty() {
            content.push('\n');
        }
        write_atomic(Path::new(&self.file_name), content)
    }

    pub fn add_item(&mut self, mut item: TodoItem) {
        // A done item can carry a creation date only with a completion date, and the date
        // cannot be invented later, so items get one here, where they enter the library.
        if item.creation_date.is_none() {
            item.creation_date = Some(Local::now().date_naive());
        }
        self.items.push(item);
    }

    /// Removes the item at `index` and, recursively, every item whose `sub` is the uuid of a
    /// removed item. Returns the removed items in file order, or `None` if `index` is out of
    /// bounds.
    pub fn remove_item(&mut self, index: usize) -> Option<Vec<TodoItem>> {
        if index >= self.items.len() {
            return None;
        }
        let mut marked = vec![false; self.items.len()];
        marked[index] = true;
        let mut visited = HashSet::new();
        let mut pending: Vec<Uuid> = self.items[index].uuid.into_iter().collect();
        // Items are marked once, so cyclic links terminate; `visited` skips rescans of duplicated uuids.
        while let Some(parent) = pending.pop() {
            if !visited.insert(parent) {
                continue;
            }
            for (i, item) in self.items.iter().enumerate() {
                if !marked[i] && item.sub == Some(parent) {
                    marked[i] = true;
                    pending.extend(item.uuid);
                }
            }
        }
        let (removed, kept) = std::mem::take(&mut self.items)
            .into_iter()
            .zip(marked)
            .partition::<Vec<_>, _>(|(_, m)| *m);
        self.items = kept.into_iter().map(|(item, _)| item).collect();
        Some(removed.into_iter().map(|(item, _)| item).collect())
    }

    /// The file `archive` moves done items to, next to the todo file: `done.txt` for
    /// `todo.txt` (the todo.sh convention), `<stem>.done.<ext>` for any other file, so two todo
    /// files in one directory never share one.
    pub fn archive_path(&self) -> PathBuf {
        let path = Path::new(&self.file_name);
        if path.file_name().is_some_and(|name| name == "todo.txt") {
            return path.with_file_name("done.txt");
        }
        let mut name = OsString::from(path.file_stem().unwrap_or_default());
        name.push(".done");
        if let Some(extension) = path.extension() {
            name.push(".");
            name.push(extension);
        }
        path.with_file_name(name)
    }

    /// Moves the done items to `archive_path()`, appending their lines in file order, and returns
    /// them. With `completed_before`, only items completed before that date move; a done item
    /// without a completion date counts as old. The archive is written before the items leave
    /// the library, so an interruption before the caller's `save` duplicates them rather than
    /// losing them; on an error the library is unchanged. The todo file itself is not saved.
    pub fn archive(
        &mut self,
        completed_before: Option<NaiveDate>,
    ) -> Result<Vec<TodoItem>, std::io::Error> {
        let archived = |item: &TodoItem| {
            item.done
                && match (completed_before, item.completion_date) {
                    (Some(cutoff), Some(completed)) => completed < cutoff,
                    _ => true,
                }
        };
        if !self.items.iter().any(archived) {
            return Ok(Vec::new());
        }
        let mut text: String = self
            .items
            .iter()
            .filter(|item| archived(item))
            .map(|item| format!("{item}\n"))
            .collect();
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(self.archive_path())?;
        if file.metadata()?.len() > 0 {
            let mut last = [0u8];
            file.seek(SeekFrom::End(-1))?;
            file.read_exact(&mut last)?;
            if last[0] != b'\n' {
                text.insert(0, '\n');
            }
        }
        file.write_all(text.as_bytes())?;
        let (moved, kept) = std::mem::take(&mut self.items)
            .into_iter()
            .partition(|item| archived(item));
        self.items = kept;
        Ok(moved)
    }

    pub fn list_items(&self) -> &Vec<TodoItem> {
        &self.items
    }

    pub fn clear_items(&mut self) {
        self.items.clear();
    }
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
    pub fn complete_item(&mut self, index: usize) -> Option<bool> {
        if index >= self.items.len() {
            return None;
        }
        // Completing a done item keeps its completion date.
        if !self.items[index].done {
            self.items[index].completion_date = Some(Local::now().date_naive());
        }
        self.items[index].done = true;
        let has_recurrence = self.items[index].recurrence.is_some();
        if let Some(ref recurrence) = self.items[index].recurrence {
            let today = Local::now().date_naive();
            let advance = |from: NaiveDate| match recurrence.unit {
                TodoRecurrenceUnit::Daily => from + Duration::days(recurrence.count as i64),
                TodoRecurrenceUnit::BusinessDay => add_business_days(from, recurrence.count),
                TodoRecurrenceUnit::Weekly => from + Duration::weeks(recurrence.count as i64),
                // Calendar months and years: a day the target month lacks becomes its last day.
                TodoRecurrenceUnit::Monthly => from
                    .checked_add_months(Months::new(u32::from(recurrence.count)))
                    .expect("a u16 count of months stays within chrono's date range"),
                TodoRecurrenceUnit::Yearly => from
                    .checked_add_months(Months::new(u32::from(recurrence.count) * 12))
                    .expect("a u16 count of years stays within chrono's date range"),
            };
            // Strict recurrence counts from the old date, normal recurrence from today.
            let base = |old: NaiveDate| if recurrence.strict { old } else { today };
            let (new_due, new_threshold) =
                match (self.items[index].due, self.items[index].threshold) {
                    // The threshold date is advanced from a base shifted as the due date's base
                    // is: by the interval for strict recurrence, keeping its distance to the new
                    // due date for normal recurrence.
                    (Some(due), threshold) => (
                        Some(advance(base(due))),
                        threshold.map(|threshold| advance(threshold + (base(due) - due))),
                    ),
                    // Only a threshold date: it moves, and no due date is added.
                    (None, Some(threshold)) => (None, Some(advance(base(threshold)))),
                    // Neither: due the interval from today, strict too (there is no old date).
                    (None, None) => (Some(advance(today)), None),
                };
            // A copy of the completed item, so the next occurrence keeps its line's layout.
            let new_item = TodoItem {
                done: false,
                completion_date: None,
                creation_date: Some(today),
                due: new_due,
                threshold: new_threshold,
                uuid: None,
                sub: None,
                ..self.items[index].clone()
            };
            self.items.push(new_item);
        }
        // A done line reads `pri:X` as the priority: take the item as its saved line reads back.
        if let Ok(item) = self.items[index].to_string().parse() {
            self.items[index] = item;
        }
        Some(has_recurrence)
    }

    pub fn uncomplete_item(&mut self, index: usize) -> Option<()> {
        if index >= self.items.len() {
            return None;
        }
        self.items[index].done = false;
        self.items[index].completion_date = None;
        // The priority stays; `Display` writes it as `(X)` again on an open line.
        Some(())
    }

    /// Puts `item` in place of the item at `index`, keeping its position, and returns the old
    /// item; None = out of bounds. The new item is stored as given: no date is stamped.
    pub fn replace_item(&mut self, index: usize, item: TodoItem) -> Option<TodoItem> {
        let slot = self.items.get_mut(index)?;
        Some(std::mem::replace(slot, item))
    }

    /// Every project in the file with the number of open items that carry it. Names are compared
    /// case-insensitively and spelled as first seen in file order; the list is sorted by the
    /// lowercased name. A project found only on done items is listed with 0, and an item that
    /// carries a project twice counts once.
    pub fn projects(&self) -> Vec<(TodoProject, usize)> {
        tally(
            self.items
                .iter()
                .map(|item| (item.done, item.projects.iter().map(|p| p.name.as_str()))),
        )
        .into_iter()
        .map(|(name, count)| (TodoProject { name }, count))
        .collect()
    }

    /// Every context in the file with the number of open items that carry it, with the same
    /// rules as `projects`.
    pub fn contexts(&self) -> Vec<(TodoContext, usize)> {
        tally(
            self.items
                .iter()
                .map(|item| (item.done, item.contexts.iter().map(|c| c.name.as_str()))),
        )
        .into_iter()
        .map(|(name, count)| (TodoContext { name }, count))
        .collect()
    }

    /// The file indices, in file order, of the items that carry every one of `projects` and
    /// `contexts` (`TodoItem::matches`), done items included.
    pub fn filter_items(&self, projects: &[TodoProject], contexts: &[TodoContext]) -> Vec<usize> {
        (0..self.items.len())
            .filter(|&i| self.items[i].matches(projects, contexts))
            .collect()
    }
}

/// A todo file that is a symbolic link, refused because files kept next to the todo file (the
/// archive) would land next to the link rather than the file.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("'{}' is a symbolic link to '{}'; open '{}' instead", link.display(), target.display(), target.display())]
pub struct SymlinkError {
    pub link: PathBuf,
    /// The file at the end of the link chain, or the path the link names when it does not exist.
    pub target: PathBuf,
}

/// Refuses `path` when it is a symbolic link, with an `InvalidInput` error carrying a
/// `SymlinkError`. Links among its parent directories are allowed: they change the path's
/// spelling, not the directory the file is in. A missing path passes.
pub fn refuse_symlink(path: &Path) -> std::io::Result<()> {
    match link_target(path)? {
        None => Ok(()),
        Some(target) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            SymlinkError {
                link: path.to_path_buf(),
                target,
            },
        )),
    }
}

/// The file a symbolic link at `path` leads to: the end of the link chain, or for a dangling
/// link the path it names. `None` when `path` is missing or not a link.
fn link_target(path: &Path) -> std::io::Result<Option<PathBuf>> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if !metadata.file_type().is_symlink() {
        return Ok(None);
    }
    Ok(Some(match std::fs::canonicalize(path) {
        Ok(target) => target,
        Err(_) => {
            let named = std::fs::read_link(path)?;
            path.parent().unwrap_or(Path::new("")).join(named)
        }
    }))
}

/// Writes a temporary file next to `path` and renames it over the original, keeping the
/// original's permissions. A symbolic link stays a link: the file it leads to is replaced. A
/// read-only file is refused with `PermissionDenied`, as writing it in place would be.
pub fn write_atomic(path: &Path, contents: impl AsRef<[u8]>) -> std::io::Result<()> {
    let target = link_target(path)?.unwrap_or_else(|| path.to_path_buf());
    let Some(name) = target.file_name() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the path does not name a file",
        ));
    };
    let permissions = match std::fs::metadata(&target) {
        Ok(metadata) if metadata.permissions().readonly() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "the file is read-only",
            ));
        }
        Ok(metadata) => Some(metadata.permissions()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    // Hidden, and never one of the names rtm commits, should a crash leave it behind.
    let mut temp_name = OsString::from(".");
    temp_name.push(name);
    temp_name.push(format!(".{}.tmp", Uuid::new_v4()));
    let temp = target.with_file_name(temp_name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let contents = contents.as_ref();
    let result = (|| {
        if let Some(permissions) = permissions {
            file.set_permissions(permissions)?;
        }
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, &target)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

/// The date `count` business days after `from`, skipping Saturdays and Sundays as topydo and
/// SimpleTask do (no holidays): each step lands on a weekday, so Saturday + 1 is Monday.
fn add_business_days(from: NaiveDate, count: u16) -> NaiveDate {
    let mut date = from;
    for _ in 0..count {
        date += Duration::days(1);
        while matches!(date.weekday(), Weekday::Sat | Weekday::Sun) {
            date += Duration::days(1);
        }
    }
    date
}

/// The listing behind `projects` and `contexts`, from each item's `done` and tag names. Tags
/// found only on done items are kept with 0, so done items stay reachable through the filter.
fn tally<'a, N>(items: impl Iterator<Item = (bool, N)>) -> Vec<(String, usize)>
where
    N: Iterator<Item = &'a str>,
{
    let mut tags: BTreeMap<String, (String, usize)> = BTreeMap::new();
    for (done, names) in items {
        // An item that carries a tag twice (`+a +A`) counts once.
        let mut counted = HashSet::new();
        for name in names {
            let key = name.to_lowercase();
            let entry = tags
                .entry(key.clone())
                .or_insert_with(|| (name.to_string(), 0));
            if !done && counted.insert(key) {
                entry.1 += 1;
            }
        }
    }
    tags.into_values().collect()
}

#[cfg(test)]
#[path = "tests/todo_library_file_tests.rs"]
mod file_tests;

#[cfg(test)]
#[path = "tests/todo_library_completion_tests.rs"]
mod completion_tests;

#[cfg(test)]
#[path = "tests/todo_library_items_tests.rs"]
mod items_tests;

#[cfg(test)]
#[path = "tests/todo_library_tags_tests.rs"]
mod tags_tests;

#[cfg(test)]
#[path = "tests/todo_library_archive_tests.rs"]
mod archive_tests;

#[cfg(test)]
#[path = "tests/todo_library_atomic_write_tests.rs"]
mod atomic_write_tests;
