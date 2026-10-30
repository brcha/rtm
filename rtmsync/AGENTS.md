# AGENTS.md — rtmsync (sync library)

## What This Component Is

The library that synchronises a todo.txt file through the version control system it lives in
(epic RTM-1). Git and Pijul are in scope; no others. No UI, no binary, no dependencies so far.

Done: detecting the VCS (RTM-12). Still to come: commit on change (RTM-18), the Pijul backend
(RTM-53), the sync task (RTM-19), pull before change (RTM-21), periodic GUI sync (RTM-20).

---

## Architecture / Structure

```
rtmsync/src/
└── lib.rs     ← Vcs enum, detect(), tests
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

---

## Important Notes

- Tests live inline in `#[cfg(test)]` and use scratch directories under the system temp directory.
  Run with `cargo test -p rtmsync`.
- No frontend depends on the crate yet. When one does, the release workflow's `paths` filter needs
  `rtmsync/**`.
