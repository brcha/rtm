use super::*;

fn owned(words: &[(WordKind, &str)]) -> Vec<(WordKind, String)> {
    (words.iter())
        .map(|&(kind, text)| (kind, text.to_owned()))
        .collect()
}

#[test]
fn words_come_in_written_order_with_their_kinds() {
    let item: TodoItem =
        "(A) 2026-01-02 Prepare +Horizon for @SerbianWeekly due:2026-02-01 rec:1w t:2026-01-20 h:1"
            .parse()
            .unwrap();
    use WordKind::*;
    assert_eq!(
        item.words(),
        owned(&[
            (Word, "Prepare"),
            (Project, "+Horizon"),
            (Word, "for"),
            (Context, "@SerbianWeekly"),
            (Due, "due:2026-02-01"),
            (Recurrence, "rec:1w"),
            (Threshold, "t:2026-01-20"),
            (Word, "h:1"),
        ])
    );
}

#[test]
fn the_head_is_left_out_and_tags_of_a_done_line_are_named() {
    let uuid = "123e4567-e89b-12d3-a456-426614174000";
    let line = format!("x 2026-01-03 2026-01-02 Pay rent pri:B uuid:{uuid} sub:{uuid}");
    let item: TodoItem = line.parse().unwrap();
    use WordKind::*;
    assert_eq!(
        item.words(),
        owned(&[
            (Word, "Pay"),
            (Word, "rent"),
            (Pri, "pri:B"),
            (Uuid, &*format!("uuid:{uuid}")),
            (Sub, &*format!("sub:{uuid}")),
        ])
    );
}

#[test]
fn an_unreadable_tag_is_a_word() {
    let item: TodoItem = "Ship due:soon".parse().unwrap();
    assert_eq!(
        item.words(),
        owned(&[(WordKind::Word, "Ship"), (WordKind::Word, "due:soon")])
    );
}

#[test]
fn an_item_changed_since_it_was_read_gives_its_new_line() {
    let item: TodoItem = "Call +Mom about @home".parse().unwrap();
    let item = item.add_project(TodoProject {
        name: "Dad".to_owned(),
    });
    let item = item.set_description("Call about it".to_owned());
    use WordKind::*;
    assert_eq!(
        item.words(),
        owned(&[
            (Word, "Call"),
            (Project, "+Mom"),
            (Word, "about"),
            (Word, "it"),
            (Context, "@home"),
            (Project, "+Dad"),
        ])
    );
}

#[test]
fn an_item_built_in_code_has_words_too() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Water plants".to_owned(),
        projects: vec![TodoProject {
            name: "Home".to_owned(),
        }],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
        layout: None,
    };
    assert_eq!(
        item.words(),
        owned(&[
            (WordKind::Word, "Water"),
            (WordKind::Word, "plants"),
            (WordKind::Project, "+Home"),
        ])
    );
}
