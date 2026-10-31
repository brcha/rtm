use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoRecurrence {
    pub strict: bool,
    pub count: u16,
    pub unit: TodoRecurrenceUnit,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TodoRecurrenceUnit {
    Daily,
    BusinessDay,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TodoRecurrenceParseError {
    #[error("invalid recurrence count")]
    Count(#[from] std::num::ParseIntError),
    #[error("unknown recurrence unit")]
    Unit,
}

impl FromStr for TodoRecurrence {
    type Err = TodoRecurrenceParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (strict, rest) = match s.strip_prefix('+') {
            Some(rest) => (true, rest),
            None => (false, s),
        };
        let mut chars = rest.chars();
        let unit_char = chars.next_back().ok_or(TodoRecurrenceParseError::Unit)?;
        let count = match chars.as_str() {
            "" => 1,
            digits => digits.parse::<u16>()?,
        };

        let unit = match unit_char {
            'd' => TodoRecurrenceUnit::Daily,
            'b' => TodoRecurrenceUnit::BusinessDay,
            'w' => TodoRecurrenceUnit::Weekly,
            'm' => TodoRecurrenceUnit::Monthly,
            'y' => TodoRecurrenceUnit::Yearly,
            _ => return Err(TodoRecurrenceParseError::Unit),
        };

        Ok(TodoRecurrence {
            strict,
            count,
            unit,
        })
    }
}

impl Display for TodoRecurrence {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.strict {
            write!(f, "+")?;
        }
        if self.count > 1 {
            write!(f, "{}", self.count)?;
        }
        match self.unit {
            TodoRecurrenceUnit::Daily => {
                write!(f, "d")
            }
            TodoRecurrenceUnit::BusinessDay => {
                write!(f, "b")
            }
            TodoRecurrenceUnit::Weekly => {
                write!(f, "w")
            }
            TodoRecurrenceUnit::Monthly => {
                write!(f, "m")
            }
            TodoRecurrenceUnit::Yearly => {
                write!(f, "y")
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/todo_recurrence_format_tests.rs"]
mod format_tests;
