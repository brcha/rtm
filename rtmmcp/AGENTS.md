# AGENTS.md — rtmmcp

The MCP server, a library that `rtmcli mcp` and `rtmapp mcp` run over stdio.

- `rmcp` is pinned to a minor version; its types stay inside the crate, and `schemars` is the version rmcp uses.
- Async stays inside: `serve` is synchronous and builds its own runtime; file and git calls run in `spawn_blocking`.
- stdout is the protocol: nothing else may print to it.
- Clients never name paths: a tool's `file` picks a configured source by name or path, and anything else is refused.
- Both frontends serve as `rtm` with the same tools, and one rtm server runs per client (`claim.rs`).
- A failed tool call is a result with `isError`, not a protocol error.
- Only `delete_item` is destructive: every other write sets `destructive_hint = false`, the reads `read_only_hint`.
- A change takes the item's number and its line as `list_items` printed it; on a mismatch it looks the line up, else refuses.
- A write holds the lock from before `RepoWrite::find` until after the push; every refusal before the save ends with "Nothing was changed.", and nothing after the save fails the tool.
- Edits keep the creation date, `uuid` and `sub` that the new line leaves out.
- Models act on the tool descriptions and instructions in `server.rs`; check a change to them with the [MCP eval](../doc/mcp-eval.md).
