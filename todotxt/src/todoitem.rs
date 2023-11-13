use chrono::NaiveDate;
use uuid::Uuid;
use crate::todocontext::TodoContext;
use crate::todopriority::TodoPriority;
use crate::todoproject::TodoProject;
use crate::todorecurrence::TodoRecurrence;

#[derive(Clone, Debug)]
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
    pub uuid: Option<Uuid>, // to be used to link with other items
    pub sub: Option<Uuid>, // to be used for subitems
}
