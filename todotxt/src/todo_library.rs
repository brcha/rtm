use crate::todo_item::TodoItem;
use crate::todo_recurrence::TodoRecurrenceUnit;
use chrono::{Duration, Local};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct TodoLibrary {
    pub file_name: String,
    pub items: Vec<TodoItem>,
}

impl TodoLibrary {
    pub fn new(file_name: String) -> Self {
        TodoLibrary {
            file_name,
            items: Vec::new(),
        }
    }

    pub fn load(&mut self) -> Result<(), std::io::Error> {
        let content = std::fs::read_to_string(&self.file_name)?;
        self.items = content
            .lines()
            .filter_map(|line| line.parse::<TodoItem>().ok())
            .collect();
        Ok(())
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let content = self
            .items
            .iter()
            .map(|item| item.to_string())
            .collect::<Vec<String>>()
            .join("\n");
        std::fs::write(&self.file_name, content)?;
        Ok(())
    }

    pub fn add_item(&mut self, mut item: TodoItem) {
        // The todo.txt spec allows an item with no creation date, but then a completed item
        // can never carry one either (Display refuses to emit a lone creation date; see
        // TodoItem's Display impl). Guaranteeing a creation date here, at the single point
        // items enter the library, is what makes that possible without inventing dates
        // during parsing or completion. An item that already specifies one is left alone.
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
        // Per the todo.txt spec the completion date is mandatory on a done line, so set it
        // here rather than at Display time. Never fabricate a creation date to go with it —
        // that is TodoLibrary::add_item's job, at item creation. Idempotent: completing an
        // already-done item does not clobber its existing completion date.
        if !self.items[index].done {
            self.items[index].completion_date = Some(Local::now().date_naive());
        }
        self.items[index].done = true;
        // Check for recurrence and create new item
        let has_recurrence = self.items[index].recurrence.is_some();
        if has_recurrence
            && let Some(ref recurrence) = self.items[index].recurrence
            && let Some(current_due) = self.items[index].due
        {
            let base_date = if recurrence.strict {
                current_due
            } else {
                Local::now().date_naive()
            };
            let new_due = match recurrence.unit {
                TodoRecurrenceUnit::Daily => base_date + Duration::days(recurrence.count as i64),
                TodoRecurrenceUnit::BusinessDay => {
                    // Skip weekends or something, but for simplicity, treat as daily
                    base_date + Duration::days(recurrence.count as i64)
                }
                TodoRecurrenceUnit::Weekly => base_date + Duration::weeks(recurrence.count as i64),
                TodoRecurrenceUnit::Monthly => {
                    // Approximate months as 30 days
                    base_date + Duration::days(recurrence.count as i64 * 30)
                }
                TodoRecurrenceUnit::Yearly => {
                    // Approximate years as 365 days
                    base_date + Duration::days(recurrence.count as i64 * 365)
                }
            };
            let new_item = TodoItem {
                done: false,
                priority: self.items[index].priority.clone(),
                completion_date: None,
                creation_date: Some(Local::now().date_naive()),
                description: self.items[index].description.clone(),
                projects: self.items[index].projects.clone(),
                contexts: self.items[index].contexts.clone(),
                due: Some(new_due),
                recurrence: Some(recurrence.clone()),
                threshold: self.items[index].threshold,
                uuid: None,
                sub: None,
            };
            self.items.push(new_item);
        }
        Some(has_recurrence)
    }

    pub fn uncomplete_item(&mut self, index: usize) -> Option<()> {
        if index >= self.items.len() {
            return None;
        }
        self.items[index].done = false;
        self.items[index].completion_date = None;
        // Priority and creation_date are left untouched: the priority re-serializes as
        // (X) instead of pri:X automatically, since TodoItem::Display keys that choice off
        // `done`; the creation date, if any, is a fact about the item that un-completing it
        // does not change.
        Some(())
    }
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
