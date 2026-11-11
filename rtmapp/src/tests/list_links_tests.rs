use super::*;

/// The text of each link `links` finds in `text`.
fn found(text: &str) -> Vec<&str> {
    links(text).into_iter().map(|range| &text[range]).collect()
}

#[test]
fn http_https_and_mailto_are_links_in_any_case() {
    assert_eq!(
        found("See http://a.example and HTTPS://b.example/x?y=1 or mailto:me@c.example"),
        [
            "http://a.example",
            "HTTPS://b.example/x?y=1",
            "mailto:me@c.example"
        ]
    );
}

#[test]
fn trailing_punctuation_is_not_part_of_the_link() {
    assert_eq!(
        found("Read https://a.example/ticket/42. Then https://b.example, \"https://c.example\"!"),
        [
            "https://a.example/ticket/42",
            "https://b.example",
            "https://c.example"
        ]
    );
}

#[test]
fn a_closing_parenthesis_stays_only_when_it_is_balanced() {
    assert_eq!(
        found("(see https://a.example) and https://en.wikipedia.org/wiki/Rust_(language)."),
        [
            "https://a.example",
            "https://en.wikipedia.org/wiki/Rust_(language)"
        ]
    );
}

#[test]
fn a_link_may_follow_a_bracket_or_a_key() {
    assert_eq!(
        found("<https://a.example> url:https://b.example"),
        ["https://a.example", "https://b.example"]
    );
}

#[test]
fn other_schemes_bare_hosts_and_glued_schemes_are_not_links() {
    assert_eq!(
        found("file:///etc/passwd www.example.com xhttps://a.example https:// mailto:"),
        Vec::<&str>::new()
    );
}

#[test]
fn ranges_are_bytes_after_non_ascii_text() {
    let text = "Čišćenje kuće https://kuća.example/ž";
    let start = text.find("https").unwrap();
    assert_eq!(links(text).first(), Some(&(start..text.len())));
    assert_eq!(found(text), ["https://kuća.example/ž"]);
}
