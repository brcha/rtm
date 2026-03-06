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
mod tests {
    use super::TodoRecurrenceUnit::*;
    use super::*;

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

    #[test]
    fn display_test() {
        assert_eq!(
            "+d",
            TodoRecurrence {
                count: 1,
                strict: true,
                unit: Daily
            }
            .to_string()
        );
        assert_eq!(
            "13y",
            TodoRecurrence {
                count: 13,
                strict: false,
                unit: Yearly
            }
            .to_string()
        );
        assert_eq!(
            "3w",
            TodoRecurrence {
                count: 3,
                strict: false,
                unit: Weekly
            }
            .to_string()
        );
        assert_eq!(
            "2b",
            TodoRecurrence {
                count: 2,
                strict: false,
                unit: BusinessDay
            }
            .to_string()
        );
        assert_eq!(
            "4m",
            TodoRecurrence {
                count: 4,
                strict: false,
                unit: Monthly
            }
            .to_string()
        );
    }

    #[test]
    fn parse_invalid() {
        assert_eq!(Err(TodoRecurrenceParseError), TodoRecurrence::from_str("x"));
        assert_eq!(
            Err(TodoRecurrenceParseError),
            TodoRecurrence::from_str("1x")
        );
        assert_eq!(
            Err(TodoRecurrenceParseError),
            TodoRecurrence::from_str("+1x")
        );
    }

    #[test]
    fn display_strict_variants() {
        assert_eq!(
            "+d",
            TodoRecurrence {
                strict: true,
                count: 1,
                unit: Daily
            }
            .to_string()
        );
        assert_eq!(
            "+2w",
            TodoRecurrence {
                strict: true,
                count: 2,
                unit: Weekly
            }
            .to_string()
        );
        assert_eq!(
            "+3m",
            TodoRecurrence {
                strict: true,
                count: 3,
                unit: Monthly
            }
            .to_string()
        );
        assert_eq!(
            "+y",
            TodoRecurrence {
                strict: true,
                count: 1,
                unit: Yearly
            }
            .to_string()
        );
    }

    #[test]
    fn recurrence_equality() {
        let r1 = TodoRecurrence {
            strict: false,
            count: 1,
            unit: Daily,
        };
        let r2 = TodoRecurrence {
            strict: false,
            count: 1,
            unit: Daily,
        };
        let r3 = TodoRecurrence {
            strict: true,
            count: 1,
            unit: Daily,
        };
        assert_eq!(r1, r2);
        assert_ne!(r1, r3);
    }

    #[test]
    fn recurrence_clone() {
        let r1 = TodoRecurrence {
            strict: false,
            count: 1,
            unit: Weekly,
        };
        let r2 = r1.clone();
        assert_eq!(r1, r2);
    }
}
