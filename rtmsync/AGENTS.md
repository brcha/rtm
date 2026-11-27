# AGENTS.md — rtmsync

Commits and syncs a todo.txt through its Git or Pijul repository. Git and Pijul only.

- VCSs are driven through their executables (`git`, `pijul`); never link a Git or Pijul library.
- `detect` looks only at the markers `.git` and `.pijul` and runs nothing; `.git` is checked with `exists`, as it is a file in worktrees and submodules.
- `check` is the one copy of the repository rules; frontends map its result to messages, exit codes or dialogs and never re-implement them.
- `commit_files` commits only the files named and leaves the user's other staged changes staged; never `add -A` or `commit -a`.
- Executables get paths after `--` (git also `--literal-pathspecs`, except for `check-ignore`), a null stdin, and `CREATE_NO_WINDOW` on Windows.
- Parse only locale-independent output (porcelain v2, plain path lists); no `LC_ALL`.
- Pull and push run with `GIT_TERMINAL_PROMPT=0` and a 60 s timeout, their stderr in a private temp file, not a pipe (ssh can outlive git and hold it open).
- A local call failing on `index.lock` is retried for about 3 s; rtm never removes the lock and never aborts a stopped rebase.
- A missing file's state is unreliable: callers skip the check, and rtm never commits a deletion.
- Canonical Windows paths lose their `\\?\` prefix, which git refuses as a working directory.
- `tests/sync_pijul_tests.rs` holds exactly one test, as it sets process-wide environment variables; it skips without `pijul` unless `RTM_REQUIRE_PIJUL` is set.
- Pijul from 1.0.0-beta.15 signs only through an SSH agent, a stub on Windows, so it cannot record there; Windows CI uses the prebuilt beta.11 (pijul does not build under MSVC).
- The Pijul test on CI's version, locally: `nix develop -c nix shell github:NixOS/nixpkgs/nixos-unstable#pijul -c cargo test -p rtmsync --test sync_pijul_tests`.
