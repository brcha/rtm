# AGENTS.md — rtmupdate

Checks the website's `releases.json` for a newer release on the user's channel.

- The `schema` field is read first; an unknown one is an error, never "up to date".
- The request sends nothing identifying: the fixed User-Agent `rtm-update-check`, no version, cookie or query string.
- `ureq` with rustls and bundled webpki-roots: no async runtime, no OpenSSL, no OS certificate store.
- `RTM_UPDATE_URL` overrides the URL through `url()`; the crate's own tests call `check_at` and never set it.
- Tests never use the network: they serve canned answers from a local `TcpListener`.
