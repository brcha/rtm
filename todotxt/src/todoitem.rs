use chrono::NaiveDate;

pub struct TodoItem {
    pub done: bool,
    pub priority: char,
    pub description: str,
    pub due: Some(NaiveDate),
    pub reccurence: Some(()),
}
