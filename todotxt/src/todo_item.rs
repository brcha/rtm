use crate::todo_context::TodoContext;
use crate::todo_priority::TodoPriority;
use crate::todo_project::TodoProject;
use crate::todo_recurrence::TodoRecurrence;
use chrono::NaiveDate;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use crate::todo_context::TodoContextParseError;
use crate::todo_priority::TodoPriorityParseError;
use crate::todo_project::TodoProjectParseError;
use crate::todo_recurrence::TodoRecurrenceParseError;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct TodoItem {
    pub done: bool,
    pub priority: TodoPriority,
    pub completion_date: Option<NaiveDate>,
    pub creation_date: Option<NaiveDate>, // must exist if completion date is set
    pub description: String,
    pub projects: Vec<TodoProject>,
    pub contexts: Vec<TodoContext>,
    pub due: Option<NaiveDate>,
    pub recurrence: Option<TodoRecurrence>,
    pub threshold: Option<NaiveDate>,
    pub uuid: Option<Uuid>,
    pub sub: Option<Uuid>,
    /// The line the item was read from, so `Display` writes back what did not change; `None`
    /// for an item built in code.
    pub layout: Option<Layout>,
}

/// Equality compares what the item says, not how its line was spelled: `layout` is left out.
/// A new field must be added here too.
impl PartialEq for TodoItem {
    fn eq(&self, other: &Self) -> bool {
        self.done == other.done
            && self.priority == other.priority
            && self.completion_date == other.completion_date
            && self.creation_date == other.creation_date
            && self.description == other.description
            && self.projects == other.projects
            && self.contexts == other.contexts
            && self.due == other.due
            && self.recurrence == other.recurrence
            && self.threshold == other.threshold
            && self.uuid == other.uuid
            && self.sub == other.sub
    }
}

impl Eq for TodoItem {}

/// The layout of a line as read: its text, where each word stands and what it set, and the
/// fields as read. Opaque outside this crate, which can only write `layout: None`.
#[derive(Clone, Debug)]
pub struct Layout {
    text: String,
    /// Where the head (`x`, `(A)`, the completion and creation dates) ends; 0 without one.
    head_end: usize,
    /// A done line with its priority as `(A)` between the `x` and the dates, as some clients
    /// write it: it is written back in the specification's form, with the priority as `pri:A`.
    priority_in_head: bool,
    /// The words after the head, in order.
    tokens: Vec<Token>,
    /// The fields as read, with no layout of their own.
    read: Box<TodoItem>,
}

#[derive(Clone, Debug)]
struct Token {
    start: usize,
    end: usize,
    kind: Kind,
}

/// What a word after the head set. `Pri` is a `pri:` tag that set the priority; one that did
/// not, like any unreadable tag, is a description `Word`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Word,
    Project,
    Context,
    Due,
    Recurrence,
    Threshold,
    Uuid,
    Sub,
    Pri,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TodoItemParseError {
    #[error("invalid uuid")]
    Uuid(#[from] uuid::Error),
    #[error("invalid date")]
    Date(#[from] chrono::ParseError),
    #[error("invalid priority")]
    Priority(#[from] TodoPriorityParseError),
    #[error("invalid project")]
    Project(#[from] TodoProjectParseError),
    #[error("invalid context")]
    Context(#[from] TodoContextParseError),
    #[error("invalid recurrence")]
    Recurrence(#[from] TodoRecurrenceParseError),
    #[error("conflicting priorities: ({0}) and pri:{1}")]
    ConflictingPriority(char, char),
}

impl FromStr for TodoItem {
    type Err = TodoItemParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Each word with its byte offset in the line, for the layout.
        let words: Vec<(usize, &str)> = s
            .split_whitespace()
            .map(|word| (word.as_ptr() as usize - s.as_ptr() as usize, word))
            .collect();
        let parts: Vec<&str> = words.iter().map(|&(_, word)| word).collect();
        let mut index = 0;

        let mut done = false;
        if parts.get(index) == Some(&"x") {
            done = true;
            index += 1;
        }

        // A `(X)` whose letter is not A-Z (`(b)`, `(1)`) is not a priority: the word starts
        // the description.
        let mut priority = TodoPriority { priority: None };
        if let Some(prio_str) = parts.get(index)
            && prio_str.starts_with('(')
            && prio_str.ends_with(')')
            && prio_str.len() == 3
            && let Ok(prio) = TodoPriority::from_str(prio_str)
        {
            priority = prio;
            index += 1;
        }
        // Whether the leading `(X)` form was seen, to detect a conflict with a `pri:Y` tag.
        let priority_from_parens = priority.priority.is_some();

        let mut completion_date = None;
        let mut creation_date = None;

        if done {
            if let Some(date_str) = parts.get(index)
                && let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            {
                completion_date = Some(date);
                index += 1;
            }
            if let Some(date_str) = parts.get(index)
                && let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            {
                creation_date = Some(date);
                index += 1;
            }
        } else if let Some(date_str) = parts.get(index)
            && let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        {
            creation_date = Some(date);
            index += 1;
        }

        // The rest is description with embedded elements
        let head_end = match index {
            0 => 0,
            n => words[n - 1].0 + words[n - 1].1.len(),
        };
        let mut tokens = Vec::with_capacity(words.len() - index);
        let mut projects = vec![];
        let mut contexts = vec![];
        let mut due = None;
        let mut recurrence = None;
        let mut threshold = None;
        let mut uuid: Option<Uuid> = None;
        let mut sub: Option<Uuid> = None;
        let mut clean_description_parts = vec![];

        // A known tag whose value cannot be read (`due:soon`, `rec:often`, `uuid:nope`) does
        // not match its branch and falls through to plain description text, so every line
        // parses and TodoLibrary::load never drops one.
        for &(start, word) in &words[index..] {
            let kind = if word.starts_with('+') && word.len() > 1 {
                let project = TodoProject::from_str(word)?;
                projects.push(project);
                Kind::Project
            } else if word.starts_with('@') && word.len() > 1 {
                let context = TodoContext::from_str(word)?;
                contexts.push(context);
                Kind::Context
            } else if word.starts_with("due:")
                && word.len() > 5
                && let Ok(date) = chrono::NaiveDate::parse_from_str(&word[4..], "%Y-%m-%d")
            {
                due = Some(date);
                Kind::Due
            } else if word.starts_with("rec:")
                && word.len() > 4
                && let Ok(rec) = TodoRecurrence::from_str(&word[4..])
            {
                recurrence = Some(rec);
                Kind::Recurrence
            } else if word.starts_with("t:")
                && word.len() > 2
                && let Ok(date) = chrono::NaiveDate::parse_from_str(&word[2..], "%Y-%m-%d")
            {
                threshold = Some(date);
                Kind::Threshold
            } else if word.starts_with("uuid:")
                && word.len() > 5
                && let Ok(parsed_uuid) = Uuid::parse_str(&word[5..])
            {
                uuid = Some(parsed_uuid);
                Kind::Uuid
            } else if word.starts_with("sub:")
                && word.len() > 4
                && let Ok(parsed_sub) = Uuid::parse_str(&word[4..])
            {
                sub = Some(parsed_sub);
                Kind::Sub
            } else if word.starts_with("pri:")
                && word.len() > 4
                && let Ok(parsed) = TodoPriority::from_tag_value(&word[4..])
                // A `pri:` that disagrees with the leading `(X)` stays text; `(X)` keeps
                // the priority.
                && (!priority_from_parens || priority.priority == parsed.priority)
            {
                priority = parsed;
                Kind::Pri
            } else {
                clean_description_parts.push(word.to_string());
                Kind::Word
            };
            tokens.push(Token {
                start,
                end: start + word.len(),
                kind,
            });
        }

        let description = clean_description_parts.join(" ");

        let read = TodoItem {
            done,
            priority,
            completion_date,
            creation_date,
            description,
            projects,
            contexts,
            due,
            recurrence,
            threshold,
            uuid,
            sub,
            layout: None,
        };
        Ok(TodoItem {
            layout: Some(Layout {
                text: s.to_owned(),
                head_end,
                priority_in_head: done && priority_from_parens,
                tokens,
                read: Box::new(read.clone()),
            }),
            ..read
        })
    }
}

impl Display for TodoItem {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match &self.layout {
            // A line rtm did not change is written as it was read.
            Some(layout) if *self == *layout.read && !layout.priority_in_head => {
                write!(f, "{}", layout.text)
            }
            // A line with no words has no place to patch.
            Some(layout) if layout.head_end > 0 || !layout.tokens.is_empty() => {
                write!(f, "{}", self.patched(layout))
            }
            _ => write!(f, "{}", self.rebuilt()),
        }
    }
}

impl TodoItem {
    /// The head of the line: `x` and the dates for a done item, `(A)` and the creation date for
    /// an open one.
    fn head(&self) -> Vec<String> {
        let mut parts: Vec<String> = Vec::new();

        if self.done {
            parts.push("x".to_string());
        } else if self.priority.priority.is_some() {
            parts.push(self.priority.to_string());
        }

        if self.done {
            // The creation date is only ever emitted alongside a completion date, in that
            // order. Per the todo.txt spec a done line's completion date comes first and
            // its (optional) creation date must follow it; a lone date on a done line would
            // otherwise be re-read as the completion date on the next parse, silently
            // discarding the creation date it actually was.
            if let Some(cd) = self.completion_date {
                parts.push(cd.format("%Y-%m-%d").to_string());
                if let Some(cd) = self.creation_date {
                    parts.push(cd.format("%Y-%m-%d").to_string());
                }
            }
        } else if let Some(cd) = self.creation_date {
            parts.push(cd.format("%Y-%m-%d").to_string());
        }
        parts
    }

    /// The `pri:` tag that carries a done item's priority; an open item's is in the head.
    fn pri_tag(&self) -> Option<String> {
        match self.priority.priority {
            Some(p) if self.done => Some(format!("pri:{}", (p + b'A') as char)),
            _ => None,
        }
    }

    /// The line built from the fields alone, for an item built in code: the head, the
    /// description, then the tags in a fixed order.
    fn rebuilt(&self) -> String {
        // Built as tokens and joined with a single space, rather than interleaving
        // conditional leading/trailing spaces, so an absent field (e.g. an empty
        // description on a done item) can never produce a doubled-up space.
        let mut parts = self.head();

        if !self.description.is_empty() {
            parts.push(self.description.clone());
        }

        // A completed item's priority is written as the pri: tag rather than the
        // leading (X) form, which is reserved for open items.
        parts.extend(self.pri_tag());
        parts.extend(self.projects.iter().map(ToString::to_string));
        parts.extend(self.contexts.iter().map(ToString::to_string));
        parts.extend(self.due_tag());
        parts.extend(self.recurrence_tag());
        parts.extend(self.threshold_tag());
        parts.extend(self.uuid_tag());
        parts.extend(self.sub_tag());
        parts.join(" ")
    }

    fn due_tag(&self) -> Option<String> {
        self.due.map(|d| format!("due:{}", d.format("%Y-%m-%d")))
    }

    fn recurrence_tag(&self) -> Option<String> {
        self.recurrence.as_ref().map(|r| format!("rec:{r}"))
    }

    fn threshold_tag(&self) -> Option<String> {
        self.threshold
            .map(|t| format!("t:{}", t.format("%Y-%m-%d")))
    }

    fn uuid_tag(&self) -> Option<String> {
        self.uuid.map(|u| format!("uuid:{u}"))
    }

    fn sub_tag(&self) -> Option<String> {
        self.sub.map(|s| format!("sub:{s}"))
    }

    /// The line as read, changed only where a field differs from what was read: a new head
    /// for completion, priority and dates; a changed tag in place; a removed tag with the
    /// whitespace before it; a new tag at the end of the line; for a changed description, only
    /// the words between the part it shares with the old one at the start and at the end.
    fn patched(&self, layout: &Layout) -> String {
        let read = &*layout.read;
        let text = layout.text.as_str();
        let mut edits = Edits::new(text);
        let of = |kind: Kind| layout.tokens.iter().filter(move |t| t.kind == kind);

        let priority_changed =
            self.done != read.done || self.priority != read.priority || layout.priority_in_head;
        let head_changed = priority_changed
            || self.completion_date != read.completion_date
            || self.creation_date != read.creation_date;
        if head_changed {
            let head = self.head().join(" ");
            match (layout.head_end, head.is_empty()) {
                (0, false) => edits.replace(0, 0, format!("{head} ")),
                (end, _) => edits.replace(0, end, head),
            }
        }

        // Description words: keep the common start and end, replace the middle.
        let old: Vec<&Token> = of(Kind::Word).collect();
        let old_words: Vec<&str> = old.iter().map(|t| &text[t.start..t.end]).collect();
        let new_words: Vec<&str> = self.description.split_whitespace().collect();
        if old_words != new_words {
            let prefix = old_words
                .iter()
                .zip(&new_words)
                .take_while(|(a, b)| a == b)
                .count();
            let suffix = old_words[prefix..]
                .iter()
                .rev()
                .zip(new_words[prefix..].iter().rev())
                .take_while(|(a, b)| a == b)
                .count();
            let old_middle = &old[prefix..old.len() - suffix];
            let new_middle = new_words[prefix..new_words.len() - suffix].join(" ");
            match old_middle.split_first() {
                Some((first, rest)) => {
                    if new_middle.is_empty() {
                        edits.remove(first);
                    } else {
                        edits.replace(first.start, first.end, new_middle);
                    }
                    rest.iter().for_each(|t| edits.remove(t));
                }
                None if prefix > 0 => {
                    let at = old[prefix - 1].end;
                    edits.replace(at, at, format!(" {new_middle}"));
                }
                None if suffix > 0 => {
                    let at = old[prefix].start;
                    edits.replace(at, at, format!("{new_middle} "));
                }
                None => match layout.tokens.first() {
                    // shortcut: when the first tag is removed in the same edit, the space
                    // written here doubles the one left after it; patch it if a frontend ever
                    // saves both changes at once on a line without a head or description.
                    Some(first) if layout.head_end == 0 => {
                        edits.replace(first.start, first.start, format!("{new_middle} "))
                    }
                    _ => edits.replace(layout.head_end, layout.head_end, format!(" {new_middle}")),
                },
            }
        }

        let mut appended: Vec<String> = Vec::new();
        if priority_changed {
            of(Kind::Pri).for_each(|t| edits.remove(t));
            appended.extend(self.pri_tag());
        }
        appended.extend(tags(
            &mut edits,
            of(Kind::Project),
            &read.projects,
            &self.projects,
        ));
        appended.extend(tags(
            &mut edits,
            of(Kind::Context),
            &read.contexts,
            &self.contexts,
        ));
        for (kind, changed, tag) in [
            (Kind::Due, self.due != read.due, self.due_tag()),
            (
                Kind::Recurrence,
                self.recurrence != read.recurrence,
                self.recurrence_tag(),
            ),
            (
                Kind::Threshold,
                self.threshold != read.threshold,
                self.threshold_tag(),
            ),
            (Kind::Uuid, self.uuid != read.uuid, self.uuid_tag()),
            (Kind::Sub, self.sub != read.sub, self.sub_tag()),
        ] {
            if !changed {
                continue;
            }
            let tokens: Vec<&Token> = of(kind).collect();
            match (tag, tokens.last()) {
                // The last one is the one that set the field.
                (Some(tag), Some(last)) => edits.replace(last.start, last.end, tag),
                (Some(tag), None) => appended.push(tag),
                (None, _) => tokens.iter().for_each(|t| edits.remove(t)),
            }
        }
        if !appended.is_empty() {
            let at = layout.tokens.last().map_or(layout.head_end, |t| t.end);
            edits.replace(at, at, format!(" {}", appended.join(" ")));
        }

        let line = edits.apply();
        // A removed head or first word leaves the whitespace after it at the start of the line.
        if text.starts_with(char::is_whitespace) {
            line
        } else {
            line.trim_start().to_owned()
        }
    }
}

/// Keeps the tag tokens whose tag is still listed (matched in order, so a repeated tag stays as
/// often as it is still listed), removes the others and returns the new tags to append.
fn tags<'a, T: PartialEq + Display>(
    edits: &mut Edits,
    tokens: impl Iterator<Item = &'a Token>,
    read: &[T],
    current: &[T],
) -> Vec<String> {
    let mut unmatched: Vec<&T> = current.iter().collect();
    for (token, tag) in tokens.zip(read) {
        match unmatched.iter().position(|t| *t == tag) {
            Some(i) => {
                unmatched.remove(i);
            }
            None => edits.remove(token),
        }
    }
    unmatched.iter().map(ToString::to_string).collect()
}

/// Replacements of byte ranges of a line, applied together.
struct Edits<'a> {
    text: &'a str,
    edits: Vec<(usize, usize, String)>,
}

impl<'a> Edits<'a> {
    fn new(text: &'a str) -> Self {
        Edits {
            text,
            edits: Vec::new(),
        }
    }

    fn replace(&mut self, start: usize, end: usize, with: String) {
        self.edits.push((start, end, with));
    }

    /// Removes a word with the whitespace before it. Removals of neighbouring words never
    /// overlap; whitespace left at the start of the line is trimmed by `patched`.
    fn remove(&mut self, token: &Token) {
        let start = self.text[..token.start].trim_end().len();
        self.edits.push((start, token.end, String::new()));
    }

    fn apply(mut self) -> String {
        // From the end of the line, so earlier offsets stay valid; at one offset, a removal
        // before an insertion.
        self.edits
            .sort_by_key(|&(start, end, _)| std::cmp::Reverse((start, end)));
        let mut line = self.text.to_owned();
        for (start, end, with) in self.edits {
            line.replace_range(start..end, &with);
        }
        line
    }
}

impl TodoItem {
    pub fn add_subtask(&self, child: &TodoItem) -> (Option<TodoItem>, TodoItem) {
        let new_uuid = if let Some(existing_uuid) = self.uuid {
            existing_uuid
        } else {
            Uuid::new_v4()
        };
        let new_parent = if self.uuid.is_none() {
            let mut p = self.clone();
            p.uuid = Some(new_uuid);
            Some(p)
        } else {
            None
        };
        let mut new_child = child.clone();
        new_child.sub = Some(new_uuid);
        (new_parent, new_child)
    }

    pub fn set_done(&self, done: bool) -> TodoItem {
        TodoItem {
            done,
            ..self.clone()
        }
    }

    pub fn set_priority(&self, priority: TodoPriority) -> TodoItem {
        TodoItem {
            priority,
            ..self.clone()
        }
    }

    pub fn set_completion_date(&self, completion_date: Option<NaiveDate>) -> TodoItem {
        TodoItem {
            completion_date,
            ..self.clone()
        }
    }

    pub fn set_creation_date(&self, creation_date: Option<NaiveDate>) -> TodoItem {
        TodoItem {
            creation_date,
            ..self.clone()
        }
    }

    pub fn set_description(&self, description: String) -> TodoItem {
        TodoItem {
            description,
            ..self.clone()
        }
    }

    pub fn set_projects(&self, projects: Vec<TodoProject>) -> TodoItem {
        TodoItem {
            projects,
            ..self.clone()
        }
    }

    pub fn add_project(&self, project: TodoProject) -> TodoItem {
        let mut new_projects = self.projects.clone();
        new_projects.push(project);
        TodoItem {
            projects: new_projects,
            ..self.clone()
        }
    }

    pub fn remove_project(&self, project: &TodoProject) -> Option<TodoItem> {
        if let Some(pos) = self.projects.iter().position(|p| p == project) {
            let mut new_projects = self.projects.clone();
            new_projects.remove(pos);
            Some(TodoItem {
                projects: new_projects,
                ..self.clone()
            })
        } else {
            None
        }
    }

    pub fn set_contexts(&self, contexts: Vec<TodoContext>) -> TodoItem {
        TodoItem {
            contexts,
            ..self.clone()
        }
    }

    pub fn add_context(&self, context: TodoContext) -> TodoItem {
        let mut new_contexts = self.contexts.clone();
        new_contexts.push(context);
        TodoItem {
            contexts: new_contexts,
            ..self.clone()
        }
    }

    pub fn remove_context(&self, context: &TodoContext) -> Option<TodoItem> {
        if let Some(pos) = self.contexts.iter().position(|c| c == context) {
            let mut new_contexts = self.contexts.clone();
            new_contexts.remove(pos);
            Some(TodoItem {
                contexts: new_contexts,
                ..self.clone()
            })
        } else {
            None
        }
    }

    pub fn set_due(&self, due: Option<NaiveDate>) -> TodoItem {
        TodoItem {
            due,
            ..self.clone()
        }
    }

    pub fn set_recurrence(&self, recurrence: Option<TodoRecurrence>) -> TodoItem {
        TodoItem {
            recurrence,
            ..self.clone()
        }
    }

    pub fn set_threshold(&self, threshold: Option<NaiveDate>) -> TodoItem {
        TodoItem {
            threshold,
            ..self.clone()
        }
    }

    /// Whether the item carries every one of `projects` and every one of `contexts`, names
    /// compared case-insensitively (`+Work` matches `+work`). Empty lists match every item.
    pub fn matches(&self, projects: &[TodoProject], contexts: &[TodoContext]) -> bool {
        projects.iter().all(|wanted| {
            self.projects
                .iter()
                .any(|p| same_name(&p.name, &wanted.name))
        }) && contexts.iter().all(|wanted| {
            self.contexts
                .iter()
                .any(|c| same_name(&c.name, &wanted.name))
        })
    }
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

#[cfg(test)]
#[path = "tests/todo_item_parsing_tests.rs"]
mod parsing_tests;

#[cfg(test)]
#[path = "tests/todo_item_display_tests.rs"]
mod display_tests;

#[cfg(test)]
#[path = "tests/todo_item_editing_tests.rs"]
mod editing_tests;

#[cfg(test)]
#[path = "tests/todo_item_filter_tests.rs"]
mod filter_tests;

#[cfg(test)]
#[path = "tests/todo_item_layout_tests.rs"]
mod layout_tests;
