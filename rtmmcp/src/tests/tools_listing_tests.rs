use super::support_tests::*;
use super::*;

const FILE: &str = "2026-01-01 Buy milk +home @shop\n\
    x 2026-01-03 2026-01-01 Pay rent +home\n\
    2026-01-02 Write report +work @office\n";

#[test]
fn test_open_items_are_listed_by_default_with_their_file_numbers() {
    let source = source_with(FILE);
    let listed = list_items(&source, &Filter::default()).unwrap();
    assert_eq!(
        listed,
        "1 2026-01-01 Buy milk +home @shop\n3 2026-01-02 Write report +work @office"
    );
    remove(source);
}

#[test]
fn test_items_are_filtered_by_status_tags_and_text() {
    let source = source_with(FILE);
    let done = Filter {
        status: Status::Done,
        ..Filter::default()
    };
    assert_eq!(
        list_items(&source, &done).unwrap(),
        "2 x 2026-01-03 2026-01-01 Pay rent +home"
    );
    let home = Filter {
        status: Status::All,
        projects: vec!["home".to_owned()],
        ..Filter::default()
    };
    assert_eq!(list_items(&source, &home).unwrap().lines().count(), 2);
    let office = Filter {
        contexts: vec!["@office".to_owned()],
        ..Filter::default()
    };
    assert!(list_items(&source, &office).unwrap().starts_with("3 "));
    let report = Filter {
        text: Some("REPORT".to_owned()),
        ..Filter::default()
    };
    assert!(list_items(&source, &report).unwrap().starts_with("3 "));
    remove(source);
}

#[test]
fn test_items_are_filtered_by_due_date() {
    let today = Local::now().date_naive();
    let source = source_with(&format!(
        "Today due:{today}\nLate due:{}\nLater due:{}\nNo date\n",
        today - Duration::days(1),
        today + Duration::days(10),
    ));
    let due = |due| {
        list_items(
            &source,
            &Filter {
                due,
                ..Filter::default()
            },
        )
        .unwrap()
    };
    assert!(due(Due::Today).starts_with("1 Today"));
    assert!(due(Due::Week).starts_with("1 Today"));
    assert!(due(Due::Overdue).starts_with("2 Late"));
    assert!(due(Due::Future).starts_with("3 Later"));
    assert_eq!(due(Due::All).lines().count(), 4);
    remove(source);
}

#[test]
fn test_nothing_matching_says_so() {
    let source = source_with(FILE);
    let none = Filter {
        text: Some("nothing like this".to_owned()),
        ..Filter::default()
    };
    assert_eq!(list_items(&source, &none).unwrap(), "No matching items.");
    remove(source);
}

#[test]
fn test_projects_and_contexts_with_their_open_counts() {
    let source = source_with(FILE);
    assert_eq!(
        list_projects_and_contexts(&source).unwrap(),
        "+home (1 open)\n+work (1 open)\n@office (1 open)\n@shop (1 open)"
    );
    remove(source);
}

#[test]
fn test_a_file_is_picked_by_name_or_path_and_others_are_refused() {
    let home = source_with("Buy milk\n");
    let work = Source {
        name: Some("work".to_owned()),
        ..source_with("Write report\n")
    };
    let both = || vec![home.clone(), work.clone()];
    assert_eq!(pick(both(), None).unwrap().path, home.path);
    assert_eq!(pick(both(), Some("work")).unwrap().path, work.path);
    let by_path = work.path.to_string_lossy().into_owned();
    assert_eq!(pick(both(), Some(&by_path)).unwrap().path, work.path);
    let error = pick(both(), Some("/etc/passwd")).unwrap_err();
    assert!(error.contains("todo.txt, work"), "{error}");
    remove(home);
    remove(work);
}
