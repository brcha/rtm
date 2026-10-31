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
    assert!(matches!(
        TodoRecurrence::from_str("x"),
        Err(TodoRecurrenceParseError::Unit)
    ));
    assert!(matches!(
        TodoRecurrence::from_str("1x"),
        Err(TodoRecurrenceParseError::Unit)
    ));
    assert!(matches!(
        TodoRecurrence::from_str("+1x"),
        Err(TodoRecurrenceParseError::Unit)
    ));
}

#[test]
fn parse_malformed_does_not_panic() {
    for s in ["", "+", "é", "+é"] {
        assert_eq!(
            TodoRecurrence::from_str(s),
            Err(TodoRecurrenceParseError::Unit),
            "{s:?}"
        );
    }
    for s in ["1é", "éd", "+éw"] {
        assert!(
            matches!(
                TodoRecurrence::from_str(s),
                Err(TodoRecurrenceParseError::Count(_) | TodoRecurrenceParseError::Unit)
            ),
            "{s:?}"
        );
    }
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
