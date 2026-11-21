# Changelog

Generated with git-cliff from the squash-merged pull requests of this release line.

## [26.1.1] - 2026-11-21

This release makes rtm follow the todo.txt format more closely and keep each line as it was
written: lines it cannot interpret, the line's layout and spacing, and the file's final newline
are kept; only YYYY-MM-DD words are read as dates, `pri:` is the priority only on completed
lines, and a line that starts with a space is never read as complete or prioritized. Recurrence
now uses calendar months and years, skips weekends for business days, works without a due date,
keeps a count of 1 (`rec:1w`) and moves the threshold date (`t:`) with the item; items whose
threshold date has passed are shown, date filters read the threshold date, and an item whose
threshold date is after its due date is refused. On Windows, the app started from the
installer's last screen syncs, and file paths show without the `\\?\` prefix.

### Features

- todotxt: refuse an item whose threshold date is after its due date -- RTM-214 (#183)

### Bug fixes

- start no Release run when a release is published -- RTM-102 (#77)
- todotxt: calendar months and years in recurrence -- RTM-207 (#136)
- todotxt: move the threshold date with a recurring item -- RTM-177 (#140)
- todotxt: keep lines rtm cannot interpret -- RTM-203 (#143)
- todotxt: keep the line's layout when writing it back -- RTM-204 (#146)
- todotxt: read only YYYY-MM-DD words as dates -- RTM-205 (#149)
- todotxt: write a recurrence with its count -- RTM-206 (#152)
- todotxt: skip weekends in business-day recurrence -- RTM-208 (#155)
- rtmapp: show and save Windows paths without the \\?\ prefix -- RTM-217 (#162)
- rtmapp: start the app through Explorer after installing -- RTM-216 (#160)
- todotxt: recur items without a due date -- RTM-209 (#169)
- todotxt: keep the final newline when saving -- RTM-210 (#172)
- todotxt: read pri: as the priority only on done lines -- RTM-211 (#175)
- todotxt: read the head only at the start of the line -- RTM-212 (#178)
- rtmapp: decide future items by their threshold date -- RTM-222 (#190)
- todotxt: date filters read the threshold date -- RTM-230 (#198)

### CI

- github: require a scope in commit and PR titles -- RTM-110 (#81)
- github: pin actions to commit SHAs -- RTM-145 (#129)
- github: fall back to a mirror outside ubuntu.com for apt -- RTM-213 (#133)
- github: link each pull request description to its Jira issue -- RTM-219 (#165)

### Tests

- todotxt: specification test suite -- RTM-196 (#127)

## [26.1.0] - 2026-11-01

### Features

- add the rtmgui scaffold -- RTM-34 (#10)
- log file dialog failures to stderr -- RTM-42 (#12)
- list filters, completion and quick-add in rtmgui -- RTM-35 (#13)
- edit dialog in rtmgui -- RTM-36 (#14)
- calendar date picker in rtmgui -- RTM-40 (#16)
- rtmapp app icon -- RTM-46 (#19)
- delete sub-items with their parent -- RTM-14 (#24)
- rtmcli: delete command -- RTM-23 (#25)
- rtmapp: delete button in the edit dialog -- RTM-24 (#26)
- set the Windows AppUserModelID in rtmapp and its shortcuts -- RTM-48 (#22)
- rtmsync: detect whether todo.txt is versioned -- RTM-12 (#28)
- rtmsync: commit a todo.txt to its repository -- RTM-18 (#32)
- rtmsync: sync a todo.txt's repository with its remote -- RTM-19 (#33)
- rtmcli: sync the todo.txt through its repository -- RTM-21 (#34)
- rtmapp: sync the todo.txt through its repository -- RTM-64 (#35)
- rtmcli: check the todo.txt for outside changes before syncing -- RTM-63 (#36)
- rtmapp: sync the todo.txt in the background -- RTM-20 (#37)
- rtmapp: ask about a dirty or untracked todo.txt on open -- RTM-70 (#38)
- rtmapp: create a new todo.txt -- RTM-55 (#40)
- add the project website's landing page -- RTM-88 (#62)
- add the install guide to the website -- RTM-93 (#65)
- enlarge the screenshots in a dialog on click -- RTM-99 (#69)

### Bug fixes

- return an error for malformed recurrence -- RTM-43 (#15)
- rtmapp: open the file dialogs without blocking the UI -- RTM-65 (#41)

### Refactoring

- replace the Tauri app with the egui app -- RTM-39 (#18)
- fix the clippy warnings across the workspace -- RTM-92 (#60)

### Documentation

- update the GUI screenshots -- RTM-51 (#30)
- step-by-step install instructions -- RTM-56 (#31)
- record the release process -- RTM-72 (#42)
- record which changes start which workflow -- RTM-76 (#45)
- move the detailed AGENTS.md sections into doc/ -- RTM-81 (#57)
- add the website release post to the release process -- RTM-91 (#64)
- replace the rtmcli screenshot -- RTM-100 (#68)

### Build

- add set-version.sh and refresh Cargo.lock -- RTM-41 (#11)
- rtmgui Nix package and dev shell fixes -- RTM-37 (#17)
- package rtmapp without Tauri's bundler (#20)
- macOS packages -- RTM-11 (#29)
- fix the Nix flake build -- RTM-71 (#39)
- generate CHANGELOG.md with git-cliff on release branches -- RTM-74 (#47)
- define the Nix packages and dev shell once, with metadata from Cargo.toml -- RTM-75 (#48)
- build Linux packages on AlmaLinux 9 and add RPMs -- RTM-9 (#55)
- use pollster 1 in rtmapp -- RTM-84 (#56)

### CI

- upgrade and smoke-launch tests in the release workflow -- RTM-49 (#23)
- move GitHub Actions to their Node.js 24 majors -- RTM-50 (#27)
- release from a release branch by workflow dispatch -- RTM-73 (#44)
- skip the builds for documentation-only changes -- RTM-76 (#43)
- require one gate check per workflow -- RTM-76 (#46)
- run the tests and Nix builds for pull requests against release branches -- RTM-78 (#49)
- skip the Nix and Rust builds when a pull request changes none of their inputs -- RTM-82 (#51)
- skip the Rust and Nix builds on pushes that change only the website -- RTM-87 (#59)
- build the website and check its links -- RTM-89 (#63)
- publish the website on Vercel -- RTM-90 (#66)

### Tests

- move the tests into src/tests and add integration tests -- RTM-83 (#58)
- add egui_kittest UI tests for rtmapp -- RTM-86 (#61)

### Other

- add egui theme fidelity spike -- RTM-33 (#9)
- remove the egui theme spike -- RTM-47 (#21)

