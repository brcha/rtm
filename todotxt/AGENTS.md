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
  `save()` writes back. No async.
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
  its day of the month), normal recurrence keeps its distance to the new due date. A recurring
  item without `due:` gets no next occurrence yet.
- `uncomplete_item(index)` clears `done` and `completion_date`, returns `Option<()>` (None =
  out of bounds). Priority and creation date are untouched — priority re-serializes as `(X)`
  automatically because `Display` keys that choice off `done`, and the `pri:` tag goes.
- `replace_item(index, item)` (RTM-115) puts `item` in the old one's place and returns the old
  item, `Option<TodoItem>` (None = out of bounds). The new item is stored as given: no date is
  stamped, and `uuid`/`sub` are the caller's to carry over. Like every mutation it does not save.
- `remove_item(index)` removes the item and, recursively, every item whose `sub` is the uuid
  of a removed item (cyclic links terminate). Returns `Option<Vec<TodoItem>>`: None = out of
  bounds, otherwise the removed items in file order. Like every mutation it does not save.
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
    tag that set the priority and appends `pri:X` for a done item with one;
  - a changed `due:`, `t:`, `rec:`, `uuid:` or `sub:` replaces its tag in place (the last one,
    which set the field), `None` removes every such tag, and a field that had none appends one;
  - projects and contexts still listed keep their place (a repeated one as often as it is still
    listed), removed ones go, new ones are appended;
  - a changed description replaces only the words between the part it shares with the old one
    at its start and at its end, so a typo fix or an added word keeps every tag in place;
  - a removed word takes the whitespace before it; new tags go after the last word, before
    trailing whitespace.

  Equality (`PartialEq`, written by hand) compares the fields and leaves out `layout`, so two
  lines that differ only in spacing are equal items; a new field must be added there too. A
  done line with its priority between the `x` and the dates (`x (A) 2011-03-02 …`) is written
  in the specification's form even unchanged (`x 2011-03-02 … pri:A`).
- **Priority has two serialization forms, chosen by `done`:** an open item writes `(X)`
  before the description; a completed item writes `pri:X` (appended at the end of the line
  when rtm completes it). Both forms parse regardless of `done`; an unchanged line keeps the
  form it was read in. If a line carries both and the letters agree (`(C) … pri:C`), the
  duplicate is accepted; if the letters disagree
  (`(C) … pri:A`), `(C)` stays the priority and `pri:A` stays in the description as text. A
  malformed `pri:` value (`pri:xyz`) is text like any other unreadable tag (below).
- **`TodoItem::from_str` accepts every line.** A known tag (`due:`, `t:`, `rec:`, `uuid:`,
  `sub:`, `pri:`) whose value rtm cannot read and a leading `(X)` that is not A-Z (`(b)`, `(1)`)
  stay in the description as written, so `TodoLibrary::load`, which skips lines that fail to
  parse, never drops one and the next save keeps it.
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
- Business-day recurrence is currently approximated as daily (known limitation).

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
