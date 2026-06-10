---
name: cenno-setup
description: Install, verify, and wire up the cenno app so agents in a project can ask the user questions. Use when cenno's ask_user tool is missing from a project, when the user says "set up cenno", "add cenno to this project", "let me answer prompts from this agent", or when an ask_user call failed with "not running / not found". Adds cenno to a project's MCP config and confirms the binary + socket are reachable. macOS only.
---

# Setting up cenno for a project

cenno is a macOS app. Once installed and added to a project's MCP config, any agent in that project gets an `ask_user` tool (see the `cenno` skill for using it).

## 1. Confirm it's installed

The canonical binary path is:

```
/Applications/cenno.app/Contents/MacOS/cenno
```

Check it:

```bash
test -x /Applications/cenno.app/Contents/MacOS/cenno && echo "cenno installed" || echo "missing"
```

If missing, tell the user to install it: **download the latest DMG from
https://github.com/glebis/cenno/releases/latest** (signed & notarized, Apple Silicon, macOS 12+) and drag cenno to Applications. Do not try to build it for them unless they ask. On first launch it lives in the menu bar and registers to launch at login.

If it's installed somewhere non-standard, find it:

```bash
mdfind "kMDItemCFBundleIdentifier == 'app.cenno'" 2>/dev/null
```

## 2. Add it to the project's MCP config

Put this in the project's `.mcp.json` (or merge into an existing `mcpServers` block). Use the absolute binary path from step 1:

```json
{
  "mcpServers": {
    "cenno": {
      "command": "/Applications/cenno.app/Contents/MacOS/cenno",
      "args": ["--mcp-stdio"]
    }
  }
}
```

The `--mcp-stdio` bridge auto-launches the app in the background if it isn't already running, so nothing else needs to be started. After editing `.mcp.json`, the MCP client must be restarted/reloaded for the new server to register (e.g. restart the Claude Code session).

## 3. Verify the round-trip

From the shell, confirm the binary answers (this prints a result and times out cleanly if no one answers):

```bash
/Applications/cenno.app/Contents/MacOS/cenno ask "cenno setup check — tap or ignore" --timeout 8
```

- A panel appears, and your answer (or `{"answered": false, ...}` on the 8s timeout) prints as JSON → working.
- `cenno is not running — start it or use 'cenno --mcp-stdio'` → the app couldn't start; check step 1.

The local socket lives at `~/Library/Application Support/app.cenno/mcp.sock`; the history database (every answered/timed-out prompt) at `~/Library/Application Support/app.cenno/cenno.db` (`cenno export` dumps it as JSON/CSV).

## Notes

- **macOS only.** On other platforms there is no cenno; agents should fall back to another prompt mechanism.
- **Quiet mode**: the user can pause cenno or have it stay silent in fullscreen (tray menu). That's intentional — prompts then time out instead of appearing. It's not a setup failure.
- Nothing leaves the machine: cenno makes no network calls of its own except the explicit "Check for updates…" tray action.
