# AGENTS.md — rtmsync (sync library)

## What This Component Is

The library that synchronises a todo.txt file through the version control system it lives in
(epic RTM-1). Git and Pijul are in scope; no others. No UI, no binary; `thiserror` is the only
dependency.

Done: detecting the VCS (RTM-12), committing the file (RTM-18). Still to come: the Pijul backend
(RTM-53), the sync task (RTM-19), pull before change (RTM-21), periodic GUI sync (RTM-20). Open
design question: a dirty repository or todo.txt when opening it (RTM-57).

---

## Architecture / Structure

```
rtmsync/
├── src/lib.rs        ← Vcs enum, detect(), Vcs::commit(), Error, detection tests
└── tests/
    ├── git.rs        ← Vcs::commit with Git
    └── pijul.rs      ← Vcs::commit with Pijul (a single test, see below)
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
  nothing. The caller chooses the message; no frontend calls it yet (planned for RTM-21).
- Executables run in the file's directory with the file name after `--` (`--literal-pathspecs` for
  git), stdin null (prompts on stdin fail; terminal prompts such as pinentry still can, timeouts
  are RTM-19's), and `CREATE_NO_WINDOW` on Windows. Errors: `Spawn` (could not start; `NotFound`
  usually means not in `PATH`, but also a missing directory) and `Failed` (non-zero exit, with
  stderr). Output is never parsed, so no `LC_ALL=C`.
- Paths are resolved by one helper for `detect` and `commit`; on Windows it strips the `\\?\`
  prefix of canonical paths, which git does not accept as a working directory.
- **Pijul signs every change.** From 1.0.0-beta.15 on only through the SSH agent (`signing_key` in
  its config picks a key when the agent holds several); its agent client is a stub on Windows, so
  current pijul cannot record there and `commit` reports its error (RTM-58).

---

## Important Notes

- Detection tests are inline, commit tests in `tests/`; all use scratch directories under the
  system temp directory. Run with `cargo test -p rtmsync`.
- `tests/git.rs` gives each scratch repository a local identity, `commit.gpgsign=false` and an empty
  `core.hooksPath`, so the user's global config does not interfere.
- `tests/pijul.rs` holds exactly one test, because it sets process-wide environment variables
  (`unsafe` in Rust 2024): `HOME`, `XDG_CONFIG_HOME` and `PIJUL_CONFIG_DIR` (on Windows pijul's
  default comes from the system, not `APPDATA`) point at scratch directories. On Unix it starts its
  own `ssh-agent` with a generated key and gives every ssh tool that socket explicitly, so it never
  reaches the user's agent; the socket path is relative, as the absolute one can exceed the Unix
  limit. On Windows it creates an on-disk identity for pijul 1.0.0-beta.12. It skips when `pijul`
  is not in `PATH`, unless `RTM_REQUIRE_PIJUL` is set.
- CI (`rust.yml`) builds pijul on all three OSs (beta.24 on Linux and macOS, beta.12 on Windows),
  cached per OS and version, and sets `RTM_REQUIRE_PIJUL=1`. On Windows it is built for the GNU
  target with MSYS2's MinGW libraries, with their DLLs copied next to it: `thrussh-libsodium` forces
  libsodium-sys's pkg-config mode, which panics on MSVC.
- No frontend depends on the crate yet. When one does, the release workflow's `paths` filter needs
  `rtmsync/**`.
