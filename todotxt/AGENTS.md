# AGENTS.md — todotxt (core library)

## What This Component Is

The core Rust library for parsing, representing, and serializing Todo.txt files. No binary, no UI.
All three frontends depend on this crate via `path` dependency.

---

## Architecture / Structure

```
todotxt/src/
├── lib.rs              ← Public API re-exports
├── todoitem.rs         ← TodoItem struct + FromStr + Display
├── todolibrary.rs      ← TodoLibrary (load/save/CRUD)
├── todocontext.rs      ← @context tag
├── todoproject.rs      ← +project tag
├── todopriority.rs     ← (A)-(Z) priority
└── todorecurrence.rs   ← rec: tag (daily/weekly/monthly/yearly/business-day)
```

---

## Conventions

- **Edition:** Rust 2024
- **No external dependencies** except `chrono` (dates) and `uuid` (item identity).
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
- Tests live in `todolibrary.rs` (inline `#[cfg(test)]` module). Run with `cargo test -p todotxt`.
