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
- `complete_item(index)` marks done, handles recurrence (appends new item), returns
  `Option<bool>` (None = out of bounds, Some(true) = had recurrence, Some(false) = no recurrence).
- `TodoItem` fields: `done`, `priority`, `completion_date`, `creation_date`, `description`,
  `projects`, `contexts`, `due`, `recurrence`, `threshold`, `uuid`, `sub`.
- Business-day recurrence is currently approximated as daily (known limitation).
- Monthly recurrence is approximated as 30 days; yearly as 365 days.

---

## Important Notes

- `uuid` and `sub` fields exist in the data model for future subtask support but are not yet
  surfaced in any UI.
- Tests live inline in each module's `#[cfg(test)]` block. Run with `cargo test -p todotxt`.
