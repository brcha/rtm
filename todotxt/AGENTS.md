# AGENTS.md — todotxt (core library)

## What This Component Is

The core Rust library for parsing, representing, and serializing Todo.txt files. No binary, no UI.
All three frontends depend on this crate via `path` dependency.

---

## Architecture / Structure

```
todotxt/src/
├── lib.rs               ← Public API re-exports
├── todo_item.rs         ← TodoItem struct + FromStr + Display
├── todo_library.rs      ← TodoLibrary (load/save/CRUD)
├── todo_context.rs      ← @context tag
├── todo_project.rs      ← +project tag
├── todo_priority.rs     ← (A)-(Z) priority
├── todo_recurrence.rs   ← rec: tag (daily/weekly/monthly/yearly/business-day)
├── write_lock.rs        ← WriteLock: the lock every rtm process holds while changing a file
└── tests/               ← Unit tests, one file per source file and topic
todotxt/tests/
├── library_file_tests.rs    ← Load, edit and save todo.txt files through TodoLibrary
├── spec_format_tests.rs     ← Specification: the format rules and their negative cases
├── spec_extension_tests.rs  ← Specification: due:, t:, rec:, pri:, uuid:, sub:
├── spec_roundtrip_tests.rs  ← Specification: lines and files come back byte for byte
└── spec_operation_tests.rs  ← Specification: complete, uncomplete, edit, recurrence
```

---

## Conventions

- **Edition:** Rust 2024
- **External dependencies:** `chrono` (dates), `uuid` (item identity), `thiserror` (parse
  error types), `libc` on Unix (the user ID naming the lock directory).
- Parse errors are `thiserror` enums that preserve their source via `#[from]`; they derive
  `Clone, Debug, Eq, PartialEq` alongside `thiserror::Error`.
- `TodoItem` implements `FromStr` (parse from a todo.txt line) and `Display` (serialize back).
- `TodoLibrary` owns a `Vec<TodoItem>` and a file path string. `load()` reads from disk,
  `save()` writes back. No async. The file's final newline is kept as read (RTM-210): a file
  that ends with one keeps it, one without stays without, and a new or empty file gets one.
  CRLF line endings are saved as LF. `save()` writes through `write_atomic` (below), so a crash
  or a full disk never leaves the file empty or cut short.
- **`write_atomic(path, contents)`** replaces a file so that readers see the old
  content or the new one, never a part. It writes `.<name>.<uuid>.tmp` in the same directory
  (`create_new`), gives it the original's permissions, `sync_all`s it and renames it over the
  original; on any error the temporary file is removed and the original is untouched. rtm
  commits only the files it names, so a temporary file a crash leaves behind is never
  committed. A symbolic link stays a link: the file it leads to is replaced (for a dangling
  link, the path it names is created); this is for rtmapp's `config.toml`, as todo files are
  never links. A read-only file (`Permissions::readonly`) is refused with `PermissionDenied`, as
  an in-place write would be; the error texts carry no path, as callers prefix it. Saving needs
  write permission on the directory, not only on the file. The replacement is owned by the writing user and breaks hard
  links, as with editors that save by rename. On Windows the rename fails while another process
  holds the file open without share-delete; that error reaches the caller like any write error.
  The directory is not synced, so a power loss right after the rename may bring back the
  previous version, never a partial one.
- **Symlinked todo files are refused.** `refuse_symlink(path)` fails with an `InvalidInput`
  `io::Error` carrying a `SymlinkError` (`link`, `target`; `downcast_ref` it from the error) when
  the file itself is a symbolic link, dangling or not: "'<link>' is a symbolic link to
  '<target>'; open '<target>' instead", the target being the file at the end of the link chain
  (`canonicalize`), or for a dangling link the path it names. Files kept next to the todo file
  (the archive) would otherwise land next to the link. Links among the parent directories pass:
  they change the path's spelling, not the directory. A missing path passes too. `load()` calls
  it first; each frontend also calls it before it locks, syncs or creates the file, since those
  happen before the load.
- `add_item(item)` stamps `creation_date = today` when the item has none; an explicit date is
  left alone. `load()` never does this — a file with dateless lines stays dateless. This is
  the single point new items enter the library, which is what guarantees a completed item can
  always carry a creation date without `complete_item` or `Display` having to invent one.
- `complete_item(index)` marks done, sets `completion_date = today` (idempotent — completing
  an already-done item does not overwrite its existing completion date), handles recurrence
  (appends new item), returns `Option<bool>` (None = out of bounds, Some(true) = had
  recurrence, Some(false) = no recurrence). It never fabricates a creation date; see
  `add_item` above. The next occurrence is a copy of the completed item (open, today's creation
  date, the new dates, no `uuid`/`sub`), so it keeps the line's layout. Its threshold date (`t:`) moves with its due date:
  strict recurrence advances it by the interval (in calendar months for `m` and `y`, so it keeps
  its day of the month, and in business days for `b`), normal recurrence keeps its distance to
  the new due date. Business days (`b`) skip Saturdays and Sundays, as in topydo and SimpleTask
  (no holidays). Every recurring item gets a next occurrence (RTM-209): one with only `t:`
  moves its threshold (strict from the old `t:`, normal from today) and gets no `due:`; one with
  neither date is due the interval from today, strict too. The completed item is then taken
  from its own line parsed again, so it equals what `save` writes and `load` reads back.
- `uncomplete_item(index)` clears `done` and `completion_date`, returns `Option<()>` (None =
  out of bounds). Priority and creation date are untouched — priority re-serializes as `(X)`
  automatically because `Display` keys that choice off `done`, and the `pri:` tag goes.
- `replace_item(index, item)` (RTM-115) puts `item` in the old one's place and returns the old
  item, `Option<TodoItem>` (None = out of bounds). The new item is stored as given: no date is
  stamped, and `uuid`/`sub` are the caller's to carry over. Like every mutation it does not save.
- `remove_item(index)` removes the item and, recursively, every item whose `sub` is the uuid
  of a removed item (cyclic links terminate). Returns `Option<Vec<TodoItem>>`: None = out of
  bounds, otherwise the removed items in file order. Like every mutation it does not save.
- **Archive.** `archive_path()` names the archive next to the todo file:
  `done.txt` for a file named exactly `todo.txt` (the todo.sh, SimpleTask and topydo
  convention), otherwise `<stem>.done.<ext>` (`work.txt` -> `work.done.txt`, `notes` ->
  `notes.done`), so two todo files in one directory never share an archive. The path is used as
  given; symlinked todo files are refused (above).
  `archive(completed_before)` moves the done items there and returns them in file order; with a
  date, only items completed before it (exclusive), and a done item without a completion date
  counts as old. Every done item moves, whatever its `sub:` links; until RTM-227 makes completing
  a parent complete its sub-items, a done parent can leave an open sub-item behind. The archive is only appended to (`OpenOptions` append and create,
  never read except its last byte, so a missing final newline is added first) with the lines
  `save` would write; nothing to archive creates no file. The append comes first and the items
  leave the library only after it succeeds, so an interruption before the caller's `save`
  leaves them in both files, never in neither; on an error the library is unchanged. It does
  not save the todo file: the caller saves it, and in a repository commits both files.
- **Projects and contexts** (RTM-119), shared by every frontend so they behave the same. Tag
  names are compared case-insensitively (`+Work` is `+work`).
  - `TodoLibrary::projects()` / `contexts()` return `Vec<(TodoProject, usize)>` /
    `Vec<(TodoContext, usize)>`: every tag in the file, spelled as first seen in file order,
    sorted by the lowercased name, with the number of open items carrying it. A tag found only
    on done items is listed with 0; an item carrying a tag twice counts once.
  - `TodoItem::matches(projects, contexts)` is true when the item carries **every** given
    project and context; empty lists match everything. `TodoLibrary::filter_items(projects,
    contexts)` returns the file indices of the matching items, in file order, done items
    included: status and due filters stay with each frontend.
- `TodoItem` fields: `done`, `priority`, `completion_date`, `creation_date`, `description`,
  `projects`, `contexts`, `due`, `recurrence`, `threshold`, `uuid`, `sub`, and `layout`.
- **`Display` writes back the line as read** (RTM-204). `from_str` keeps the line in `layout`
  (an opaque `Layout`: the text, where each word stands and what it set, and the fields as
  read); an item built in code has `layout: None` and is written in a fixed order (head,
  description, `pri:`, projects, contexts, `due:`, `rec:`, `t:`, `uuid:`, `sub:`). An item whose
  fields equal those read is written byte for byte; otherwise only what changed is patched:
  - a changed `done`, priority, completion or creation date writes a new head (`x` and the
    dates, or `(A)` and the creation date); a changed `done` or priority also removes the `pri:`
    tag that set the priority and appends `pri:X` for a done item with one; a head written on
    a line that starts with whitespace replaces that whitespace;
  - a changed `due:`, `t:`, `rec:`, `uuid:` or `sub:` replaces its tag in place (the last one,
    which set the field), `None` removes every such tag, and a field that had none appends one;
  - projects and contexts still listed keep their place (a repeated one as often as it is still
    listed), removed ones go, new ones are appended;
  - a changed description replaces only the words between the part it shares with the old one
    at its start and at its end, so a typo fix or an added word keeps every tag in place;
  - a removed word takes the whitespace before it; new tags go after the last word, before
    trailing whitespace;
  - a recurrence rtm writes (new or changed) always carries its count (`rec:1w`, `rec:+1m`,
    RTM-206), which SimpleTask and topydo both read; an unchanged one keeps its spelling.

  Equality (`PartialEq`, written by hand) compares the fields and leaves out `layout`, so two
  lines that differ only in spacing are equal items; a new field must be added there too. A
  done line with its priority between the `x` and the dates (`x (A) 2011-03-02 …`) is written
  in the specification's form even unchanged (`x 2011-03-02 … pri:A`).
- **Priority has two serialization forms, chosen by `done`:** an open item writes `(X)`
  before the description; a completed item writes `pri:X` (appended at the end of the line
  when rtm completes it). `(X)` parses on any line; `pri:X` sets the priority only on a done
  line (RTM-211), as the specification names it only to keep a complete task's priority. On
  an open line `pri:A` is an ordinary tag kept as written; `complete_item` reads the completed
  item back from its line, so the tag becomes the priority at once and stays where it stands,
  while a recurring item's next occurrence keeps it as a tag. An unchanged line keeps the form
  it was read in. If a done line carries both and the letters agree (`x (C) … pri:C`), the
  duplicate is accepted; if the letters disagree (`x (C) … pri:A`), `(C)` stays the priority and `pri:A`
  stays in the description as text. A malformed `pri:` value (`pri:xyz`) is text like any
  other unreadable tag (below).
- **`TodoItem::from_str` accepts every line.** A known tag (`due:`, `t:`, `rec:`, `uuid:`,
  `sub:`, `pri:`) whose value rtm cannot read and a leading `(X)` that is not A-Z (`(b)`, `(1)`)
  stay in the description as written, so `TodoLibrary::load`, which skips lines that fail to
  parse, never drops one and the next save keeps it. A date (creation, completion, `due:`,
  `t:`) is exactly `YYYY-MM-DD` (RTM-205, `strict_date`): `2011-3-2` or `11-03-02` is text.
  `strict_date` is public, so rtmapp's date fields accept the same form.
  The head (`x`, `(A)`, the dates) is read only at the start of the line, and `x` and `(A)`
  only when a space follows (RTM-212): a line that starts with whitespace, or `x` or `(A)`
  followed by a tab, is an open task with that text in its description.
- **`check_dates()`** refuses an item whose threshold date (`t:`) is after its due date
  (`TodoItemDateError::ThresholdAfterDue`, "threshold date X is after due date Y"); the same day,
  one of the two dates or neither passes. Every frontend calls it on what the user enters (added
  and edited items) and refuses with that message. Nothing in the library calls it: `load`,
  `add_item`, `replace_item`, `complete_item` and `Display` accept such an item, so a line already
  in a file loads and saves unchanged. The next occurrence of a recurring item never breaks the
  rule: both dates move by the same interval, or keep their distance.
- **`is_future(today)`** says whether an item is not actionable yet: it has a threshold (`t:`) or
  due date, and every one it has is after `today`. A passed threshold or due date makes it current;
  an item without either is never future. rtmapp's "Show future items" uses it.
- **`DateFilter`** is the one definition of the date filters of rtmcli (`list`, `complete`) and
  the MCP `list_items` `due` argument: `today` selects an item whose due or threshold date is
  today, `week` one whose due or threshold date is within the next 7 days, `overdue` one due
  before today, `future` what `is_future` reports, `all` every item.
- **`Display` never emits a creation date without a completion date on a done line.** A
  completed item's creation date is written only immediately after its completion date; if
  there is no completion date, no date is written at all, even if a creation date is present.
  This is deliberate: a lone date on a done line is indistinguishable from a completion date
  on the next parse, so writing one would silently turn a creation date into a different
  fact. It is also why fabricating dates only ever happens at `add_item`/`complete_item`, not
  in `Display`.
- **`WriteLock::acquire(path)` (RTM-115)** waits for an exclusive lock on a todo.txt and holds it
  until dropped. Every rtm process holds it whenever it writes the file, a pull included: around
  the pull, load, change, save and commit of one change (and the push, where it follows at once),
  and around every pull on its own. So two processes (the app, the CLI, MCP servers of different
  clients) never lose each other's change, nor a change a pull brought. Writers lock the file's
  repository directory when it is in one (`rtmsync::lock_target`, RTM-142: a pull rewrites every
  file there), else the file, so the path may also be a directory. `WriteLock::try_acquire(path)`
  (RTM-142) takes it only if no one holds it (`Ok(None)` otherwise), for rtmapp's UI thread, which
  must not wait out another process's pull. It is `std::fs::File::lock` (Rust 1.89) on
  `<hash>.lock` in `write_lock::lock_dir()`: never next to the todo.txt, where its repository would
  see it, and never depending on the environment (RTM-116), as MCP clients start servers without
  `XDG_RUNTIME_DIR` or `TMPDIR` and a server locking elsewhere would not exclude the app. On Unix
  it is `/tmp/rtm-<uid>` (`libc::getuid`), created with mode 0700 and refused unless it is a real
  directory owned by the user with no group or other permissions (`/tmp` is shared); on Windows
  the temporary directory's `rtm`, which is per user. Each acquire refreshes the lock file's
  modification time, as `/tmp` is cleaned of old files (systemd-tmpfiles, macOS's daily cleanup)
  and a lock file deleted while held would let the next process lock a new file of the same name;
  only the MCP server's claim, held for the server's lifetime, can age that long. Another local
  user who creates `/tmp/rtm-<uid>` first makes every rtm write fail (refused, never shared): a
  denial of service on a shared machine, accepted for the environment-independent place. The hash is 64-bit FNV-1a, written out
  because `DefaultHasher` may change between Rust versions, of the canonical path (for a file not
  created yet, its directory's canonical path and its name), so every spelling and every rtm
  binary agree. `write_lock::lock_dir()` is public, for the other rtm locks kept there (the MCP
  server's one-server-per-client check).

---

## Important Notes

- `uuid` and `sub` fields exist in the data model for future subtask support but are not yet
  surfaced in any UI.
- Unit tests are in `src/tests/`, one file per source file and topic (see Tests in the root
  `AGENTS.md`); `tests/library_file_tests.rs` loads, edits and saves todo.txt files through
  `TodoLibrary`. Run with `cargo test -p todotxt`.
- The specification suite, `tests/spec_*_tests.rs`, checks the library against the todo.txt
  specification (https://github.com/todotxt/todo.txt) and the extensions rtm supports; every
  test names the rule or tool convention it checks. A test that fails because the library
  deviates is marked `#[ignore = "RTM-NN: …"]` with the key of the issue that fixes it, and that
  fix removes the `ignore`; `cargo test -p todotxt -- --ignored` lists the open deviations.
