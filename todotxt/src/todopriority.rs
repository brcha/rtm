use std::fmt;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoPriority {
    pub priority: Option<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoPriorityParseError;

impl FromStr for TodoPriority {
    type Err = TodoPriorityParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let prio_char = s
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .ok_or(TodoPriorityParseError)?;

        if prio_char.len() == 1 {
            if let p @ b'A'..=b'Z' = prio_char.as_bytes()[0] {
                Ok(TodoPriority {
                    priority: Some(p - b'A'),
                })
            } else {
                Err(TodoPriorityParseError)
            }
        } else {
            Err(TodoPriorityParseError)
        }
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
mod tests {
    use super::*;

    #[test]
    fn parse_test() {
        assert_eq!(
            Ok(TodoPriority { priority: Some(3) }),
            TodoPriority::from_str("(D)")
        );
        assert_eq!(Err(TodoPriorityParseError), TodoPriority::from_str("(g)"));
        assert_eq!(Err(TodoPriorityParseError), TodoPriority::from_str("A"));
    }

    #[test]
    fn display_test() {
        assert_eq!("(A)", TodoPriority { priority: Some(0) }.to_string());
        assert_eq!("", TodoPriority { priority: None }.to_string());
    }

    #[test]
    fn parse_all_priorities() {
        assert_eq!(
            Ok(TodoPriority { priority: Some(0) }),
            TodoPriority::from_str("(A)")
        );
        assert_eq!(
            Ok(TodoPriority { priority: Some(1) }),
            TodoPriority::from_str("(B)")
        );
        assert_eq!(
            Ok(TodoPriority { priority: Some(2) }),
            TodoPriority::from_str("(C)")
        );
        assert_eq!(
            Ok(TodoPriority { priority: Some(25) }),
            TodoPriority::from_str("(Z)")
        );
    }

    #[test]
    fn display_all_priorities() {
        for i in 0u8..26 {
            let expected = format!("({})", (i + b'A') as char);
            assert_eq!(expected, TodoPriority { priority: Some(i) }.to_string());
        }
    }

    #[test]
    fn parse_invalid_priorities() {
        assert_eq!(Err(TodoPriorityParseError), TodoPriority::from_str("(a)"));
        assert_eq!(Err(TodoPriorityParseError), TodoPriority::from_str("(AA)"));
        assert_eq!(Err(TodoPriorityParseError), TodoPriority::from_str("(0)"));
        assert_eq!(Err(TodoPriorityParseError), TodoPriority::from_str("()"));
        assert_eq!(Err(TodoPriorityParseError), TodoPriority::from_str("(A"));
        assert_eq!(Err(TodoPriorityParseError), TodoPriority::from_str("A)"));
    }

    #[test]
    fn priority_equality() {
        let p1 = TodoPriority { priority: Some(0) };
        let p2 = TodoPriority { priority: Some(0) };
        let p3 = TodoPriority { priority: Some(1) };
        let p4 = TodoPriority { priority: None };
        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
        assert_ne!(p1, p4);
    }

    #[test]
    fn priority_clone() {
        let p1 = TodoPriority { priority: Some(5) };
        let p2 = p1.clone();
        assert_eq!(p1, p2);
    }
}
