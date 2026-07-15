# Security Policy

## Reporting a Vulnerability

Report security issues privately to `glebis@gmail.com`. Do not open public
issues for vulnerabilities involving the MCP socket, prompt injection through
`a2ui` payloads, history database access, or the update channel.

Include: affected version/commit, macOS version, reproduction steps, and
relevant logs with private paths redacted.

## Threat Model

cenno is a local-first Tauri 2 menu-bar app. Answer history and captured screen
context are stored locally. Network-capable features are separately bounded:
the optional CloudKit companion relay, the explicitly user-initiated update
check, and an optional one-time SpeechTranscriber model download. Screen
capture adds no new cenno network path. The attack surface is the local
machine, the MCP socket, agent-supplied prompt content, and untrusted screen
content returned to an agent.

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
permissions. Answers are stored in plaintext; FileVault covers data at rest.

### Screen capture and context

- Screen text can be attacker-controlled. cenno returns it as
  `captured_content` with `untrusted: true`; agents must treat it as quoted
  data, never instructions.
- Before return or storage, the Rust capture guard checks the global switch,
  exact bundle-id and host/subdomain denials, then redacts high-confidence
  private-key, AWS-key, JWT, and `sk-` token shapes.
- Accessibility reads have no macOS recording indicator, so cenno shows its
  own tray state and provides a persisted global off switch. Passive sampling
  is off by default.
- Accessibility and Screen Recording are separate, lazily requested macOS
  permissions. A denial is a normal typed outcome, not permission to bypass
  the guard.

Pattern redaction is deliberately conservative and cannot catch every secret.
Visible private content remains capturable outside denied apps and hosts.
Captured context is delivered to the requesting agent and may reach that
agent's model provider; cenno cannot control the agent's network or retention
policy. The precise claim is: cenno processes and stores captured context
locally, and screen capture adds no cenno network path — not that screen
content can never leave the machine.

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
