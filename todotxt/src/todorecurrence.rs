use std::fmt::Display;
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoRecurrenceParseError;

impl FromStr for TodoRecurrence {
    type Err = TodoRecurrenceParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let strict = s.starts_with('+');
        let count = if s.len() - (strict as usize) == 1 {
            1
        } else {
            s[strict as usize..s.len() - 1]
                .parse::<u16>()
                .or(Err(TodoRecurrenceParseError))?
        };

        let unit = match s.as_bytes()[s.len() - 1] {
            b'd' => Ok(TodoRecurrenceUnit::Daily),
            b'b' => Ok(TodoRecurrenceUnit::BusinessDay),
            b'w' => Ok(TodoRecurrenceUnit::Weekly),
            b'm' => Ok(TodoRecurrenceUnit::Monthly),
            b'y' => Ok(TodoRecurrenceUnit::Yearly),
            _ => Err(TodoRecurrenceParseError),
        };

        match unit {
            Ok(unit) => Ok(TodoRecurrence {
                strict,
                count,
                unit,
            }),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::TodoRecurrenceUnit::*;

    #[test]
    fn parse_test() {
        assert_eq!(
            Ok(TodoRecurrence {
                count: 3,
                strict: false,
                unit: Daily
            }),
            TodoRecurrence::from_str("3d")
        );
        assert_eq!(
            Ok(TodoRecurrence {
                count: 17,
                strict: true,
                unit: Daily
            }),
            TodoRecurrence::from_str("+17d")
        );
        assert_eq!(
            Ok(TodoRecurrence {
                count: 8,
                strict: false,
                unit: BusinessDay
            }),
            TodoRecurrence::from_str("8b")
        );
        assert_eq!(
            Ok(TodoRecurrence {
                count: 4,
                strict: true,
                unit: Weekly
            }),
            TodoRecurrence::from_str("+4w")
        );
        assert_eq!(
            Ok(TodoRecurrence {
                count: 1,
                strict: false,
                unit: Monthly
            }),
            TodoRecurrence::from_str("m")
        );
        assert_eq!(
            Ok(TodoRecurrence {
                count: 1,
                strict: true,
                unit: Yearly
            }),
            TodoRecurrence::from_str("+y")
        );
        assert_eq!(
            TodoRecurrence::from_str("d"),
            TodoRecurrence::from_str("1d")
        );
    }
}
