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
- **The `assets` source path (`target/release/rtmcli`) is hardcoded like that on purpose — do
  not "fix" it.** Per `cargo-deb`'s own documentation, it always wants the `target/release/`
  prefix in asset source paths, *even when that isn't the crate's real target directory* — it
  detects that literal prefix and substitutes the actual path, correctly handling workspaces,
  cross-compilation, and `CARGO_TARGET_DIR`. Replacing it with a workspace-relative path (e.g.
  because this crate is a workspace member and its real output lives at the workspace root)
  breaks packaging: cargo-deb will build stale files and mishandle debug info.
- Built via `cargo deb -p rtmcli --no-build --output <path>` in the release workflow, after a
  separate `cargo build --release -p rtmcli` step (so the binary being packaged is exactly the
  one already built and tested, not a second, independently-triggered build). **`--output` is
  required, not cosmetic (RTM-30):** `cargo deb`'s default output directory is `target/debian/`
  — not `target/release/debian/`, which is where an initial version of this workflow
  incorrectly assumed. Naming the destination explicitly removes any dependence on guessing
  cargo-deb's internal default.
- `scripts/set-version.ps1` already scopes its edit to `rtmcli/Cargo.toml`'s `[package]` table
  (see root `AGENTS.md`), so a CalVer bump reaches the Debian package version with no extra
  step.
