//! The update check of rtmcli and rtmapp: reads the release list the website publishes
//! (`releases.json`) and tells whether a release newer than the running one exists on the user's
//! channel.
//!
//! A check is one anonymous, blocking GET: a fixed User-Agent and no version, identifier, cookie or
//! query string, with a 5 s limit on the whole call. Callers with a UI run it off the UI thread.

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use semver::Version;
use serde::Deserialize;

pub const URL: &str = "https://rtm.brcha.com/releases.json";

const TIMEOUT: Duration = Duration::from_secs(5);
const USER_AGENT: &str = "rtm-update-check";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Beta,
}

impl Channel {
    /// Whether a user on this channel is offered a release of channel `release`: the beta channel
    /// gets stable releases too.
    fn offers(self, release: Channel) -> bool {
        self == Channel::Beta || release == Channel::Stable
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("unknown channel {0:?}")]
pub struct ChannelParseError(String);

impl FromStr for Channel {
    type Err = ChannelParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "stable" => Ok(Channel::Stable),
            "beta" => Ok(Channel::Beta),
            _ => Err(ChannelParseError(s.to_string())),
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Channel::Stable => "stable",
            Channel::Beta => "beta",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub channel: Channel,
    /// The release's news post.
    pub url: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Network, TLS, timeout, or an HTTP error status.
    #[error("cannot fetch the release list: {0}")]
    Http(#[from] ureq::Error),
    /// Not JSON, or not the documented shape.
    #[error("cannot read the release list: {0}")]
    Json(#[from] serde_json::Error),
    /// A later format of the file; this version cannot tell whether it is up to date.
    #[error("the release list has schema {0}, which this version does not know")]
    Schema(u64),
    #[error("invalid running version: {0}")]
    Version(#[from] semver::Error),
}

#[derive(Deserialize)]
struct Schema {
    schema: u64,
}

#[derive(Deserialize)]
struct Feed {
    releases: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    version: String,
    channel: String,
    url: String,
}

/// The highest release in `feed` above `current` that `channel` is offered. Entries with an
/// unknown channel or an invalid version are skipped.
pub fn newest(feed: &str, current: &str, channel: Channel) -> Result<Option<Release>, Error> {
    let current = Version::parse(current)?;
    // The schema is read on its own first, so that a later format whose entries differ gives
    // `Error::Schema` rather than a parse error.
    let Schema { schema } = serde_json::from_str(feed)?;
    if schema != 1 {
        return Err(Error::Schema(schema));
    }
    let Feed { releases } = serde_json::from_str(feed)?;
    Ok(releases
        .into_iter()
        .filter_map(|entry| {
            let release_channel: Channel = entry.channel.parse().ok()?;
            let version = Version::parse(&entry.version).ok()?;
            (channel.offers(release_channel) && version > current).then_some((
                version,
                Release {
                    version: entry.version,
                    channel: release_channel,
                    url: entry.url,
                },
            ))
        })
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, release)| release))
}

/// The URL in `RTM_UPDATE_URL`, or [`URL`] when it is unset or empty.
pub fn url() -> String {
    std::env::var("RTM_UPDATE_URL")
        .ok()
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| URL.to_owned())
}

/// [`check_at`] the [`url`].
pub fn check(current: &str, channel: Channel) -> Result<Option<Release>, Error> {
    check_at(&url(), current, channel)
}

/// Fetches the release list at `url`, following redirects, and returns [`newest`] of it.
pub fn check_at(url: &str, current: &str, channel: Channel) -> Result<Option<Release>, Error> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(USER_AGENT)
        .build()
        .into();
    let feed = agent.get(url).call()?.body_mut().read_to_string()?;
    newest(&feed, current, channel)
}

#[cfg(test)]
#[path = "tests/lib_select_tests.rs"]
mod select_tests;
