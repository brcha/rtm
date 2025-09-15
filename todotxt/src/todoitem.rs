use crate::todocontext::TodoContext;
use crate::todopriority::TodoPriority;
use crate::todoproject::TodoProject;
use crate::todorecurrence::TodoRecurrence;
use chrono::NaiveDate;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use crate::todocontext::TodoContextParseError;
use crate::todopriority::TodoPriorityParseError;
use crate::todoproject::TodoProjectParseError;
use crate::todorecurrence::TodoRecurrenceParseError;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoItem {
    pub done: bool,
    pub priority: TodoPriority,
    pub completion_date: Option<NaiveDate>,
    pub creation_date: Option<NaiveDate>, // must exist if completion date is
    pub description: String,
    pub projects: Vec<TodoProject>,
    pub contexts: Vec<TodoContext>,
    pub due: Option<NaiveDate>,
    pub recurrence: Option<TodoRecurrence>,
    pub threshold: Option<NaiveDate>,
    pub uuid: Option<Uuid>,
    pub sub: Option<Uuid>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoItemParseError;

impl From<uuid::Error> for TodoItemParseError {
    fn from(_: uuid::Error) -> Self {
        TodoItemParseError
    }
}

impl From<chrono::ParseError> for TodoItemParseError {
    fn from(_: chrono::ParseError) -> Self {
        TodoItemParseError
    }
}

impl From<TodoPriorityParseError> for TodoItemParseError {
    fn from(_: TodoPriorityParseError) -> Self {
        TodoItemParseError
    }
}

impl From<TodoProjectParseError> for TodoItemParseError {
    fn from(_: TodoProjectParseError) -> Self {
        TodoItemParseError
    }
}

impl From<TodoContextParseError> for TodoItemParseError {
    fn from(_: TodoContextParseError) -> Self {
        TodoItemParseError
    }
}

impl From<TodoRecurrenceParseError> for TodoItemParseError {
    fn from(_: TodoRecurrenceParseError) -> Self {
        TodoItemParseError
    }
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

        let priority = if let Some(prio_str) = parts.get(index) {
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

        let mut completion_date = None;
        let mut creation_date = None;

        if done {
            let comp_str = parts.get(index).ok_or(TodoItemParseError)?;
            completion_date = Some(chrono::NaiveDate::parse_from_str(comp_str, "%Y-%m-%d")?);
            index += 1;

            let creat_str = parts.get(index).ok_or(TodoItemParseError)?;
            creation_date = Some(chrono::NaiveDate::parse_from_str(creat_str, "%Y-%m-%d")?);
            index += 1;
        } else {
            if let Some(date_str) = parts.get(index) {
                if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                    creation_date = Some(date);
                    index += 1;
                }
            }
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
        if self.done {
            write!(f, "x")?;
            if self.priority.priority.is_some() {
                write!(f, " {}", self.priority)?;
            }
            write!(f, " ")?;
        } else {
            if self.priority.priority.is_some() {
                write!(f, "{} ", self.priority)?;
            }
        }

        if self.done {
            if let Some(cd) = self.completion_date {
                write!(f, "{} ", cd.format("%Y-%m-%d"))?;
            }
            if let Some(cd) = self.creation_date {
                write!(f, "{} ", cd.format("%Y-%m-%d"))?;
            }
        } else {
            if let Some(cd) = self.creation_date {
                write!(f, "{} ", cd.format("%Y-%m-%d"))?;
            }
        }

        write!(f, "{}", self.description)?;

        for p in &self.projects {
            write!(f, " {}", p)?;
        }

        for c in &self.contexts {
            write!(f, " {}", c)?;
        }

        if let Some(d) = self.due {
            write!(f, " due:{}", d.format("%Y-%m-%d"))?;
        }

        if let Some(ref r) = self.recurrence {
            write!(f, " rec:{}", r)?;
        }

        if let Some(t) = self.threshold {
            write!(f, " t:{}", t.format("%Y-%m-%d"))?;
        }

        if let Some(u) = self.uuid {
            write!(f, " uuid:{}", u)?;
        }

        if let Some(s) = self.sub {
            write!(f, " sub:{}", s)?;
        }

        Ok(())
    }
}

impl TodoItem {
    pub fn add_subtask(mut parent: TodoItem, mut child: TodoItem) -> (Option<TodoItem>, TodoItem) {
        let has_uuid = parent.uuid.is_some();
        let new_uuid = if !has_uuid {
            let uuid = Uuid::new_v4();
            parent.uuid = Some(uuid);
            uuid
        } else {
            parent.uuid.unwrap()
        };
        child.sub = Some(new_uuid);
        let updated_parent = if has_uuid { None } else { Some(parent) };
        (updated_parent, child)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn parse_incomplete_simple() {
        let item: TodoItem = "Buy groceries".parse().unwrap();
        assert!(!item.done);
        assert_eq!(item.priority, TodoPriority { priority: None });
        assert_eq!(item.completion_date, None);
        assert_eq!(item.creation_date, None);
        assert_eq!(item.description, "Buy groceries");
        assert!(item.projects.is_empty());
        assert!(item.contexts.is_empty());
        assert_eq!(item.due, None);
        assert_eq!(item.recurrence, None);
        assert_eq!(item.threshold, None);
        assert_eq!(item.uuid, None);
        assert_eq!(item.sub, None);
    }

    #[test]
    fn parse_with_priority() {
        let item: TodoItem = "(A) Call mom".parse().unwrap();
        assert!(!item.done);
        assert_eq!(item.priority.priority, Some(0));
        assert_eq!(item.description, "Call mom");
    }

    #[test]
    fn parse_with_project_and_context() {
        let item: TodoItem = "Buy milk +Personal @home".parse().unwrap();
        assert!(!item.done);
        assert_eq!(
            item.projects,
            vec![TodoProject {
                name: "Personal".to_string()
            }]
        );
        assert_eq!(
            item.contexts,
            vec![TodoContext {
                name: "home".to_string()
            }]
        );
        assert_eq!(item.description, "Buy milk");
        assert_eq!(item.threshold, None);
        assert_eq!(item.uuid, None);
        assert_eq!(item.sub, None);
    }

    #[test]
    fn parse_completed_with_dates() {
        let item: TodoItem = "x (A) 2023-05-26 2023-05-20 Review code".parse().unwrap();
        assert!(item.done);
        assert_eq!(item.priority.priority, Some(0));
        assert_eq!(
            item.completion_date,
            Some(NaiveDate::from_ymd_opt(2023, 5, 26).unwrap())
        );
        assert_eq!(
            item.creation_date,
            Some(NaiveDate::from_ymd_opt(2023, 5, 20).unwrap())
        );
        assert_eq!(item.description, "Review code");
    }

    #[test]
    fn display_simple() {
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Buy milk".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        assert_eq!(item.to_string(), "Buy milk");
        assert_eq!(item.threshold, None);
        assert_eq!(item.uuid, None);
        assert_eq!(item.sub, None);
    }

    #[test]
    fn parse_with_extensions() {
        let item: TodoItem = "Buy groceries +Personal @home due:2023-05-30 rec:1m t:2023-05-25"
            .parse()
            .unwrap();
        assert!(!item.done);
        assert_eq!(item.priority, TodoPriority { priority: None });
        assert_eq!(item.description, "Buy groceries");
        assert_eq!(
            item.projects,
            vec![TodoProject {
                name: "Personal".to_string()
            }]
        );
        assert_eq!(
            item.contexts,
            vec![TodoContext {
                name: "home".to_string()
            }]
        );
        assert_eq!(
            item.due,
            Some(NaiveDate::from_ymd_opt(2023, 5, 30).unwrap())
        );
        assert_eq!(item.recurrence, Some("1m".parse().unwrap()));
        assert_eq!(
            item.threshold,
            Some(NaiveDate::from_ymd_opt(2023, 5, 25).unwrap())
        );
    }

    #[test]
    fn display_with_extensions() {
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Buy groceries".to_string(),
            projects: vec![TodoProject {
                name: "Personal".to_string(),
            }],
            contexts: vec![TodoContext {
                name: "home".to_string(),
            }],
            due: Some(NaiveDate::from_ymd_opt(2023, 5, 30).unwrap()),
            recurrence: Some("1m".parse().unwrap()),
            threshold: Some(NaiveDate::from_ymd_opt(2023, 5, 25).unwrap()),
            uuid: None,
            sub: None,
        };
        assert_eq!(
            item.to_string(),
            "Buy groceries +Personal @home due:2023-05-30 rec:m t:2023-05-25"
        );
    }

    #[test]
    fn parse_with_uuid_and_sub() {
        let item: TodoItem = "Buy milk uuid:12345678-1234-1234-1234-123456789abc sub:87654321-4321-4321-4321-abc123456789".parse().unwrap();
        assert_eq!(item.description, "Buy milk");
        assert_eq!(
            item.uuid,
            Some(Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap())
        );
        assert_eq!(
            item.sub,
            Some(Uuid::parse_str("87654321-4321-4321-4321-abc123456789").unwrap())
        );
    }

    #[test]
    fn display_with_uuid_and_sub() {
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Buy milk".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: Some(Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap()),
            sub: Some(Uuid::parse_str("87654321-4321-4321-4321-abc123456789").unwrap()),
        };
        assert_eq!(
            item.to_string(),
            "Buy milk uuid:12345678-1234-1234-1234-123456789abc sub:87654321-4321-4321-4321-abc123456789"
        );
    }

    #[test]
    fn add_subtask_new_uuid() {
        let parent = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Parent task".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        let child = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Child task".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        let (updated_parent, new_child) = TodoItem::add_subtask(parent, child);
        assert!(updated_parent.is_some());
        let up = updated_parent.unwrap();
        assert_eq!(up.uuid, new_child.sub);
        assert!(new_child.sub.is_some());
        assert_eq!(up.description, "Parent task");
        assert_eq!(new_child.description, "Child task");
    }

    #[test]
    fn add_subtask_existing_uuid() {
        let existing_uuid = Uuid::new_v4();
        let parent = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Parent task".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: Some(existing_uuid),
            sub: None,
        };
        let child = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Child task".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: Some(Uuid::new_v4()), // existing sub, should be overwritten
        };
        let (updated_parent, new_child) = TodoItem::add_subtask(parent, child);
        assert!(updated_parent.is_none());
        assert_eq!(new_child.sub, Some(existing_uuid));
        assert_eq!(new_child.description, "Child task");
    }
}
