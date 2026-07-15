# L1a Accessibility screen context — verification scenarios

Run these with the shared [L0 screen-capture security matrix](2026-07-15-l0-screen-capture-security.md). L1a is on-demand, Accessibility-only, and returns semantic text—not pixels or OCR.

## Automated scenarios

| Scenario | Setup | Expected evidence |
|---|---|---|
| Tool discovery | List MCP tools | `get_screen_context` is listed with optional `include_visible_text` and `max_chars` |
| Typed statuses | Inject `ok`, `permission_denied`, `ax_unavailable`, and policy-blocked readers | Every status is a successful tool result; only unexpected reader/JSON failure is an MCP error |
| Cost bound | Omit, set 0, and set above 8000 | Effective bounds are 8000, 1, and 8000 characters |
| Whole-field redaction | Put high-confidence secrets in title, URL, selection, and visible text | No original secret crosses the socket; `redaction_count` covers every replacement |
| Denylist non-leakage | Deny the fake bundle or host | `blocked` plus reason; all app/title/URL/role/text fields are null |
| Kill-switch race | Disable capture while the injected reader is active | Final release is blocked and the indicator returns idle |
| Thin tree | Return app/window metadata without selection, visible text, or URL | `ax_unavailable`, not an error |

Run:

```bash
cd src-tauri
cargo test --lib protocol::tests::screen_context_
cargo test --lib screen_context::tests
cargo test --test mcp_socket get_screen_context -- --nocapture
swift test --package-path swift --filter CennoScreenContextTests
```

## Installed-app scenarios

1. Reset or deny Accessibility permission, make one real tool call, and confirm `permission_denied` is returned successfully without polling or a prompt loop. Grant permission only by user choice, then call again.
2. Focus Notes, select known text, and call with visible text enabled. Confirm app name, bundle ID, window title, focused role, and exact selection; returned content is marked `untrusted: true`.
3. Focus a Notes document longer than 8000 characters. Confirm bounded output and `truncated: true`; repeat with a smaller explicit `max_chars`.
4. Focus Safari or Chrome's address field. A valid direct URL may be returned. If it is not directly exposed, `url: null` is correct; cenno must not force enhanced browser accessibility.
5. Focus a known Electron/canvas surface with no selected text, direct value, or URL. Confirm `ax_unavailable`, proving the L1b hand-off contract.
6. Add the focused app bundle ID and then a browser host to the user denylist. Confirm content-free `blocked` results with the matching reason.
7. Put test secret patterns in Notes title, selection, and visible content. Confirm all are redacted before the MCP result and the count is accurate.
8. Open the tray during a call. Confirm `Reading screen context…` appears only while a lease is active. Turn capture off and confirm the next call blocks before AX; turn it back on for later tests.
9. Observe process connections before and during a call. Confirm the screen-context path opens no new cenno connection. Separately note that the requesting agent may transmit returned context to its model provider.
10. Confirm no Screen Recording prompt appears and the installed app has no new restricted entitlement.

Record the app build, macOS version, focused apps, exact returned statuses, test counts, and any best-effort URL limitation on `cenno-jc6.1`.
