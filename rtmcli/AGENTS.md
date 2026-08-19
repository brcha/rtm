# AGENTS.md — rtmcli (CLI frontend)

## What This Component Is

A command-line interface for the Todo.txt manager. Uses `clap` for argument parsing and the
`todotxt` library for all data operations.

---

## Architecture / Structure

```
rtmcli/src/
└── main.rs     ← Single file: CLI definition + all command handlers
rtmcli/tests/   ← Integration tests
```

---

## Commands

| Command    | Description                                              |
|------------|----------------------------------------------------------|
| `list`     | List items, optionally filtered by completion and date   |
| `add`      | Add a new item in Todo.txt format                        |
| `complete` | Complete items by filter, index, or UUID                 |

---

## Conventions

- **Edition:** Rust 2024
- File path resolved from: `-f <file>` flag → `$TODOTXT` env var → `todo.txt` (cwd fallback).
- Filters: `today`, `week`, `all`, `overdue`, `future` (date-range based).
- `complete` accepts: a filter name (optionally + index/UUID), an absolute index, or a UUID.
- No config file — stateless, all options via CLI args/env.

---

## Build & Run

```sh
cargo build -p rtmcli
cargo run -p rtmcli -- -f ~/todo.txt list today
cargo test -p rtmcli
```

---

## Debian Packaging (RTM-8)

- `Cargo.toml` carries `description`, `license = "MIT"`, and `repository` in `[package]` —
  `cargo-deb` requires them and errors without. Do not remove these as "unused" metadata; they
  are load-bearing for the release build, not decorative.
- `[package.metadata.deb]` pins `name = "rtmcli"` explicitly (matching the crate name, but not
  left to derive by accident), `section = "utils"`, `priority = "optional"`, and an `assets`
  list mapping the release binary to `/usr/bin/rtmcli` and the workspace-root `LICENSE` to
  `/usr/share/doc/rtmcli/copyright` — the Debian-conventional location.
- Built via `cargo deb -p rtmcli --no-build` in the release workflow, after a separate
  `cargo build --release -p rtmcli` step (so the binary being packaged is exactly the one
  already built and tested, not a second, independently-triggered build).
- `scripts/set-version.ps1` already scopes its edit to `rtmcli/Cargo.toml`'s `[package]` table
  (see root `AGENTS.md`), so a CalVer bump reaches the Debian package version with no extra
  step.
