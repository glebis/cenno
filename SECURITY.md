# Security Policy

## Reporting a Vulnerability

Report security issues privately to `glebis@gmail.com`. Do not open public
issues for vulnerabilities involving the MCP socket, prompt injection through
`a2ui` payloads, history database access, or the update channel.

Include: affected version/commit, macOS version, reproduction steps, and
relevant logs with private paths redacted.

## Threat Model

cenno is a local-first Tauri 2 menu-bar app. It makes no network connections
on its own; the one exception is the explicitly user-initiated update check.
All answer history stays on disk. The attack surface is the local machine,
the MCP socket, and agent-supplied prompt content.

### MCP Unix Socket (`mcp.sock`)

- Located in `~/Library/Application Support/app.cenno/`; connections are not
  further authenticated — file permissions are the access-control boundary.
- Any local process running as the same user can connect and prompt the user
  or receive their answers. This matches the macOS security model:
  same-user processes already have equivalent filesystem access.
- The exposed surface is deliberately small: a single `ask_user` tool.

### Agent-supplied content (`a2ui` payloads)

- Raw `a2ui` payloads are validated at the MCP boundary
  (`src-tauri/src/a2ui_guard.rs`): envelope shape, catalog id, component
  count cap (200), and size cap (256 KiB). Malformed payloads are rejected
  with actionable errors instead of reaching the webview.
- The webview enforces CSP in bundled builds (`tauri.conf.json`); images are
  restricted to `'self'` and `data:`. Markdown links never navigate the
  panel — they open externally via the opener plugin.
- Component schemas are advisory at runtime; adapters defensively narrow
  props (see docs/design/CONTROLS.md, "Known limitation").

### History database

- `~/Library/Application Support/app.cenno/cenno.db`, created with `0600`
  permissions. Answers are stored in plaintext; FileVault covers data at
  rest. Nothing is transmitted anywhere.

### Update channel

- "Check for updates…" queries GitHub releases over HTTPS and verifies
  update artifacts against the minisign public key pinned in
  `tauri.conf.json` before installing. A compromised GitHub release without
  a valid signature will not install.
- The signing private key is held offline by the maintainer and is not in
  this repository.

### CSP and dev builds

- CSP is only enforced in bundled builds. Test security-relevant changes
  against `npx tauri build` output, never `tauri dev`.
