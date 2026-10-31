use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoContext {
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("context must start with '@' and be non-empty")]
pub struct TodoContextParseError;

impl FromStr for TodoContext {
    type Err = TodoContextParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with('@') && s.len() > 1 {
            Ok(TodoContext {
                name: s[1..].to_string(),
            })
        } else {
            Err(TodoContextParseError)
        }
    }
}

impl Display for TodoContext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}", self.name)
    }
}

#[cfg(test)]
#[path = "tests/todo_context_format_tests.rs"]
mod format_tests;
