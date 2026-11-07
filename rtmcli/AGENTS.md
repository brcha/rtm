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
rtmcli/tests/   ← Integration tests, running the binary on scratch files
├── cli_commands_tests.rs  ← add, list, complete, delete
├── cli_filter_tests.rs    ← The due-date filters of list and complete
├── cli_file_tests.rs      ← Finding the file: -f, $TODOTXT, todo.txt in the working directory
├── cli_lock_tests.rs      ← The write lock shared with rtmapp and the MCP server
├── cli_sync_tests.rs      ← Sync through git
├── cli_tags_tests.rs      ← projects and contexts, and filtering list by them
└── cli_update_tests.rs    ← check-update against a local HTTP server
```

---

## Commands

| Command        | Description                                              |
|----------------|----------------------------------------------------------|
| `list`         | List items, filtered by completion, date and tags        |
| `projects`     | List the projects with their open-item counts (RTM-122)  |
| `contexts`     | List the contexts with their open-item counts (RTM-122)  |
| `add`          | Add a new item in Todo.txt format                        |
| `complete`     | Complete items by filter, index, or UUID                 |
| `delete`       | Delete an item and its sub-items, after confirmation     |
| `sync`         | Pull and push the repository holding the file            |
| `check-update` | Check whether a newer release of rtm exists              |

---

## Conventions

- **Edition:** Rust 2024
- File path resolved from: `-f <file>` flag → `$TODOTXT` env var → `todo.txt` (cwd fallback).
- Filters: `today`, `week`, `all`, `overdue`, `future` (date-range based).
- `list` takes any number of filter terms, in any order (RTM-123): at most one date filter, plus
  `+project` and `@context` terms. An item must match all of them (`TodoItem::matches`, see
  `todotxt/AGENTS.md`). Rows are numbered `1..n` within the filter. An unknown term, a bare `+` or
  `@`, or a second date filter is a usage error (exit 2). `complete` and `delete` take no tags.
- `projects` / `contexts` sync first and load like `list`, then print `<tag> (<open items>)`
  lines from `TodoLibrary::projects` / `contexts` (order, case merging and `(0)` for done-only
  tags come from `todotxt`, see `todotxt/AGENTS.md`). They never write.
- `complete` accepts: a filter name (optionally + index/UUID), an absolute index, or a UUID.
- `delete` takes one item: a number as shown by `list` (with `-c`, as shown by `list -c`) or a
  UUID (open or completed; `-c` does not apply). It also removes the item's sub-items
  (`TodoLibrary::remove_item`), prints every line it will remove, and asks `Delete? [y/N]`;
  anything but `y`/`yes`, including no input, exits 0 without writing. `-y`/`--yes` skips the
  prompt.
- No config file — stateless, all options via CLI args/env.

## Update check (RTM-137)

- `check-update [--channel stable|beta]` asks `rtmupdate` (see `rtmupdate/AGENTS.md`) whether a
  newer release exists on the channel (default `stable`; `beta` is also offered betas):
  - a newer release: `rtm <version> is available: <url>`, exit 0;
  - none: `rtmcli <version> is up to date`, exit 0;
  - a failed check (offline, an HTTP error, an unreadable or unknown file): `Error: ...` on stderr,
    exit 1.
- It is handled first in `main()`, before the file name is resolved and `Repo::find` runs: it
  reads no file, detects no repository and syncs nothing. It is the only command that uses the
  network; no other command checks for updates.
- `RTM_UPDATE_URL` replaces the release list's URL (the tests point it at a local server).
- The default cargo feature `update-check` pulls in `rtmupdate`. The Nix package builds without
  it (`buildNoDefaultFeatures`, see `doc/nix-packaging.md`); there `check-update` says the build
  has no update check and to update through the package manager, exit 0. `cli_update_tests.rs`
  covers both builds (`cargo test -p rtmcli --no-default-features`).

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
- Tests: `tests/cli_sync_tests.rs`, against scratch bare git remotes and clones (local paths, no
  network) with local config that keeps the user's identity, signing, hooks and rerere out. Pijul is
  covered in `rtmsync`; the CLI logic does not depend on the VCS.
- **The write lock (RTM-158)** is the one rtmapp and the MCP server take (`todotxt::WriteLock` on
  `rtmsync::lock_target(file)`: the file's repository directory by its markers, whether or not it
  syncs, else the file; RTM-142). Every command that touches the file (all but `check-update`)
  takes it in `main` before `Repo::find`, which may commit and reads the file's state (another rtm
  process between its save and its commit would otherwise look like an outside change), and holds
  it until it ends, its commit and push included. rtmcli waits for it. `delete` releases it while
  it asks `Delete? [y/N]`, so no other process waits on a person; after a yes it takes it again,
  runs `Repo::find` again (an editor may have left changes, or a pull a conflict, meanwhile: exit 1
  unless `--commit-changes`, as at the start), reads the file again and finds the item again
  (`find_again`), and deletes nothing (exit 1) when the item or its sub-items changed meanwhile. A
  lock that cannot be taken: a file whose directory does not exist goes on without it (the command
  reports its usual error); a command that writes, or may pull (sync on and the file in a
  repository), exits 1; any other read only warns. Tests: `tests/cli_lock_tests.rs` (they keep
  `XDG_RUNTIME_DIR`, so test and rtmcli share the lock directory).

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
