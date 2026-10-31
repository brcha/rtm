use super::*;

#[test]
fn parse_test() {
    assert_eq!(
        Ok(TodoProject {
            name: "work".to_string()
        }),
        TodoProject::from_str("+work")
    );
    assert!(matches!(
        TodoProject::from_str("work"),
        Err(TodoProjectParseError)
    ));
    assert!(matches!(
        TodoProject::from_str("+"),
        Err(TodoProjectParseError)
    ));
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

#[test]
fn parse_with_special_characters() {
    assert_eq!(
        Ok(TodoProject {
            name: "work-home".to_string()
        }),
        TodoProject::from_str("+work-home")
    );
    assert_eq!(
        Ok(TodoProject {
            name: "project123".to_string()
        }),
        TodoProject::from_str("+project123")
    );
}

#[test]
fn project_equality() {
    let p1 = TodoProject {
        name: "work".to_string(),
    };
    let p2 = TodoProject {
        name: "work".to_string(),
    };
    let p3 = TodoProject {
        name: "personal".to_string(),
    };
    assert_eq!(p1, p2);
    assert_ne!(p1, p3);
}

#[test]
fn project_clone() {
    let p1 = TodoProject {
        name: "work".to_string(),
    };
    let p2 = p1.clone();
    assert_eq!(p1, p2);
}
