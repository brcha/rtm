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

#[derive(Clone, Debug, Eq, PartialEq)]
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
        let parts: Vec<&str> = s.split_whitespace().collect();
        let mut index = 0;

        let mut done = false;
        if parts.get(index) == Some(&"x") {
            done = true;
            index += 1;
        }

        let mut priority = if let Some(prio_str) = parts.get(index) {
            if prio_str.starts_with('(') && prio_str.ends_with(')') && prio_str.len() == 3 {
                let prio = TodoPriority::from_str(prio_str)?;
                index += 1;
                prio
            } else {
                TodoPriority { priority: None }
            }
        } else {
            TodoPriority { priority: None }
        };
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
        let description_vec = &parts[index..];
        let mut projects = vec![];
        let mut contexts = vec![];
        let mut due = None;
        let mut recurrence = None;
        let mut threshold = None;
        let mut uuid: Option<Uuid> = None;
        let mut sub: Option<Uuid> = None;
        let mut clean_description_parts = vec![];

        for word in description_vec {
            if word.starts_with('+') && word.len() > 1 {
                let project = TodoProject::from_str(word)?;
                projects.push(project);
            } else if word.starts_with('@') && word.len() > 1 {
                let context = TodoContext::from_str(word)?;
                contexts.push(context);
            } else if word.starts_with("due:") && word.len() > 5 {
                let date_str = &word[4..];
                due = Some(chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")?);
            } else if word.starts_with("rec:") && word.len() > 4 {
                let rec_str = &word[4..];
                recurrence = Some(TodoRecurrence::from_str(rec_str)?);
            } else if word.starts_with("t:") && word.len() > 2 {
                let thresh_str = &word[2..];
                threshold = Some(chrono::NaiveDate::parse_from_str(thresh_str, "%Y-%m-%d")?);
            } else if word.starts_with("uuid:") && word.len() > 5 {
                let uuid_str = &word[5..];
                let parsed_uuid = Uuid::parse_str(uuid_str)?;
                // Only set if not already set, or overwrite?
                uuid = Some(parsed_uuid);
            } else if word.starts_with("sub:") && word.len() > 4 {
                let sub_str = &word[4..];
                let parsed_sub = Uuid::parse_str(sub_str)?;
                sub = Some(parsed_sub);
            } else if word.starts_with("pri:") && word.len() > 4 {
                let prio_str = &word[4..];
                // Unlike the other tags, an unparseable pri: value is not a hard error: it
                // falls through to plain description text, so a malformed tag never causes
                // TodoLibrary::load to silently drop the whole line.
                match TodoPriority::from_tag_value(prio_str) {
                    Ok(parsed) => {
                        if priority_from_parens && priority.priority != parsed.priority {
                            let existing = (priority.priority.unwrap() + b'A') as char;
                            let new = (parsed.priority.unwrap() + b'A') as char;
                            return Err(TodoItemParseError::ConflictingPriority(existing, new));
                        }
                        priority = parsed;
                    }
                    Err(_) => clean_description_parts.push(word.to_string()),
                }
            } else {
                clean_description_parts.push(word.to_string());
            }
        }

        let description = clean_description_parts.join(" ");

        Ok(TodoItem {
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
        })
    }
}

impl Display for TodoItem {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        // Built as tokens and joined with a single space, rather than interleaving
        // conditional leading/trailing spaces, so an absent field (e.g. an empty
        // description on a done item) can never produce a doubled-up space.
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

        if !self.description.is_empty() {
            parts.push(self.description.clone());
        }

        // A completed item's priority is written as the pri: tag rather than the
        // leading (X) form, which is reserved for open items.
        if self.done
            && let Some(p) = self.priority.priority
        {
            parts.push(format!("pri:{}", (p + b'A') as char));
        }

        for p in &self.projects {
            parts.push(p.to_string());
        }

        for c in &self.contexts {
            parts.push(c.to_string());
        }

        if let Some(d) = self.due {
            parts.push(format!("due:{}", d.format("%Y-%m-%d")));
        }

        if let Some(ref r) = self.recurrence {
            parts.push(format!("rec:{}", r));
        }

        if let Some(t) = self.threshold {
            parts.push(format!("t:{}", t.format("%Y-%m-%d")));
        }

        if let Some(u) = self.uuid {
            parts.push(format!("uuid:{}", u));
        }

        if let Some(s) = self.sub {
            parts.push(format!("sub:{}", s));
        }

        write!(f, "{}", parts.join(" "))
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
