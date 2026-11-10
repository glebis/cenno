# Setting up cenno

Use this when the `ask_user` tool is missing from a project, or an `ask_user` call fails with "not running / not found". **macOS only** — on other platforms there is no cenno; use the fallback order in SKILL.md.

## 1. Confirm it's installed

```bash
test -x /Applications/cenno.app/Contents/MacOS/cenno && echo installed || echo missing
```

If missing, tell the user how to install it (don't build it unless asked):

- **DMG:** the latest from https://github.com/glebis/cenno/releases/latest (signed & notarized, Apple Silicon, macOS 12+); drag cenno to Applications.
- **Homebrew:** `brew install --cask glebis/tap/cenno`

For a non-standard location: `mdfind "kMDItemCFBundleIdentifier == 'app.cenno'"`.

## 2. Add it to the project's MCP config

Merge into the project's `.mcp.json`, using the absolute path from step 1:

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

`--mcp-stdio` auto-launches the app in the background. The MCP client must be restarted or reloaded to pick up the server (e.g. restart the Claude Code session).

## 3. Verify the round-trip

```bash
/Applications/cenno.app/Contents/MacOS/cenno ask "cenno setup check — tap or ignore" --timeout 8
```

A panel appears and the answer (or `{"answered": false, ...}` after 8s) prints as JSON → working. `cenno is not running — start it…` → it couldn't launch; recheck step 1. The socket lives at `~/Library/Application Support/app.cenno/mcp.sock`.

## Driving cenno over a raw socket (no MCP client)

To call cenno from a plain script with no MCP runtime, pipe JSON-RPC through the stdio bridge — initialize, then `tools/call`:

```bash
BIN=/Applications/cenno.app/Contents/MacOS/cenno
printf '%s\n' \
 '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"deploy-script","title":"Deploy script","version":"1"}}}' \
 '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
 '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"ask_user","arguments":{"title":"Ship it?","input":{"kind":"confirm"},"timeout_s":60}}}' \
 | "$BIN" --mcp-stdio
```

- `clientInfo.title` (else `name`) is what the panel header shows as the asker — name the script something the user will recognise.
- Read stdout line by line until the line whose `id` is `2`; its `result.content[0].text` is the answer JSON. Keep stdin open until then.
- If cenno is cold, the first call can lose a socket race — launch `"$BIN" --tray` and wait for `~/Library/Application Support/app.cenno/mcp.sock` to exist before the first `ask_user`.
