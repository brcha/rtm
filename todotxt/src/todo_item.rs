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
            if let Some(date_str) = parts.get(index) {
                if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                    completion_date = Some(date);
                    index += 1;
                }
            }
            if let Some(date_str) = parts.get(index) {
                if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                    creation_date = Some(date);
                    index += 1;
                }
            }
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
            if let Some(cd) = self.completion_date {
                parts.push(cd.format("%Y-%m-%d").to_string());
            }
            if let Some(cd) = self.creation_date {
                parts.push(cd.format("%Y-%m-%d").to_string());
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
mod tests {
    use super::*;
    use crate::todo_recurrence::TodoRecurrenceUnit;
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
    fn parse_pri_tag_on_completed_item() {
        let item: TodoItem = "x 2024-10-07 2024-08-31 Task pri:A +p @c due:2024-10-05 rec:5w"
            .parse()
            .unwrap();
        assert!(item.done);
        assert_eq!(item.priority.priority, Some(0));
        assert_eq!(
            item.completion_date,
            Some(NaiveDate::from_ymd_opt(2024, 10, 7).unwrap())
        );
        assert_eq!(
            item.creation_date,
            Some(NaiveDate::from_ymd_opt(2024, 8, 31).unwrap())
        );
        assert_eq!(item.description, "Task");
    }

    #[test]
    fn parse_pri_tag_on_open_item() {
        let item: TodoItem = "Task pri:B".parse().unwrap();
        assert!(!item.done);
        assert_eq!(item.priority.priority, Some(1));
        assert_eq!(item.description, "Task");
    }

    #[test]
    fn parse_pri_tag_agreeing_with_parens() {
        let item: TodoItem = "x (C) 2024-01-02 2024-01-01 Task pri:C".parse().unwrap();
        assert_eq!(item.priority.priority, Some(2));
        assert_eq!(item.description, "Task");
    }

    #[test]
    fn parse_pri_tag_conflicting_with_parens() {
        let result: Result<TodoItem, _> = "x (C) 2024-01-02 2024-01-01 Task pri:A".parse();
        assert_eq!(
            result,
            Err(TodoItemParseError::ConflictingPriority('C', 'A'))
        );
    }

    #[test]
    fn parse_pri_tag_malformed_falls_through_to_description() {
        let item: TodoItem = "Task pri:xyz".parse().unwrap();
        assert_eq!(item.priority.priority, None);
        assert_eq!(item.description, "Task pri:xyz");
    }

    #[test]
    fn parse_legacy_bare_x_no_dates() {
        let item: TodoItem = "x Task".parse().unwrap();
        assert!(item.done);
        assert_eq!(item.completion_date, None);
        assert_eq!(item.creation_date, None);
        assert_eq!(item.description, "Task");
    }

    #[test]
    fn parse_legacy_bare_x_with_priority_no_dates() {
        let item: TodoItem = "x (A) Task".parse().unwrap();
        assert!(item.done);
        assert_eq!(item.priority.priority, Some(0));
        assert_eq!(item.completion_date, None);
        assert_eq!(item.creation_date, None);
        assert_eq!(item.description, "Task");
    }

    #[test]
    fn parse_reported_issue_line() {
        let item: TodoItem =
            "x (C) 2026-05-23 Инфостан pri:C +рачуни @кућа due:2026-05-09 rec:+m t:2026-03-15"
                .parse()
                .unwrap();
        assert!(item.done);
        assert_eq!(item.priority.priority, Some(2));
        assert_eq!(item.description, "Инфостан");
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
    fn display_completed_item_uses_pri_tag() {
        let item: TodoItem =
            "x 2024-10-07 2024-08-31 Стронгхолд pri:A +одржавање @фејнман @здравље due:2024-10-05 rec:5w"
                .parse()
                .unwrap();
        assert_eq!(
            item.to_string(),
            "x 2024-10-07 2024-08-31 Стронгхолд pri:A +одржавање @фејнман @здравље due:2024-10-05 rec:5w"
        );
    }

    #[test]
    fn display_completed_item_round_trip_is_idempotent() {
        let item: TodoItem =
            "x 2024-10-07 2024-08-31 Стронгхолд pri:A +одржавање @фејнман @здравље due:2024-10-05 rec:5w"
                .parse()
                .unwrap();
        let once = item.to_string();
        let twice: TodoItem = once.parse().unwrap();
        assert_eq!(twice.to_string(), once);
    }

    #[test]
    fn display_open_item_with_priority_uses_parens_not_pri_tag() {
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: Some(0) },
            completion_date: None,
            creation_date: None,
            description: "Call mom".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        assert_eq!(item.to_string(), "(A) Call mom");
    }

    #[test]
    fn display_normalizes_pri_tag_on_open_item_to_parens() {
        let item: TodoItem = "Task pri:B".parse().unwrap();
        assert_eq!(item.to_string(), "(B) Task");
    }

    #[test]
    fn display_normalizes_parens_on_completed_item_to_pri_tag() {
        let item: TodoItem = "x (C) 2024-01-02 2024-01-01 Task".parse().unwrap();
        assert_eq!(item.to_string(), "x 2024-01-02 2024-01-01 Task pri:C");
    }

    #[test]
    fn display_legacy_bare_x_round_trip() {
        let item: TodoItem = "x Task".parse().unwrap();
        assert_eq!(item.to_string(), "x Task");
    }

    #[test]
    fn display_completed_item_with_empty_description_has_no_double_space() {
        let item = TodoItem {
            done: true,
            priority: TodoPriority { priority: Some(0) },
            completion_date: None,
            creation_date: None,
            description: "".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        assert_eq!(item.to_string(), "x pri:A");
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
        let (updated_parent, new_child) = parent.add_subtask(&child);
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
        let (updated_parent, new_child) = parent.add_subtask(&child);
        assert!(updated_parent.is_none());
        assert_eq!(new_child.sub, Some(existing_uuid));
        assert_eq!(new_child.description, "Child task");
    }

    #[test]
    fn test_set_done() {
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Test".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        let new_item = item.set_done(true);
        assert_eq!(new_item.done, true);
        assert_eq!(item.done, false); // original unchanged
    }

    #[test]
    fn test_set_priority() {
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: Some(0) },
            completion_date: None,
            creation_date: None,
            description: "Test".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        let new_item = item.set_priority(TodoPriority { priority: Some(1) });
        assert_eq!(new_item.priority.priority, Some(1));
        assert_eq!(item.priority.priority, Some(0));
    }

    #[test]
    fn test_set_description() {
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Old".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        let new_item = item.set_description("New".to_string());
        assert_eq!(new_item.description, "New");
        assert_eq!(item.description, "Old");
    }

    #[test]
    fn test_add_project() {
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Test".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        let proj = TodoProject {
            name: "Work".to_string(),
        };
        let new_item = item.add_project(proj.clone());
        assert_eq!(new_item.projects.len(), 1);
        assert_eq!(new_item.projects[0], proj);
        assert_eq!(item.projects.len(), 0); // original unchanged
    }

    #[test]
    fn test_remove_project() {
        let proj = TodoProject {
            name: "Work".to_string(),
        };
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Test".to_string(),
            projects: vec![proj.clone()],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        let new_item = item.remove_project(&proj).unwrap();
        assert_eq!(new_item.projects.len(), 0);
        assert_eq!(item.projects.len(), 1);
    }

    #[test]
    fn test_add_context() {
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Test".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        let ctx = TodoContext {
            name: "Home".to_string(),
        };
        let new_item = item.add_context(ctx.clone());
        assert_eq!(new_item.contexts.len(), 1);
        assert_eq!(new_item.contexts[0], ctx);
        assert_eq!(item.contexts.len(), 0);
    }

    #[test]
    fn test_remove_context() {
        let ctx = TodoContext {
            name: "Home".to_string(),
        };
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Test".to_string(),
            projects: vec![],
            contexts: vec![ctx.clone()],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        let new_item = item.remove_context(&ctx).unwrap();
        assert_eq!(new_item.contexts.len(), 0);
        assert_eq!(item.contexts.len(), 1);
    }

    #[test]
    fn test_set_due() {
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Test".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        let date = NaiveDate::from_ymd_opt(2023, 5, 30).unwrap();
        let new_item = item.set_due(Some(date));
        assert_eq!(new_item.due, Some(date));
        assert_eq!(item.due, None);
    }

    #[test]
    fn test_set_recurrence() {
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Test".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        let rec = TodoRecurrence::from_str("1m").unwrap();
        let new_item = item.set_recurrence(Some(rec.clone()));
        assert_eq!(new_item.recurrence, Some(rec));
        assert_eq!(item.recurrence, None);
    }

    #[test]
    fn test_set_threshold() {
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Test".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        let date = NaiveDate::from_ymd_opt(2023, 5, 25).unwrap();
        let new_item = item.set_threshold(Some(date));
        assert_eq!(new_item.threshold, Some(date));
        assert_eq!(item.threshold, None);
    }

    #[test]
    fn test_set_uuid() {
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Test".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        let uuid = Uuid::new_v4();
        let new_item = TodoItem {
            uuid: Some(uuid),
            ..item.clone()
        };
        assert_eq!(new_item.uuid, Some(uuid));
        assert_eq!(item.uuid, None);
    }

    #[test]
    fn test_set_sub() {
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Test".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        let sub = Uuid::new_v4();
        let new_item = TodoItem {
            sub: Some(sub),
            ..item.clone()
        };
        assert_eq!(new_item.sub, Some(sub));
        assert_eq!(item.sub, None);
    }

    #[test]
    fn test_set_completion_date() {
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Test".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        let date = NaiveDate::from_ymd_opt(2023, 5, 26).unwrap();
        let new_item = item.set_completion_date(Some(date));
        assert_eq!(new_item.completion_date, Some(date));
        assert_eq!(item.completion_date, None);
    }

    #[test]
    fn test_set_creation_date() {
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Test".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        let date = NaiveDate::from_ymd_opt(2023, 5, 20).unwrap();
        let new_item = item.set_creation_date(Some(date));
        assert_eq!(new_item.creation_date, Some(date));
        assert_eq!(item.creation_date, None);
    }

    #[test]
    fn test_set_contexts() {
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Test".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        let contexts = vec![TodoContext {
            name: "home".to_string(),
        }];
        let new_item = item.set_contexts(contexts.clone());
        assert_eq!(new_item.contexts, contexts);
        assert!(item.contexts.is_empty());
    }

    #[test]
    fn test_item_equality() {
        let item1 = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "Test".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        let item2 = item1.clone();
        assert_eq!(item1, item2);
    }

    #[test]
    fn test_item_clone() {
        let item1 = TodoItem {
            done: false,
            priority: TodoPriority { priority: Some(1) },
            completion_date: None,
            creation_date: Some(NaiveDate::from_ymd_opt(2023, 5, 20).unwrap()),
            description: "Test".to_string(),
            projects: vec![TodoProject {
                name: "Work".to_string(),
            }],
            contexts: vec![TodoContext {
                name: "home".to_string(),
            }],
            due: Some(NaiveDate::from_ymd_opt(2023, 5, 30).unwrap()),
            recurrence: Some(TodoRecurrence {
                strict: false,
                count: 1,
                unit: TodoRecurrenceUnit::Daily,
            }),
            threshold: Some(NaiveDate::from_ymd_opt(2023, 5, 25).unwrap()),
            uuid: Some(Uuid::new_v4()),
            sub: None,
        };
        let item2 = item1.clone();
        assert_eq!(item1, item2);
    }

    #[test]
    fn test_roundtrip_with_all_fields() {
        let original =
            "(A) 2023-05-20 Complete task +Work @office due:2023-05-30 rec:1w t:2023-05-25";
        let parsed: TodoItem = original.parse().unwrap();
        let output = parsed.to_string();
        assert!(output.contains("(A)"));
        assert!(output.contains("Complete task"));
        assert!(output.contains("+Work"));
        assert!(output.contains("@office"));
        assert!(output.contains("due:2023-05-30"));
        assert!(output.contains("rec:w"));
        assert!(output.contains("t:2023-05-25"));
    }

    #[test]
    fn test_parse_only_description() {
        let item: TodoItem = "Just a simple task".parse().unwrap();
        assert_eq!(item.description, "Just a simple task");
        assert!(!item.done);
        assert_eq!(item.priority.priority, None);
        assert!(item.projects.is_empty());
        assert!(item.contexts.is_empty());
        assert_eq!(item.due, None);
    }

    #[test]
    fn test_parse_multiple_projects_and_contexts() {
        let item: TodoItem = "Task +Project1 +Project2 @context1 @context2"
            .parse()
            .unwrap();
        assert_eq!(item.projects.len(), 2);
        assert_eq!(item.contexts.len(), 2);
        assert_eq!(item.projects[0].name, "Project1");
        assert_eq!(item.projects[1].name, "Project2");
        assert_eq!(item.contexts[0].name, "context1");
        assert_eq!(item.contexts[1].name, "context2");
    }

    #[test]
    fn test_display_empty_item() {
        let item = TodoItem {
            done: false,
            priority: TodoPriority { priority: None },
            completion_date: None,
            creation_date: None,
            description: "".to_string(),
            projects: vec![],
            contexts: vec![],
            due: None,
            recurrence: None,
            threshold: None,
            uuid: None,
            sub: None,
        };
        assert_eq!(item.to_string(), "");
    }
}
