# AGENTS.md — rtmmcp (MCP server library)

## What This Component Is

The MCP (Model Context Protocol) server of rtm, part of epic RTM-113: lets MCP clients (Claude
Desktop, Claude Code and others) read and change todo.txt files over stdio. A library, no binary:
`rtmcli mcp` and `rtmapp mcp` run it (RTM-116), so it is there whichever program a user installs.
Decided in RTM-114; the decisions are recorded on RTM-113.

---

## Architecture / Structure

```
rtmmcp/
├── src/
│   ├── lib.rs      ← Source, Sources, Error, serve(), serve_on()
│   ├── server.rs   ← The rmcp server: tool parameters, annotations, instructions, get_info
│   ├── tools.rs    ← What each tool does, synchronously, on one file; the write sequence
│   ├── sync.rs     ← RepoWrite: rtmcli's sync rules for one change, returning messages
│   ├── claim.rs    ← One rtm server per client
│   └── tests/      ← Unit tests: claim, the server's duplicate check, sync (git), tools
└── tests/
    └── mcp_protocol_tests.rs  ← The server through rmcp's client on an in-memory pipe
```

---

## Conventions

- **SDK:** the official `rmcp`, pinned to a minor version (`3.9`): it releases about weekly and
  changes its API between majors. rmcp types stay inside the crate; `schemars` is the version rmcp
  uses (a second copy would not satisfy its bounds).
- **Async only here, behind a synchronous entry point** (root `AGENTS.md`): `serve(sources)` builds
  a current-thread tokio runtime and serves stdin and stdout until the client closes stdin.
  `serve_on(sources, read, write)` serves any reader and writer, for tests, without the
  one-server-per-client check. Every file and git call runs in `spawn_blocking`, so the protocol
  is still answered while git waits on the network.
- **stdout is the protocol.** Nothing else may print to it; `rtmsync` captures or discards the
  output of git and pijul.
- **Files:** a `Source` is a path, a display name, whether it syncs and the VCS choice for a
  directory with both. `Sources` is a closure the server calls at the start of every tool call (the
  frontend may re-read its config); the first source is the default. A tool's `file` argument picks
  a source by name or path; anything else is refused. The client never names a path of its own.
- **Server identity:** `rtm` for both frontends, with the same tools, so a client sees one server.
- **One rtm server per client (`claim.rs`):** the first server takes `try_lock` on
  `mcp-client-<parent pid>.lock` in `todotxt::write_lock::lock_dir()`; a held lock is tried again
  for 2 s (a restarted server can start before the old one exits), then the server offers no tools
  and its instructions say another rtm server runs for this client. No parent ID or lock
  directory, or a wrapper process between client and server (a different parent), turns the check
  off: the tools stay. The parent ID is `std::os::unix::process::parent_id` on Unix and a
  Toolhelp32 snapshot (`windows-sys` 0.52, rtmapp's version) on Windows. The claim is released
  before the runtime is dropped, which waits for blocking work such as a pull.
- **Tools:** `list_files`, `list_items` (status open/done/all, due all/today/week/overdue/future as
  rtmcli's filters, projects and contexts with or without their sigil, text ignoring case; an
  explicit `null` counts as left out), `list_projects_and_contexts` (`TodoLibrary::projects` and
  `contexts`, RTM-119), `add_item`, `complete_item`, `uncomplete_item`, `edit_item`, `delete_item`.
  Results are text: one `<number> <line>` per item. A failure is a result with `isError` (the
  assistant reads it), not a protocol error.
- **Annotations:** the reads carry `read_only_hint`. MCP counts a tool without
  `destructiveHint` as destructive, so `add_item`, `complete_item`, `uncomplete_item` and
  `edit_item` say `destructive_hint = false`; only `delete_item` is destructive, so clients confirm
  deletes and nothing else.
- **Items:** every call loads the file from disk; nothing is kept between calls. An item's number
  is its place in the file, from 1. A change takes the number and the line as `list_items` printed
  it (`TodoItem`'s `Display`, not the raw file line): the item at the number if its line matches,
  else the one item with that line (a pull or another program may have moved it), else a refusal,
  showing the item's new line when its `uuid` still exists.
- **A write (`tools::write`):** `todotxt::WriteLock` from before the repository check (another rtm
  process between its save and its commit would look like an outside change) until after the push;
  `RepoWrite::find`, pull, load, change, save, commit, pull and push. Every refusal in that sequence
  before the save ends with "Nothing was changed." (an invalid line or an unknown `file`, refused
  before it starts, also changes nothing); after the save nothing fails the tool (a client might repeat a
  change that landed): a failed commit or push, and a conflict, are notes in the result.
- **Sync (`sync.rs`)** follows rtmcli (`rtmcli/AGENTS.md`), returning messages instead of
  exiting: sync off, no repository or an ignored file → not synced; an untracked file → changed
  without sync, with a note (no `--commit-changes` equivalent); both Git and Pijul without a
  choice, or a choice the directory lacks → refused; a conflicted repository → refused, checked
  before the outside changes, since a file in a stopped rebase is also modified; outside changes →
  refused, the user commits or reverts them. No remote → commit only.
- **Edits keep facts the new line leaves out:** the creation date, `uuid` and `sub`.
- **Commit messages** as rtmcli: `rtm: add <description>`, `rtm: complete <line>`,
  `rtm: uncomplete <line>`, `rtm: edit <old line>`, `rtm: delete <line>`.

---

## Important Notes

- Tests work in scratch directories under the system temp directory. The git tests (sync, and two
  tool tests) give each repository a local identity, `commit.gpgsign=false`, an empty
  `core.hooksPath` and `rerere.enabled=false`; the sync tests use a bare remote and two clones on
  local paths, no network. Pijul is covered by `rtmsync`; the both-VCS case needs only an empty
  `.pijul` directory, as detection looks at markers.
- The tool tests take the real `todotxt::WriteLock`, so their lock files land in the lock directory
  (`$XDG_RUNTIME_DIR/rtm` on a Linux desktop), not in the scratch directory; they are empty files
  named after a hash of the scratch path.
- The claim test sleeps through the 2 s retry, so `cargo test -p rtmmcp` takes about 2 s.
- The Nix `rtmcli` package runs this crate's tests (`cargoTestFlags` in `default.nix`).
- Run with `cargo test -p rtmmcp`.
