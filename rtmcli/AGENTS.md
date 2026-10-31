# AGENTS.md — rtmcli (CLI frontend)

## What This Component Is

A command-line interface for the Todo.txt manager. Uses `clap` for argument parsing, the
`todotxt` library for all data operations, and `rtmsync` to sync the file through its Git or Pijul
repository (RTM-21).

---

## Architecture / Structure

```
rtmcli/src/
└── main.rs     ← Single file: CLI definition + all command handlers
rtmcli/tests/   ← Integration tests (sync_test.rs: sync through git)
```

---

## Commands

| Command    | Description                                              |
|------------|----------------------------------------------------------|
| `list`     | List items, optionally filtered by completion and date   |
| `add`      | Add a new item in Todo.txt format                        |
| `complete` | Complete items by filter, index, or UUID                 |
| `delete`   | Delete an item and its sub-items, after confirmation     |
| `sync`     | Pull and push the repository holding the file            |

---

## Conventions

- **Edition:** Rust 2024
- File path resolved from: `-f <file>` flag → `$TODOTXT` env var → `todo.txt` (cwd fallback).
- Filters: `today`, `week`, `all`, `overdue`, `future` (date-range based).
- `complete` accepts: a filter name (optionally + index/UUID), an absolute index, or a UUID.
- `delete` takes one item: a number as shown by `list` (with `-c`, as shown by `list -c`) or a
  UUID (open or completed; `-c` does not apply). It also removes the item's sub-items
  (`TodoLibrary::remove_item`), prints every line it will remove, and asks `Delete? [y/N]`;
  anything but `y`/`yes`, including no input, exits 0 without writing. `-y`/`--yes` skips the
  prompt.
- No config file — stateless, all options via CLI args/env.

## Sync (RTM-21, RTM-63)

- `Repo::find` decides, before the command runs: `--no-sync` / `RTM_NO_SYNC` (clap's boolish
  parser: `1`/`0`, `true`/`false`, `yes`/`no`, `on`/`off`) → no sync and no commits.
  `rtmsync::detect`: none → no sync; one → it; both Git and Pijul → `--vcs` / `RTM_VCS` picks, else
  exit 2 (a `--vcs` that the directory lacks is also exit 2). Then the file's state
  (`Vcs::state`, RTM-63, the RTM-57 decision): ignored → no sync, silently; untracked → no sync, with
  a hint naming `--commit-changes` and that ignoring the file silences it (so rtmcli in a project
  directory never adds a stray `todo.txt` on its own); a conflicted repository (`Vcs::is_conflicted`)
  → exit 1 before loading, and before any commit; modified (changes made by hand or with
  `--no-sync`) → exit 1, saying to commit or revert them or use the flag. `--commit-changes` (global,
  deliberately no environment variable) answers yes: modified → commit `rtm: commit outside
  changes`, untracked → commit `rtm: track <name>`, then the command runs; a failed commit exits 1.
  A missing file skips the state check (synced when tracked, as before): `state` would call a
  deleted tracked file modified, and the flag would commit the deletion of every item. Finally
  `has_remote` decides whether to pull and push. A failed check (e.g. `git` not installed) is a
  "Not syncing" warning. Exit 1 is for a state the user must fix, exit 2 for a usage error.
- Every command syncs (pull, then push) first when there is a remote. `add`, `complete` and
  `delete` then commit (`rtm: add <description>`, `rtm: complete <item>` or `rtm: complete <n>
  items`, `rtm: delete <item>`) and sync again (`Repo::commit_and_sync`). Without a remote they only
  commit, silently.
- `complete` and `delete` resolve their items against the file **before** the pull, then find the
  same items (equal lines) after it; if one is gone or changed, nothing is written and rtmcli exits
  1, printing the item's new text when its UUID still exists.
- A failed pull or push (offline, timeout, rejected push) is a "Not synced" warning; the change is
  still saved and committed. `Conflict` exits 1 with "resolve them by hand" — also after a saved
  and committed change, when the push afterwards meets a conflict. `rtmcli sync` exits 1 whenever it
  cannot sync.
- Tests: `tests/sync_test.rs`, against scratch bare git remotes and clones (local paths, no network)
  with local config that keeps the user's identity, signing, hooks and rerere out. Pijul is covered
  in `rtmsync`; the CLI logic does not depend on the VCS.

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
  (see [`doc/release-process.md`](../doc/release-process.md)), so a CalVer bump reaches the Debian package version with no extra
  step.
