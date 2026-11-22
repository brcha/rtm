use super::*;

fn item(line: &str) -> TodoItem {
    line.parse().unwrap()
}

fn project(name: &str) -> RowTag {
    RowTag::Project(TodoProject {
        name: name.to_owned(),
    })
}

fn context(name: &str) -> RowTag {
    RowTag::Context(TodoContext {
        name: name.to_owned(),
    })
}

fn row_text(item: &TodoItem) -> String {
    row_job(&theme::LIGHT, item, 400.0, &[], &[]).0.text
}

/// The text of each tag span in `item`'s row, with no selection.
fn tag_texts(item: &TodoItem) -> Vec<(RowTag, String)> {
    let (job, tags) = row_job(&theme::LIGHT, item, 400.0, &[], &[]);
    tags.iter()
        .map(|span| {
            let text: String = (job.text.chars().skip(span.chars.start))
                .take(span.chars.len())
                .collect();
            (span.tag.clone(), text)
        })
        .collect()
}

#[test]
fn tag_spans_cover_the_tag_without_the_space_before_it() {
    let item = item("(A) Write the report +work @office due:2026-11-01");
    assert_eq!(
        tag_texts(&item),
        [
            (project("work"), "+work".to_owned()),
            (context("office"), "@office".to_owned())
        ]
    );
}

#[test]
fn tag_spans_count_characters_not_bytes() {
    let item = item("Čišćenje kuće +kuća @dvorište +posao");
    assert_eq!(
        tag_texts(&item),
        [
            (project("kuća"), "+kuća".to_owned()),
            (context("dvorište"), "@dvorište".to_owned()),
            (project("posao"), "+posao".to_owned())
        ]
    );
}

#[test]
fn done_items_have_tag_spans_too() {
    let item = item("x 2026-01-01 Plant bulbs +garden");
    assert_eq!(
        tag_texts(&item),
        [(project("garden"), "+garden".to_owned())]
    );
}

#[test]
fn selected_tags_are_bold_whatever_their_case() {
    let item = item("Call the client +Work @phone");
    let selected = [TodoProject {
        name: "work".into(),
    }];
    let (job, tags) = row_job(&theme::LIGHT, &item, 400.0, &selected, &[]);
    // The font of the section holding a tag's first character.
    let font = |span: &TagSpan| {
        let byte = job.text.char_indices().nth(span.chars.start).unwrap().0;
        let section = (job.sections.iter())
            .find(|s| (s.byte_range.start.0..s.byte_range.end.0).contains(&byte))
            .unwrap();
        section.format.font_id.clone()
    };
    assert_eq!(font(&tags[0]), theme::bold(11.9));
    assert_eq!(font(&tags[1]), FontId::proportional(11.9));
}

#[test]
fn link_spans_cover_the_url_and_are_underlined_in_the_link_colour() {
    let item = item("Čišćenje url:https://kuća.example/a, then call +home");
    let (job, _) = row_job(&theme::LIGHT, &item, 400.0, &[], &[]);
    assert!(
        job.text
            .starts_with("Čišćenje url:https://kuća.example/a, then call")
    );
    let url = "https://kuća.example/a";
    assert_eq!(
        tag_texts(&item),
        [
            (RowTag::Link(url.to_owned()), url.to_owned()),
            (project("home"), "+home".to_owned())
        ]
    );
    let link = (job.sections.iter())
        .find(|s| job.text[s.byte_range.start.0..s.byte_range.end.0] == *"https://kuća.example/a")
        .unwrap();
    assert_eq!(link.format.color, theme::LIGHT.link);
    assert_eq!(link.format.underline, Stroke::new(1.0, theme::LIGHT.link));
}

#[test]
fn a_row_reads_as_its_line_is_written() {
    let item = item(
        "(A) 2026-01-02 Prepare presentation on +Horizon and @DDI-O for @SerbianWeekly \
         due:2026-02-01 rec:1w t:2026-01-20 h:1",
    );
    assert_eq!(
        row_text(&item),
        "(A) Prepare presentation on +Horizon and @DDI-O for @SerbianWeekly due:2026-02-01 \
         rec:1w t:2026-01-20 h:1"
    );
    assert_eq!(
        tag_texts(&item),
        [
            (project("Horizon"), "+Horizon".to_owned()),
            (context("DDI-O"), "@DDI-O".to_owned()),
            (context("SerbianWeekly"), "@SerbianWeekly".to_owned())
        ]
    );
}

#[test]
fn the_head_and_the_sub_item_links_are_left_out() {
    let uuid = "123e4567-e89b-12d3-a456-426614174000";
    let item = item(&format!(
        "x 2026-01-03 2026-01-02 Pay uuid:{uuid} rent sub:{uuid} pri:B +home"
    ));
    assert_eq!(row_text(&item), "(B) Pay rent +home");
}

#[test]
fn a_link_in_the_middle_of_the_line_keeps_the_tags_around_it() {
    let item = item("Read +rust https://doc.rust-lang.org/book/ @desk");
    let url = "https://doc.rust-lang.org/book/";
    assert_eq!(row_text(&item), format!("Read +rust {url} @desk"));
    assert_eq!(
        tag_texts(&item),
        [
            (project("rust"), "+rust".to_owned()),
            (RowTag::Link(url.to_owned()), url.to_owned()),
            (context("desk"), "@desk".to_owned())
        ]
    );
}
