# Changelog

Each release's section opens with a summary of the release, followed by the changes generated with
git-cliff from the squash-merged pull requests of this release line.

## [26.2.1] - 2026-11-21

rtm 26.2.1 follows the todo.txt format more closely and fixes several problems in the desktop app.

- **Lines kept as written.** Lines rtm cannot interpret, the line's layout and spacing, and the
  file's final newline are kept when the file is saved. Only `YYYY-MM-DD` words are read as dates,
  `pri:` is the priority only on completed lines, and a line that starts with a space is never
  read as complete or prioritized.
- **Recurrence.** Monthly and yearly recurrence uses calendar months and years, business-day
  recurrence skips weekends, a recurring item without a due date recurs, `rec:1w` keeps its count,
  and the threshold date (`t:`) moves with the item.
- **Threshold dates.** Items whose threshold date has passed are shown even when their due date is
  in the future, the date filters of `rtmcli` and the MCP tools read the threshold date, and an
  item whose threshold date is after its due date is refused.
- **Desktop app.** On exit, a change whose commit is queued together with the last sync is now
  committed. On Windows, the app started from the installer's last screen syncs, and file paths
  show without the `\\?\` prefix.

### Changes

#### Features

- todotxt: refuse an item whose threshold date is after its due date -- RTM-214 (#182)

#### Bug fixes

- todotxt: calendar months and years in recurrence -- RTM-207 (#135)
- todotxt: move the threshold date with a recurring item -- RTM-177 (#139)
- todotxt: keep lines rtm cannot interpret -- RTM-203 (#142)
- todotxt: keep the line's layout when writing it back -- RTM-204 (#145)
- todotxt: read only YYYY-MM-DD words as dates -- RTM-205 (#148)
- todotxt: write a recurrence with its count -- RTM-206 (#151)
- todotxt: skip weekends in business-day recurrence -- RTM-208 (#154)
- rtmapp: show and save Windows paths without the \\?\ prefix -- RTM-217 (#161)
- rtmapp: start the app through Explorer after installing -- RTM-216 (#159)
- todotxt: recur items without a due date -- RTM-209 (#168)
- todotxt: keep the final newline when saving -- RTM-210 (#171)
- todotxt: read pri: as the priority only on done lines -- RTM-211 (#174)
- todotxt: read the head only at the start of the line -- RTM-212 (#177)
- rtmapp: wait for a commit queued with the last sync on exit -- RTM-220 (#180)
- rtmapp: decide future items by their threshold date -- RTM-222 (#188)
- todotxt: date filters read the threshold date -- RTM-230 (#197)

#### CI

- github: fall back to a mirror outside ubuntu.com for apt -- RTM-213 (#134)
- github: link each pull request description to its Jira issue -- RTM-219 (#164)

#### Tests

- todotxt: specification test suite -- RTM-196 (#128)

## [26.2.0] - 2026-11-08

rtm 26.2.0 opens several todo.txt files at once in the desktop app, lets AI assistants work with
your tasks, lists and filters projects and contexts, and tells you when a newer release is out.

- **Several todo.txt files in the desktop app.** Keep personal, work and project lists open side
  by side, one tab each. The app reopens them at the next start; a tab can be renamed from its
  context menu, and an item can be moved to another open file. Each repository syncs on its own,
  whether the files share one or not.
- **AI assistants.** `rtmcli mcp` and `rtmapp mcp` run an MCP server, so an assistant such as
  Claude Desktop or Claude Code can list, add, edit and complete your items. The app, the
  command-line tool and the server now take turns writing a todo.txt and its repository, so none
  of them overwrites another's change, and the app reloads a file that another program changed.
- **Projects and contexts.** The desktop app lists every `+project` and `@context` in a sidebar,
  with the number of open items; click one there, or in an item, to filter the list.
  `rtmcli projects` and `rtmcli contexts` list them, and `rtmcli list` filters by them.
- **Update check.** The desktop app tells you when a newer release is out, and a new Settings
  dialog turns the check off or follows the beta channel. `rtmcli check-update` checks on demand.
  The check is a single request to the website and sends no identifier.
- **Development.** Every pull request now gets an automatic code review.

### Changes

#### Features

- link the latest release news from the top of the homepage -- RTM-103 (#74)
- download links at the top of the release news page -- RTM-104 (#76)
- latest downloads at the top of the install guide -- RTM-105 (#79)
- site: Nix install of a release tag or branch -- RTM-111 (#82)
- rtmapp: keep a list of todo.txt files in the config -- RTM-124 (#85)
- rtmapp: burger menu and settings dialog -- RTM-134 (#89)
- site: releases.json and beta posts -- RTM-135 (#91)
- rtmapp: one sync worker per repository -- RTM-126 (#90)
- rtmapp: open several todo.txt files as tabs -- RTM-127 (#92)
- rtmupdate: fetch and compare releases -- RTM-136 (#93)
- rtmapp: move an item to another open file -- RTM-128 (#94)
- rtmcli: check-update command -- RTM-137 (#95)
- todotxt: list projects and contexts and filter by them -- RTM-119 (#96)
- rtmcli: list projects and contexts -- RTM-122 (#97)
- rtmapp: update notice and update settings -- RTM-138 (#98)
- rtmcli: filter the list by projects and contexts -- RTM-123 (#101)
- release: beta releases -- RTM-139 (#103)
- rtmapp: list projects and contexts in the sidebar -- RTM-120 (#105)
- rtmapp: filter the list from the sidebar -- RTM-121 (#108)
- rtmapp: filter by a tag clicked in an item row -- RTM-150 (#111)
- rtmapp: rename a file's tab from its context menu -- RTM-151 (#112)
- rtmmcp: MCP server crate with todo.txt tools -- RTM-115 (#113)
- rtmapp: reload a todo.txt changed on disk, and hold the write lock -- RTM-142 (#114)
- rtmcli: lock the repository around changes and pulls -- RTM-158 (#116)
- workspace: ship the MCP server in rtmcli and rtmapp -- RTM-116 (#117)
- release: announce stable releases on X and Facebook through Buffer -- RTM-171 (#120)
- release: announce releases on X only -- RTM-170 (#121)

#### Bug fixes

- start no Release run when a release is published -- RTM-102 (#75)
- site: center a lone download box -- RTM-117 (#83)
- github: rename the pinned plugin marketplace -- RTM-147 (#106)
- github: let the Claude review start its review agents -- RTM-147 (#107)

#### Refactoring

- rtmapp: move per-file state into its own struct -- RTM-125 (#86)

#### Documentation

- record the release branch push -- RTM-98 (#71)
- website post for 26.1.0 -- RTM-98 (#73)
- name cherry-pick branches cherry/RTM-NN/<slug> -- RTM-102 (#78)
- workspace: ignore Vercel bot comments outside site changes -- RTM-141 (#87)
- github: note that a skipped Claude review still passes -- RTM-147 (#109)
- workspace: README status, summary of what works and badges -- RTM-163 (#115)
- release: changelog summary, release epics and announcements -- RTM-154 (#119)

#### Build

- nix: add pijul to the dev shell -- RTM-146 (#100)

#### CI

- github: require a scope in commit and PR titles -- RTM-110 (#80)
- github: pin actions to commit SHAs -- RTM-145 (#99)
- github: Claude review of pull requests -- RTM-147 (#102)
- github: review pull requests on every push -- RTM-147 (#104)

#### Tests

- release: check that quitting on macOS runs the exit sync -- RTM-140 (#88)
- rtmmcp: LLM eval of the MCP tools -- RTM-159 (#118)

