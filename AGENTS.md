# AGENTS.md — Rusty Todo.txt Manager (rtm)

A [Todo.txt](https://github.com/todotxt/todo.txt) manager in Rust for GNU/Linux, macOS and Windows.

- Subparts: [`todotxt`](todotxt/AGENTS.md), [`rtmcli`](rtmcli/AGENTS.md), [`rtmapp`](rtmapp/AGENTS.md), [`rtmsync`](rtmsync/AGENTS.md), [`rtmupdate`](rtmupdate/AGENTS.md), [`rtmmcp`](rtmmcp/AGENTS.md), [`site`](site/AGENTS.md).
- Reference: [release process](doc/release-process.md), [Nix packaging](doc/nix-packaging.md), [CI](doc/ci.md), [desktop app](doc/rtmapp.md), [MCP eval](doc/mcp-eval.md), [website](doc/website.md).

## Conventions

- Rust edition 2024 in every crate; async only where a library needs it (rtmapp's file dialogs, `rmcp`), and never delaying a save.
- `todotxt` is the only code that parses or prints todo.txt lines and holds the shared logic (filters, tag matching, archive); frontends call it so they behave alike.
- Save right after every change and, for a synced file, commit right after the save; only rtmapp holds changes made during one of its sync jobs, then writes them in one commit.
- Every process writing a todo file, a pull included, holds `todotxt::WriteLock` on `rtmsync::lock_target(file)` from before the repository check until after the push.
- Every frontend refuses a todo file that is itself a symlink (`todotxt::refuse_symlink`), before locking, syncing or creating it.
- rtm's own commits are `rtm: <verb> <description>` (`add`, `complete`, `delete`, `archive`, …), the same in every frontend.
- The `update-check` feature of rtmcli and rtmapp is on by default and off in the Nix packages; test both builds (`--no-default-features`).
- **Branches:** `issues/RTM-NN/<slug>` from `main`, or from `releases/vYY.M` for a PR against a release branch; a cherry-pick to a release branch goes on `cherry/RTM-NN/<slug>`.
- **Commit subjects** and PR titles: `type(scope): summary -- RTM-NN`, one conventional type, exactly one scope; `.github/workflows/pr-title.yml` checks them, and a new scope goes there and here together.
  - `todotxt`, `rtmcli`, `rtmapp`, `rtmsync`, `rtmupdate`, `rtmmcp`: the crates (`rtmapp` with its packaging metadata).
  - `site`: the website, `scripts/site/`, `vercel.json`, its workflow and the release posts.
  - `release`: `release.yml`, `announce.yml`, the packages, `scripts/set-version.*`, `scripts/changelog.sh`, `scripts/announce.sh`, `cliff.toml`, `doc/release-process.md`, release PRs.
  - `nix`: `default.nix`, `flake.nix`, `shell.nix`, `doc/nix-packaging.md`.
  - `github`: the other workflows, the issue forms, the rulesets and `doc/ci.md`.
  - `workspace`: root `AGENTS.md`, `README.md`, the workspace `Cargo.toml` and `Cargo.lock`, `.claude/`, and changes no part dominates.
  - A change takes the scope of the part it is about; the type says what kind (`ci(github)`, not `ci(ci)`).
- **Tests:**
  - Unit tests: `<crate>/src/tests/<source stem>_<topic>_tests.rs`, declared at the source file's end as `#[cfg(test)] #[path = "tests/<file>"] mod <topic>_tests;`; no inline `mod tests`, `mod.rs` or plain `tests.rs`.
  - Helpers shared by one source file's topic files: `<stem>_support_tests.rs` as `mod support_tests;`, items `pub(super)`; a source in a subdirectory mirrors it under `src/tests/`.
  - Integration tests: `<crate>/tests/<area>_<topic>_tests.rs`; rtmcli's run `CARGO_BIN_EXE_rtmcli`, never `cargo run`; rtmapp, a binary, has unit tests only, UI tests included.
  - Tests use scratch directories under the system temp directory, never the user's `rtm/config.toml` or todo files, and a test running a program removes `TODOTXT`, `RTM_NO_SYNC`, `RTM_VCS`, `RTM_UPDATE_URL` and `RTM_ARCHIVE_DAYS`.
  - Git tests give each scratch repository a local identity, `commit.gpgsign=false`, an empty `core.hooksPath` and `rerere.enabled=false`, with local bare remotes, never the network.
- **Epic close:** before closing an epic that touches `rtmmcp/`, `todotxt/src/` or the `mcp` entry points, run the [MCP eval](doc/mcp-eval.md) and report its table in the epic's last PR; a failing scenario becomes its own issue.

## Important Notes

- AGENTS.md files hold one-line conventions and constraints only; detailed reference goes to `doc/`.
- Run cargo in the dev shell (`nix develop`) on NixOS.
- **Jira** (https://brcha.atlassian.net, space RTM): Open → In Progress → Code Review when the PR opens → Done after the merge; To Verify is for manual checks, never review.
- Jira work types Epic, Story, Task, Bug, Spike; an epic is the parent field; links "depends on", "blocks", "relates to"; access through the Atlassian MCP or `~/.config/rtm-jira/env`.
- GitHub issues are user reports from `.github/ISSUE_TEMPLATE/` (change with `site/content/report.md`); work from one gets a Jira issue, the PR may say `Closes #N`; never link the private Jira in user-facing text.
- `vercel[bot]` comments are status notices, not review: ignore them unless a website PR's deployment failed.
- `claude[bot]` comments (`claude-review.yml`) are review feedback: fix what holds, reply where you disagree.
- **Native dependency sync:** a native library a crate links goes into both `rust.yml` (a hardened apt step, see [CI](doc/ci.md)) and `default.nix`'s `eguiDeps` (and `shell.nix` if the dev-shell build needs it).
- Libraries rtmapp loads with `dlopen` go only into `eguiDeps`, the deb's `depends`/`suggests` and the RPM's `requires`/`suggests` by soname ([desktop app](doc/rtmapp.md)), never into an apt step.
- Never trim the Nix lists to match apt's; test tools such as pijul go into `nativeCheckInputs` and the dev shell, never `eguiDeps` or the packages.

## Future Plans

- See README's "General plan".
