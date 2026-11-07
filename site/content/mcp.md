+++
title = "Use rtm with AI assistants"
description = "Let Claude Desktop, Claude Code or another MCP client read and change your todo.txt through rtmcli or the rtm desktop app."
+++

Both programs include an [MCP](https://modelcontextprotocol.io) server, so an AI assistant such as
Claude Desktop or Claude Code can list your items, projects and contexts, and add, edit, complete,
reopen and delete items. Use whichever program you installed:

- `rtmcli mcp` serves one file, from `-f` or `$TODOTXT`.
- `rtmapp mcp` serves the files open in the desktop app (its `[[files]]` in `rtm/config.toml`), the
  shown one first, and picks up files you add or rename there. The app does not need to be running,
  and no window opens.

Add exactly one rtm server to each client, `rtmcli mcp` or `rtmapp mcp`, not both, and name it
`rtm`. A second rtm server in the same client offers no tools.

Clients may start the server without your shell's environment. Pass the variables it needs
explicitly (`-e` in Claude Code, `env` in Claude Desktop): `TODOTXT`, `RTM_NO_SYNC` and `RTM_VCS`
for `rtmcli mcp`, and `XDG_CONFIG_HOME` for `rtmapp mcp` if you set it (GNU/Linux), so that it finds
`rtm/config.toml`.

The server runs on your computer and talks to the client over stdin and stdout. It uses the network
only to sync with your repository's remote; the items the assistant reads reach its AI service like
anything else you show it. It can use only the files listed above, never a path the assistant
names. Changes are saved and synced as rtmcli saves and syncs them: each change is committed and
pushed at once when the file is in a Git or Pijul repository (see
[Syncing](https://github.com/brcha/rtm#syncing-through-git-or-pijul) in the README), and the
desktop app, if it is open, shows them.
Deleting is marked as destructive, so clients that confirm tool calls ask you first.

Where the programs are, once [installed](@/install.md):

| Installed from | `rtmcli`                            | `rtmapp`                                                         |
|----------------|-------------------------------------|------------------------------------------------------------------|
| deb, RPM       | `/usr/bin/rtmcli`                   | `/usr/bin/rtmapp`                                                |
| AppImage       |                                     | the `.AppImage` file itself, wherever you keep it                |
| macOS          | where you moved it                  | `/Applications/Rusty Todo.txt Manager.app/Contents/MacOS/rtmapp` |
| Windows        | where you moved it                  | `C:\Program Files\Rusty Todo.txt Manager\rtmapp.exe`             |
| Nix profile    | `~/.nix-profile/bin/rtmcli`         | `~/.nix-profile/bin/rtmapp`                                      |
| NixOS system   | `/run/current-system/sw/bin/rtmcli` | `/run/current-system/sw/bin/rtmapp`                              |

## Claude Code

```sh
claude mcp add --scope user rtm -- rtmcli mcp -f ~/todo.txt
```

or, for the desktop app's files, `claude mcp add --scope user rtm -- rtmapp mcp` (on macOS, the full
path from the table). `--scope user` makes it available in every directory; without it, only in the
current one. On Windows, quote paths with spaces:
`claude mcp add --scope user rtm -- "C:\Program Files\Rusty Todo.txt Manager\rtmapp.exe" mcp`.
Environment variables go before the name: `claude mcp add --scope user -e RTM_VCS=git rtm -- ...`.

## Claude Desktop

Open **Settings > Developer > Edit Config**, which opens `claude_desktop_config.json`
(`~/Library/Application Support/Claude/` on macOS, `%APPDATA%\Claude\` on Windows), and add the
server under `mcpServers`:

```json
{
  "mcpServers": {
    "rtm": {
      "command": "/usr/local/bin/rtmcli",
      "args": ["-f", "/Users/me/todo.txt", "mcp"]
    }
  }
}
```

or, for the desktop app's files:

```json
{
  "mcpServers": {
    "rtm": {
      "command": "/Applications/Rusty Todo.txt Manager.app/Contents/MacOS/rtmapp",
      "args": ["mcp"]
    }
  }
}
```

Then restart Claude Desktop. Claude Desktop does not start the server from a shell, so:

- Use absolute paths for the command and the file: `~` and environment variables are not expanded.
  On Windows, write backslashes twice (`"C:\\Users\\me\\todo.txt"`).
- Your shell's environment does not reach the server. Put the variables it needs (see above) in an
  `env` object next to `args`, for example `"env": {"RTM_VCS": "git"}`. `rtmapp mcp` takes its
  other settings, sync included, from `rtm/config.toml`.
- Syncing runs the `git` or `pijul` on the `PATH` the client gives the server. If yours is
  elsewhere (Homebrew's `/opt/homebrew/bin`, for example), set `PATH` in `env` too; without it,
  changes are saved but not synced.

## Other clients

Any MCP client that starts local servers over stdio works the same way: the command is `rtmcli`
with the arguments `-f <file> mcp`, or `rtmapp` with `mcp`.
