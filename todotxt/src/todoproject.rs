use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoProject {
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoProjectParseError;

impl FromStr for TodoProject {
    type Err = TodoProjectParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with('+') && s.len() > 1 {
            Ok(TodoProject {
                name: s[1..].to_string(),
            })
        } else {
            Err(TodoProjectParseError)
        }
    }
}

impl Display for TodoProject {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "+{}", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_test() {
        assert_eq!(
            Ok(TodoProject {
                name: "work".to_string()
            }),
            TodoProject::from_str("+work")
        );
        assert_eq!(Err(TodoProjectParseError), TodoProject::from_str("work"));
        assert_eq!(Err(TodoProjectParseError), TodoProject::from_str("+"));
    }

    #[test]
    fn display_test() {
        assert_eq!(
            "+personal",
            TodoProject {
                name: "personal".to_string()
            }
            .to_string()
        );
    }
}
