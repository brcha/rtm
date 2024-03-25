use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TodoRecurrence {
    Daily(u16, bool),
    BusinessDay(u16, bool),
    Weekly(u16, bool),
    Monthly(u16, bool),
    Yearly(u16, bool),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoRecurrenceParseError;

impl FromStr for TodoRecurrence {
    type Err = TodoRecurrenceParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let strict_recurrence = s.starts_with('+');
        let count = if s.len() - (strict_recurrence as usize) == 1 {
            1
        } else {
            s[strict_recurrence as usize..s.len() - 1]
                .parse::<u16>()
                .or(Err(TodoRecurrenceParseError))?
        };

        match s.as_bytes()[s.len() - 1] {
            b'd' => Ok(TodoRecurrence::Daily(count, strict_recurrence)),
            b'b' => Ok(TodoRecurrence::BusinessDay(count, strict_recurrence)),
            b'w' => Ok(TodoRecurrence::Weekly(count, strict_recurrence)),
            b'm' => Ok(TodoRecurrence::Monthly(count, strict_recurrence)),
            b'y' => Ok(TodoRecurrence::Yearly(count, strict_recurrence)),
            _ => Err(TodoRecurrenceParseError),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_test() {
        assert_eq!(
            Ok(TodoRecurrence::Daily(3, false)),
            TodoRecurrence::from_str("3d")
        );
        assert_eq!(
            Ok(TodoRecurrence::Daily(17, true)),
            TodoRecurrence::from_str("+17d")
        );
        assert_eq!(
            Ok(TodoRecurrence::BusinessDay(8, false)),
            TodoRecurrence::from_str("8b")
        );
        assert_eq!(
            Ok(TodoRecurrence::Weekly(4, true)),
            TodoRecurrence::from_str("+4w")
        );
        assert_eq!(
            Ok(TodoRecurrence::Monthly(1, false)),
            TodoRecurrence::from_str("m")
        );
        assert_eq!(
            Ok(TodoRecurrence::Yearly(1, true)),
            TodoRecurrence::from_str("+y")
        );
        assert_eq!(
            TodoRecurrence::from_str("d"),
            TodoRecurrence::from_str("1d")
        );
    }
}
