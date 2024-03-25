use std::fmt;
use std::fmt::Formatter;
use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoPriority {
    pub priority: Option<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoPriorityParseError;

impl FromStr for TodoPriority{
    type Err = TodoPriorityParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let prio_char = s
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .ok_or(TodoPriorityParseError)?;

        if prio_char.len() == 1 {
            if let p @ b'A'..=b'Z' = prio_char.as_bytes()[0] {
                Ok(TodoPriority { priority: Some(p - b'A') })
            } else {
                Err(TodoPriorityParseError)
            }
        } else {
            Err(TodoPriorityParseError)
        }
    }
}

impl fmt::Display for TodoPriority {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.priority {
            Some(p) => {
                write!(f, "({})", (p + b'A') as char)
            },
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_test() {
        assert_eq!(Ok(TodoPriority { priority: Some(3)}), TodoPriority::from_str("(D)"));
        assert_eq!(Err(TodoPriorityParseError), TodoPriority::from_str("(g)"));
        assert_eq!(Err(TodoPriorityParseError), TodoPriority::from_str("A"));
    }

    #[test]
    fn display_test() {
        assert_eq!("(A)", TodoPriority { priority: Some(0) }.to_string());
        assert_eq!("", TodoPriority { priority: None }.to_string());
    }
}
