# AGENTS.md — rtmsync (sync library)

## What This Component Is

The library that synchronises a todo.txt file through the version control system it lives in
(epic RTM-1). Git and Pijul are in scope; no others. No UI, no binary; `thiserror` is the only
dependency.

Done: detecting the VCS (RTM-12), committing the file (RTM-18), syncing with the remote (RTM-19,
which also took Pijul's part from RTM-53), the checks the frontends need and sync in the CLI
(RTM-21), sync in the GUI and the `index.lock` retry (RTM-64), the todo.txt's state for the check on
open and its use in the CLI (RTM-63, decided in RTM-57), background GUI sync replacing RTM-64's
per-change sync (RTM-20), the GUI's check on open (RTM-70).

---

## Architecture / Structure

```
rtmsync/
├── src/lib.rs        ← Vcs enum, detect(), Vcs::{commit, sync, state, is_tracked,
│                       is_conflicted, has_remote}, State, Error, unit tests
└── tests/
    ├── git.rs        ← Vcs::commit, Vcs::state and Vcs::sync with Git
    └── pijul.rs      ← Vcs::commit, Vcs::state and Vcs::sync with Pijul (a single test, see below)
```

---

## Conventions

- **Edition:** Rust 2024. No async.
- **Detection is markers only.** `detect(path) -> Vec<Vcs>` walks up from the file's directory and
  returns every VCS whose marker (`.git`, `.pijul`) is in the nearest directory that has any. Empty
  means not versioned; more than one means the caller asks the user which to use. It runs no
  executables and checks nothing else: tracked files, upstreams and repository state belong to the
  subtasks that sync.
- `.git` is tested with `exists`, not `is_dir`: it is a file in linked worktrees and submodules.
- The file's symlinks are resolved first, so a link into a repository counts as versioned. A file
  that does not exist yet is made absolute instead, so the walk still covers its parents.
- **VCSs are driven through their executables** (`git`, `pijul`), decided in RTM-16 and RTM-17: no
  Git or Pijul library is linked. A user with a repository has its executable.
- **`Vcs::commit(file, message)` commits only that file.** Git: `add`, a staged-changes check, then
  a commit with the file as pathspec, so what else the user staged stays staged. Pijul: `add`, then
  `record --all` with the file as prefix. An untracked file is added; an unchanged file commits
  nothing. The caller chooses the message (rtmcli: `rtm: add <description>` and the like).
- Executables run in the file's directory with the file name after `--` (`--literal-pathspecs` for
  git, except `check-ignore`, which takes path names and refuses it), stdin null (prompts on stdin fail), and `CREATE_NO_WINDOW` on Windows. Only network calls
  are timed (below), so `commit` can still wait on a terminal prompt such as pinentry. Errors:
  `Spawn` (could not start; `NotFound` usually means not in `PATH`, but also a missing directory),
  `Failed` (non-zero exit, with stderr), `Conflict` and `Timeout` (below). The only output parsed is
  git's porcelain v2 status, the plain path lists of `rev-parse --git-path` and `pijul ls`, the
  `U <path>` and `A <path>` lines of `pijul diff --short --untracked`, and whether `pijul remote` or
  `pijul diff --short` prints anything; none depends on the locale, so no `LC_ALL=C`.
- **`Vcs::sync(file)` pulls, then pushes only if the pull applied cleanly.** Git: `pull --rebase
  --autostash`, so local commits are rebased onto the remote; any unmerged (`u`) path in `status
  --porcelain=v2` afterwards is `Conflict`, and the stopped rebase is left for the user (rtm never
  aborts it). Pijul: `pull --all`, which on a conflict exits 0 and writes markers (`>>>>>>> `,
  `======= `, `<<<<<<< `) into the file, so the todo.txt is checked for them, even after a failed
  pull, and any is `Conflict`. Then `push` / `push --all`. A missing upstream is `Failed`. The
  conflict check is `Vcs::is_conflicted`, which for Git also counts a rebase or merge stopped
  half-way (`rebase-merge`, `rebase-apply`, `MERGE_HEAD` via `git rev-parse --git-path`): after a
  conflict the user may resolve and stage every path yet not run `git rebase --continue`, and a
  commit then would land mid-rebase.
- **Checks for the frontends:** `is_tracked` (Git: `ls-files --error-unmatch`; Pijul: `pijul ls`
  lists the root-relative path), `is_conflicted` (above), `has_remote` (Git: an upstream branch;
  Pijul: `pijul remote` prints anything). During a stopped rebase HEAD is detached and has no
  upstream, so callers check `is_conflicted` on its own rather than relying on `sync` to notice.
- **`Vcs::state(file)` (RTM-63):** `Clean`, `Modified` (uncommitted changes, staged or not, or
  added and never committed), `Untracked` or `Ignored`. Git: `is_tracked`, then porcelain v2 status
  of the file (`1`, `2` or `u` lines) for a tracked one, or `check-ignore -q` (exit 0 ignored, 1
  not; status shows nothing for an ignored file) for an untracked one. Pijul: `is_tracked`, then
  whether `pijul diff --short -- <file>` prints anything, or whether `pijul diff --short
  --untracked .` in the file's directory lists it, as `U <path>` at the repository root and
  `A <path>` below it (pijul 1.0.0-beta.18; beta.11 has the flag); an ignored file is left out.
  Without `--untracked` an untracked file at the root is not listed, and `pijul add` cannot tell
  either, as it reports success for an ignored file. A missing file is not classified reliably (a
  deleted tracked file is `Modified` for Git, a missing untracked one `Ignored` for Pijul), so
  callers skip the check for one.
- **The check on open (RTM-57 decision)**, before the first pull, whenever the frontend would commit
  or sync (the opt-out turns off commits too, so it skips the check):

  | State | What the frontend does |
  |---|---|
  | clean | nothing extra |
  | modified | asks to commit the outside changes (rtmcli: exits 1 unless `--commit-changes`); yes: commit with a fixed message, then pull; declined: not opened (the user commits or reverts by hand) |
  | untracked | offers to add it (rtmcli: a hint unless `--commit-changes`); declined: used without sync, asked again next time |
  | ignored | not versioned: used, never synced, never asked |
  | other files dirty | nothing extra (`git pull --autostash` keeps their content, not their staging) |
  | conflicted | not opened until the user resolves it (RTM-16 rule 8); an untracked file is used without sync instead |
  | missing | unchanged (the frontend's missing-file behaviour) |

  rtmcli implements it (RTM-63), the desktop app with a dialog (RTM-70). Since RTM-20 the app writes
  and commits each change at once whenever no job runs; a failed commit or a crash between the two
  can still leave its own change uncommitted, which the check then reports as an outside change.
- Network calls (pull, push) run with `GIT_TERMINAL_PROMPT=0`: credentials come from a helper or
  ssh-agent, never a terminal prompt (decided for the CLI too). They are killed after 60 s
  (`Timeout`). Their stdout is discarded and their stderr goes to a temporary file, not a pipe: ssh
  can outlive git and hold a pipe open. The file is created exclusively (never an existing file or
  symlink at its predictable name) and, on Unix, readable only by the user.
- **`index.lock`:** a local call (`add`, `diff`, `commit`, the status checks) whose stderr mentions
  `index.lock` (the app and the CLI at once) is retried, 100 ms doubling, for about 3 s; the lock is
  never removed. The file name in git's message is not translated, so this stays locale-free;
  checking for the file after the failure would miss a lock held for milliseconds. Pull and push are
  not retried: a pull that meets the lock stops before rebasing or stashing (checked) and leaves
  nothing, so it is a "Not synced" warning that the next sync clears.
- Paths are resolved by one helper for `detect`, `commit` and `sync`; on Windows it strips the
  `\\?\` prefix of canonical paths, which git does not accept as a working directory.
- **Pijul signs every change.** From 1.0.0-beta.15 on only through the SSH agent (`signing_key` in
  its config picks a key when the agent holds several); its agent client is a stub on Windows, so
  current pijul cannot record there and `commit` reports its error (RTM-58).

---

## Important Notes

- Detection and process-runner tests are inline, commit and sync tests in `tests/`; all use scratch
  directories under the system temp directory. Run with `cargo test -p rtmsync`.
- `tests/git.rs` gives each scratch repository, the bare remote of the sync tests included, a local
  identity, `commit.gpgsign=false`, an empty `core.hooksPath` and `rerere.enabled=false`, so the
  user's global config does not interfere. The sync tests use a bare remote and two clones on local
  paths: no network.
- `tests/pijul.rs` holds exactly one test, because it sets process-wide environment variables
  (`unsafe` in Rust 2024): `HOME`, `XDG_CONFIG_HOME` and `PIJUL_CONFIG_DIR` (on Windows pijul's
  default comes from the system, not `APPDATA`) point at scratch directories. On Unix it starts its
  own `ssh-agent` with a generated key and gives every ssh tool that socket explicitly, so it never
  reaches the user's agent; the socket path is relative, as the absolute one can exceed the Unix
  limit. On Windows it writes a throwaway on-disk identity for pijul 1.0.0-beta.11 (which cannot
  create one without a terminal on a fresh config). It skips when `pijul` is not in `PATH`, unless
  `RTM_REQUIRE_PIJUL` is set. Its sync part clones the scratch repository twice; each clone keeps
  its todo.txt at the same depth (`a/sub`, `b/sub`), so the relative socket path still resolves.
- CI (`rust.yml`) builds pijul 1.0.0-beta.24 on Linux and macOS, cached per OS and version, and
  sets `RTM_REQUIRE_PIJUL=1`. Windows downloads the community build of 1.0.0-beta.11 linked from
  pijul.org (boringcactus/pijul-windows-builds), checked against its SHA-256: pijul cannot be built
  from source there (libsodium-sys's build script panics under MSVC, and pijul's own Windows code
  did not compile at beta.12), and newer releases could not record anyway (RTM-58).
- `rtmcli` (RTM-21) and `rtmapp` (RTM-64) depend on the crate, so the Release workflow's `paths`
  filter includes `rtmsync/**`.
