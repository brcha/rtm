# AGENTS.md — rtmcli (CLI frontend)

## What This Component Is

A command-line interface for the Todo.txt manager. Uses `clap` for argument parsing and the
`todotxt` library for all data operations.

---

## Architecture / Structure

```
rtmcli/src/
└── main.rs     ← Single file: CLI definition + all command handlers
rtmcli/tests/   ← Integration tests
```

---

## Commands

| Command    | Description                                              |
|------------|----------------------------------------------------------|
| `list`     | List items, optionally filtered by completion and date   |
| `add`      | Add a new item in Todo.txt format                        |
| `complete` | Complete items by filter, index, or UUID                 |

---

## Conventions

- **Edition:** Rust 2024
- File path resolved from: `-f <file>` flag → `$TODOTXT` env var → `todo.txt` (cwd fallback).
- Filters: `today`, `week`, `all`, `overdue`, `future` (date-range based).
- `complete` accepts: a filter name (optionally + index/UUID), an absolute index, or a UUID.
- No config file — stateless, all options via CLI args/env.

---

## Build & Run

```sh
cargo build -p rtmcli
cargo run -p rtmcli -- -f ~/todo.txt list today
cargo test -p rtmcli
```
