use super::*;

fn entry(version: &str, channel: &str) -> String {
    format!(
        r#"{{"version": "{version}", "channel": "{channel}", "date": "2026-11-05", "url": "https://rtm.brcha.com/news/{version}/"}}"#
    )
}

fn feed(entries: &[(&str, &str)]) -> String {
    let releases: Vec<String> = entries.iter().map(|(v, c)| entry(v, c)).collect();
    format!(r#"{{"schema": 1, "releases": [{}]}}"#, releases.join(", "))
}

fn offered(entries: &[(&str, &str)], current: &str, channel: Channel) -> Option<String> {
    newest(&feed(entries), current, channel)
        .expect("a valid feed")
        .map(|release| release.version)
}

#[test]
fn stable_offers_highest_newer_stable() {
    let entries = [
        ("26.3.0", "stable"),
        ("26.2.0", "stable"),
        ("26.1.0", "stable"),
    ];
    let release = newest(&feed(&entries), "26.1.0", Channel::Stable).unwrap();
    assert_eq!(
        release,
        Some(Release {
            version: "26.3.0".into(),
            channel: Channel::Stable,
            url: "https://rtm.brcha.com/news/26.3.0/".into(),
        })
    );
}

#[test]
fn nothing_when_running_newest_or_newer() {
    let entries = [("26.2.0", "stable"), ("26.1.0", "stable")];
    assert_eq!(offered(&entries, "26.2.0", Channel::Stable), None);
    assert_eq!(offered(&entries, "26.3.0", Channel::Stable), None);
}

#[test]
fn beta_ignored_on_stable_offered_on_beta() {
    let entries = [("26.3.0-beta.1", "beta"), ("26.2.0", "stable")];
    assert_eq!(offered(&entries, "26.2.0", Channel::Stable), None);
    assert_eq!(
        offered(&entries, "26.2.0", Channel::Beta).as_deref(),
        Some("26.3.0-beta.1")
    );
}

#[test]
fn beta_sorts_below_its_release() {
    let entries = [("26.2.0", "stable"), ("26.2.0-beta.2", "beta")];
    for channel in [Channel::Stable, Channel::Beta] {
        assert_eq!(
            offered(&entries, "26.2.0-beta.1", channel).as_deref(),
            Some("26.2.0")
        );
        assert_eq!(offered(&entries, "26.2.0", channel), None);
    }
}

#[test]
fn older_line_patches() {
    let entries = [
        ("26.1.2", "stable"),
        ("26.2.0", "stable"),
        ("26.1.1", "stable"),
    ];
    assert_eq!(
        offered(&[("26.1.2", "stable")], "26.2.0", Channel::Stable),
        None
    );
    assert_eq!(
        offered(&entries, "26.1.1", Channel::Stable).as_deref(),
        Some("26.2.0")
    );
}

#[test]
fn unknown_channels_and_invalid_versions_skipped() {
    let entries = [
        ("26.4.0", "nightly"),
        ("26.5", "stable"),
        ("26.2.0", "stable"),
    ];
    assert_eq!(
        offered(&entries, "26.1.0", Channel::Beta).as_deref(),
        Some("26.2.0")
    );
}

#[test]
fn unknown_fields_ignored() {
    let feed = r#"{"schema": 1, "mirror": "x", "releases": [
        {"version": "26.2.0", "channel": "stable", "url": "u", "size": 3}
    ]}"#;
    let release = newest(feed, "26.1.0", Channel::Stable).unwrap();
    assert_eq!(release.map(|r| r.version).as_deref(), Some("26.2.0"));
}

#[test]
fn unknown_schema_read_before_entries() {
    let feed = r#"{"schema": 2, "releases": {"stable": "26.2.0"}}"#;
    let result = newest(feed, "26.1.0", Channel::Stable);
    assert!(matches!(result, Err(Error::Schema(2))), "{result:?}");
}

#[test]
fn malformed_feed_is_json_error() {
    for feed in [
        "<html>",
        r#"{"releases": []}"#,
        r#"{"schema": 1, "releases": [{"channel": "stable", "url": "u"}]}"#,
    ] {
        let result = newest(feed, "26.1.0", Channel::Stable);
        assert!(matches!(result, Err(Error::Json(_))), "{feed}: {result:?}");
    }
}

#[test]
fn invalid_running_version() {
    let result = newest(&feed(&[]), "26.1", Channel::Stable);
    assert!(matches!(result, Err(Error::Version(_))), "{result:?}");
}

#[test]
fn channel_round_trip() {
    for channel in [Channel::Stable, Channel::Beta] {
        assert_eq!(channel.to_string().parse(), Ok(channel));
    }
    assert_eq!(
        "Beta".parse::<Channel>(),
        Err(ChannelParseError("Beta".into()))
    );
    assert!("nightly".parse::<Channel>().is_err());
}
