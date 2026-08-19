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
└── todo_recurrence.rs   ← rec: tag (daily/weekly/monthly/yearly/business-day)
```

---

## Conventions

- **Edition:** Rust 2024
- **External dependencies:** `chrono` (dates), `uuid` (item identity), `thiserror` (parse
  error types).
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
  `add_item` above.
- `uncomplete_item(index)` clears `done` and `completion_date`, returns `Option<()>` (None =
  out of bounds). Priority and creation date are untouched — priority re-serializes as `(X)`
  automatically because `Display` keys that choice off `done`.
- `TodoItem` fields: `done`, `priority`, `completion_date`, `creation_date`, `description`,
  `projects`, `contexts`, `due`, `recurrence`, `threshold`, `uuid`, `sub`.
- **Priority has two serialization forms, chosen by `done`:** an open item writes `(X)`
  before the description; a completed item writes `pri:X` after it. Both forms parse
  regardless of `done`, so a file is normalized to the canonical form for its completion
  state on the next save. If a line carries both and the letters agree (`(C) … pri:C`), the
  duplicate is silently accepted and collapses to one form on save; if the letters disagree
  (`(C) … pri:A`), parsing returns `TodoItemParseError::ConflictingPriority`. A malformed
  `pri:` value (`pri:xyz`) is not an error — it falls through to plain description text,
  unlike the strict `due:`/`rec:`/`t:`/`uuid:`/`sub:` tags, because `TodoLibrary::load`
  silently drops any line that fails to parse and a hard error there would mean silent data
  loss.
- **`Display` never emits a creation date without a completion date on a done line.** A
  completed item's creation date is written only immediately after its completion date; if
  there is no completion date, no date is written at all, even if a creation date is present.
  This is deliberate: a lone date on a done line is indistinguishable from a completion date
  on the next parse, so writing one would silently turn a creation date into a different
  fact. It is also why fabricating dates only ever happens at `add_item`/`complete_item`, not
  in `Display`.
- Business-day recurrence is currently approximated as daily (known limitation).
- Monthly recurrence is approximated as 30 days; yearly as 365 days.

---

## Important Notes

- `uuid` and `sub` fields exist in the data model for future subtask support but are not yet
  surfaced in any UI.
- Tests live inline in each module's `#[cfg(test)]` block. Run with `cargo test -p todotxt`.
