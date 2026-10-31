use std::fmt;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoPriority {
    pub priority: Option<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TodoPriorityParseError {
    #[error("priority must be wrapped in parentheses")]
    NotParenthesized,
    #[error("priority must be a single character in A-Z")]
    InvalidLetter,
}

impl TodoPriority {
    /// Parses a bare priority letter, e.g. the `X` in a `pri:X` tag, with no parentheses.
    pub fn from_tag_value(s: &str) -> Result<Self, TodoPriorityParseError> {
        if s.len() == 1 {
            if let p @ b'A'..=b'Z' = s.as_bytes()[0] {
                Ok(TodoPriority {
                    priority: Some(p - b'A'),
                })
            } else {
                Err(TodoPriorityParseError::InvalidLetter)
            }
        } else {
            Err(TodoPriorityParseError::InvalidLetter)
        }
    }
}

impl FromStr for TodoPriority {
    type Err = TodoPriorityParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let prio_char = s
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .ok_or(TodoPriorityParseError::NotParenthesized)?;

        Self::from_tag_value(prio_char)
    }
}

impl Display for TodoPriority {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.priority {
            Some(p) => {
                write!(f, "({})", (p + b'A') as char)
            }
            None => Ok(()),
        }
    }
}

#[cfg(test)]
#[path = "tests/todo_priority_format_tests.rs"]
mod format_tests;
