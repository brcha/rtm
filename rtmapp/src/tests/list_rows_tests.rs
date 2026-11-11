use super::*;

fn item(line: &str) -> TodoItem {
    line.parse().unwrap()
}

/// The text of each tag span in `item`'s row, with no selection.
fn tag_texts(item: &TodoItem) -> Vec<(RowTag, String)> {
    let (job, tags) = row_job(&theme::LIGHT, item, 400.0, &[], &[]);
    tags.iter()
        .map(|span| {
            let text: String = (job.text.chars().skip(span.chars.start))
                .take(span.chars.len())
                .collect();
            (span.tag, text)
        })
        .collect()
}

#[test]
fn tag_spans_cover_the_tag_without_the_space_before_it() {
    let item = item("(A) Write the report +work @office due:2026-11-01");
    assert_eq!(
        tag_texts(&item),
        [
            (RowTag::Project(0), "+work".to_owned()),
            (RowTag::Context(0), "@office".to_owned())
        ]
    );
}

#[test]
fn tag_spans_count_characters_not_bytes() {
    let item = item("Čišćenje kuće +kuća @dvorište +posao");
    assert_eq!(
        tag_texts(&item),
        [
            (RowTag::Project(0), "+kuća".to_owned()),
            (RowTag::Project(1), "+posao".to_owned()),
            (RowTag::Context(0), "@dvorište".to_owned())
        ]
    );
}

#[test]
fn done_items_have_tag_spans_too() {
    let item = item("x 2026-01-01 Plant bulbs +garden");
    assert_eq!(
        tag_texts(&item),
        [(RowTag::Project(0), "+garden".to_owned())]
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
    let start = item.description.find("https").unwrap();
    let end = start + "https://kuća.example/a".len();
    assert_eq!(
        tag_texts(&item),
        [
            (
                RowTag::Link { start, end },
                "https://kuća.example/a".to_owned()
            ),
            (RowTag::Project(0), "+home".to_owned())
        ]
    );
    let link = (job.sections.iter())
        .find(|s| job.text[s.byte_range.start.0..s.byte_range.end.0] == *"https://kuća.example/a")
        .unwrap();
    assert_eq!(link.format.color, theme::LIGHT.link);
    assert_eq!(link.format.underline, Stroke::new(1.0, theme::LIGHT.link));
}
