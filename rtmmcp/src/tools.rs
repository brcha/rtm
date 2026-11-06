//! What the tools do, synchronously, on one of the allowed files. `Ok` is the text the client
//! reads; `Err` is a failure (shown with `isError`), and a write that fails changes nothing.
//!
//! Every call loads the file from disk; nothing is kept between calls, since other clients, rtmapp
//! and rtmcli may change it meanwhile. Items are numbered by their position in the file, from 1,
//! and a change names its item by that number and its line as listed: when the line at the number
//! differs (the file changed), the one item with that line is used, else nothing changes.

use chrono::{Duration, Local, NaiveDate};
use todotxt::{TodoContext, TodoItem, TodoLibrary, TodoProject, WriteLock};

use crate::Source;
use crate::sync::RepoWrite;

/// Which items `list_items` shows by completion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Status {
    #[default]
    Open,
    Done,
    All,
}

/// Which items `list_items` shows by due date, as rtmcli's filters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Due {
    #[default]
    All,
    Today,
    Week,
    Overdue,
    Future,
}

/// The filters of `list_items`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Filter {
    pub(crate) status: Status,
    pub(crate) due: Due,
    pub(crate) projects: Vec<String>,
    pub(crate) contexts: Vec<String>,
    pub(crate) text: Option<String>,
}

/// The allowed file the client asked for by name or path, else the first.
pub(crate) fn pick(sources: Vec<Source>, file: Option<&str>) -> Result<Source, String> {
    let names = || {
        sources
            .iter()
            .map(Source::name)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let Some(file) = file else {
        return sources
            .first()
            .cloned()
            .ok_or_else(|| "No todo.txt files are configured.".to_owned());
    };
    sources
        .iter()
        .find(|source| source.name() == file || source.path.to_string_lossy() == file)
        .cloned()
        .ok_or_else(|| {
            format!(
                "'{file}' is not one of the files this server can use: {}.",
                names()
            )
        })
}

pub(crate) fn list_items(source: &Source, filter: &Filter) -> Result<String, String> {
    let lib = load(source)?;
    let projects = tags::<TodoProject>(&filter.projects, '+')?;
    let contexts = tags::<TodoContext>(&filter.contexts, '@')?;
    let today = Local::now().date_naive();
    let range = match filter.due {
        Due::All => None,
        Due::Today => Some((today, today)),
        Due::Week => Some((today, today + Duration::days(6))),
        Due::Overdue => Some((NaiveDate::MIN, today - Duration::days(1))),
        Due::Future => Some((today + Duration::days(1), NaiveDate::MAX)),
    };
    let text = filter.text.as_deref().map(str::to_lowercase);
    let lines: Vec<String> = lib
        .filter_items(&projects, &contexts)
        .into_iter()
        .filter_map(|i| {
            let item = &lib.items[i];
            let line = item.to_string();
            let shown = match filter.status {
                Status::Open => !item.done,
                Status::Done => item.done,
                Status::All => true,
            } && range
                .is_none_or(|(start, end)| item.due.is_some_and(|d| d >= start && d <= end))
                && text
                    .as_deref()
                    .is_none_or(|text| line.to_lowercase().contains(text));
            shown.then(|| numbered(i, &line))
        })
        .collect();
    if lines.is_empty() {
        return Ok("No matching items.".to_owned());
    }
    Ok(lines.join("\n"))
}

pub(crate) fn list_projects_and_contexts(source: &Source) -> Result<String, String> {
    let lib = load(source)?;
    let mut lines: Vec<String> = lib
        .projects()
        .into_iter()
        .map(|(project, open)| format!("{project} ({open} open)"))
        .collect();
    lines.extend(
        lib.contexts()
            .into_iter()
            .map(|(context, open)| format!("{context} ({open} open)")),
    );
    if lines.is_empty() {
        return Ok("No projects or contexts.".to_owned());
    }
    Ok(lines.join("\n"))
}

pub(crate) fn add_item(source: &Source, line: &str) -> Result<String, String> {
    let item = parse(line)?;
    write(source, true, |lib| {
        let message = format!("rtm: add {}", item.description);
        lib.add_item(item);
        let i = lib.items.len() - 1;
        Ok((message, format!("Added {}", numbered(i, &lib.items[i]))))
    })
}

pub(crate) fn complete_item(source: &Source, number: usize, text: &str) -> Result<String, String> {
    write(source, false, |lib| {
        let i = find(lib, number, text)?;
        if lib.items[i].done {
            return Err(format!("Item {} is already completed.", i + 1));
        }
        let message = format!("rtm: complete {}", lib.items[i]);
        let recurring = lib.complete_item(i) == Some(true);
        let mut result = format!("Completed {}", numbered(i, &lib.items[i]));
        if recurring {
            let next = lib.items.len() - 1;
            result.push_str(&format!("\nNext: {}", numbered(next, &lib.items[next])));
        }
        Ok((message, result))
    })
}

pub(crate) fn uncomplete_item(
    source: &Source,
    number: usize,
    text: &str,
) -> Result<String, String> {
    write(source, false, |lib| {
        let i = find(lib, number, text)?;
        if !lib.items[i].done {
            return Err(format!("Item {} is not completed.", i + 1));
        }
        let message = format!("rtm: uncomplete {}", lib.items[i]);
        lib.uncomplete_item(i);
        Ok((message, format!("Reopened {}", numbered(i, &lib.items[i]))))
    })
}

pub(crate) fn edit_item(
    source: &Source,
    number: usize,
    text: &str,
    new_line: &str,
) -> Result<String, String> {
    let mut new = parse(new_line)?;
    write(source, false, |lib| {
        let i = find(lib, number, text)?;
        let old = &lib.items[i];
        // Facts the new line leaves out are kept rather than lost: the creation date, and the
        // links that make an item a sub-item.
        new.creation_date = new.creation_date.or(old.creation_date);
        new.uuid = new.uuid.or(old.uuid);
        new.sub = new.sub.or(old.sub);
        let message = format!("rtm: edit {old}");
        lib.replace_item(i, new);
        Ok((message, format!("Changed {}", numbered(i, &lib.items[i]))))
    })
}

pub(crate) fn delete_item(source: &Source, number: usize, text: &str) -> Result<String, String> {
    write(source, false, |lib| {
        let i = find(lib, number, text)?;
        let message = format!("rtm: delete {}", lib.items[i]);
        let removed = lib.remove_item(i).unwrap_or_default();
        let lines: Vec<String> = removed.iter().map(TodoItem::to_string).collect();
        let result = if lines.len() == 1 {
            format!("Deleted: {}", lines[0])
        } else {
            format!(
                "Deleted {} items (the item and its sub-items):\n{}",
                lines.len(),
                lines.join("\n")
            )
        };
        Ok((message, result))
    })
}

/// One change: holds the write lock from before the repository check (another rtm process between
/// its save and its commit would look like an outside change) until after the push; pulls, loads,
/// applies `change`, saves, commits and pushes. `change` returns the commit message and the result,
/// or a refusal. With `may_create`, a missing file starts empty.
fn write(
    source: &Source,
    may_create: bool,
    change: impl FnOnce(&mut TodoLibrary) -> Result<(String, String), String>,
) -> Result<String, String> {
    // Everything up to the save refuses the change and leaves the file as it was.
    let refused = |e: String| format!("{e} Nothing was changed.");
    let _lock = WriteLock::acquire(&source.path)
        .map_err(|e| refused(format!("Cannot lock '{}': {e}.", source.path.display())))?;
    let mut repo = RepoWrite::find(source).map_err(refused)?;
    repo.pull().map_err(refused)?;
    let mut lib = if may_create && !source.path.exists() {
        TodoLibrary::new(source.path.to_string_lossy().into_owned())
    } else {
        load(source).map_err(refused)?
    };
    let (message, result) = change(&mut lib).map_err(refused)?;
    lib.save()
        .map_err(|e| format!("Cannot write '{}': {e}", source.path.display()))?;
    repo.commit_and_push(&message);
    Ok(std::iter::once(result)
        .chain(repo.notes)
        .collect::<Vec<_>>()
        .join("\n"))
}

fn load(source: &Source) -> Result<TodoLibrary, String> {
    let mut lib = TodoLibrary::new(source.path.to_string_lossy().into_owned());
    lib.load()
        .map_err(|e| format!("Cannot read '{}': {e}.", source.path.display()))?;
    Ok(lib)
}

/// One todo.txt line, as a new or changed item.
fn parse(line: &str) -> Result<TodoItem, String> {
    let line = line.trim();
    if line.is_empty() || line.contains('\n') {
        return Err("Give exactly one todo.txt line.".to_owned());
    }
    line.parse()
        .map_err(|e| format!("'{line}' is not a valid todo.txt line: {e}."))
}

/// The tags `names` (with or without the leading `sigil`).
fn tags<T: std::str::FromStr>(names: &[String], sigil: char) -> Result<Vec<T>, String> {
    names
        .iter()
        .map(|name| {
            let tag = format!("{sigil}{}", name.trim_start_matches(sigil));
            tag.parse()
                .map_err(|_| format!("'{name}' is not a valid tag."))
        })
        .collect()
}

/// The index of the item numbered `number` whose line is `text`; see the module comment.
fn find(lib: &TodoLibrary, number: usize, text: &str) -> Result<usize, String> {
    let text = text.trim();
    let items = &lib.items;
    if let Some(i) = number.checked_sub(1)
        && items.get(i).is_some_and(|item| item.to_string() == text)
    {
        return Ok(i);
    }
    let same: Vec<usize> = (0..items.len())
        .filter(|&i| items[i].to_string() == text)
        .collect();
    if let [i] = same[..] {
        return Ok(i);
    }
    let mut error = if same.is_empty() {
        format!("No item reads '{text}' any more; list the items again.")
    } else {
        format!("Several items read '{text}'; list the items again and use the number shown.")
    };
    if same.is_empty()
        && let Some(uuid) = text.parse::<TodoItem>().ok().and_then(|item| item.uuid)
        && let Some(i) = items.iter().position(|item| item.uuid == Some(uuid))
    {
        error.push_str(&format!(" It now reads: {}", numbered(i, &items[i])));
    }
    Err(error)
}

/// An item as listed: its number, from 1, and its line.
fn numbered(index: usize, item: impl std::fmt::Display) -> String {
    format!("{} {item}", index + 1)
}

#[cfg(test)]
#[path = "tests/tools_support_tests.rs"]
mod support_tests;

#[cfg(test)]
#[path = "tests/tools_listing_tests.rs"]
mod listing_tests;

#[cfg(test)]
#[path = "tests/tools_changes_tests.rs"]
mod changes_tests;
