# AGENTS.md — todotxt

The core library: parsing, printing and changing todo.txt files. No UI, no binary.

- Parse errors are `thiserror` enums keeping their source with `#[from]` and deriving `Clone, Debug, Eq, PartialEq`.
- `TodoItem::from_str` accepts every line: an unreadable known tag or priority stays text in the description, as `load` drops lines that fail to parse.
- `Display` writes a line back byte for byte unless a field changed, and then patches only that part.
- `TodoItem`'s `PartialEq` is written by hand and ignores `layout`: a new field must be added there.
- Only `add_item` stamps creation dates; `Display` never writes a creation date on a done line without a completion date (it would read back as the completion date).
- Library mutations never save: the caller saves and, in a repository, commits.
- `save` writes through `write_atomic`; never write a todo file in place.
- `archive` appends to the archive before removing the items, so an interruption duplicates them, never loses them; the caller saves and commits both files.
- `check_dates` and `check_one_line` are for entered text: every frontend calls them, the library never does, so existing lines load and save unchanged.
- The lock directory (`write_lock::lock_dir()`) is never next to the todo file and never depends on the environment, as MCP clients start servers without `XDG_RUNTIME_DIR` or `TMPDIR`.
- Spec tests (`tests/spec_*_tests.rs`) name the rule they check; one failing on a known deviation is `#[ignore = "RTM-NN: …"]` with the fixing issue, whose fix removes it.
