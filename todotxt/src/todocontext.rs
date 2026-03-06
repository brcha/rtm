use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoContext {
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
mod tests {
    use super::*;

    #[test]
    fn parse_test() {
        assert_eq!(
            Ok(TodoContext {
                name: "home".to_string()
            }),
            TodoContext::from_str("@home")
        );
        assert_eq!(Err(TodoContextParseError), TodoContext::from_str("home"));
        assert_eq!(Err(TodoContextParseError), TodoContext::from_str("@"));
    }

    #[test]
    fn display_test() {
        assert_eq!(
            "@office",
            TodoContext {
                name: "office".to_string()
            }
            .to_string()
        );
    }

    #[test]
    fn parse_with_special_characters() {
        assert_eq!(
            Ok(TodoContext {
                name: "work-home".to_string()
            }),
            TodoContext::from_str("@work-home")
        );
        assert_eq!(
            Ok(TodoContext {
                name: "home123".to_string()
            }),
            TodoContext::from_str("@home123")
        );
    }

    #[test]
    fn context_equality() {
        let c1 = TodoContext {
            name: "home".to_string(),
        };
        let c2 = TodoContext {
            name: "home".to_string(),
        };
        let c3 = TodoContext {
            name: "work".to_string(),
        };
        assert_eq!(c1, c2);
        assert_ne!(c1, c3);
    }

    #[test]
    fn context_clone() {
        let c1 = TodoContext {
            name: "home".to_string(),
        };
        let c2 = c1.clone();
        assert_eq!(c1, c2);
    }
}
