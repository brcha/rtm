# AGENTS.md — rtmcli

The CLI (clap) on `todotxt`, `rtmsync`, `rtmupdate` and `rtmmcp`.

- No config file: everything comes from arguments and environment variables.
- Errors print `Error: <message>` on stderr; exit 1 for a state the user must fix, 2 for a usage error; a refused command writes nothing.
- `check-update` runs before the file is resolved and is the only command that uses the network.
- `mcp` takes the file from `-f` or `$TODOTXT` only, never the `todo.txt` fallback, and runs before the write lock and `Repo::find`: its server locks each change itself.
- Every other command takes the write lock in `main` before `Repo::find` and holds it through its commit and push; `delete` releases it while asking and checks the repository and the item again after a yes.
- `complete` and `delete` find their items before the pull and again, by equal line, after it; a changed or vanished item writes nothing.
- `--commit-changes` deliberately has no environment variable.
- No option may take `-V` as its short form (clap's `--version`).
- Only one test in `tests/cli_mcp_tests.rs` may complete the MCP handshake: servers with one parent process count as one client.
- cargo-deb needs `description`, `license` and `repository` in `[package]` and the asset path `target/release/rtmcli` as written; do not "fix" either.
