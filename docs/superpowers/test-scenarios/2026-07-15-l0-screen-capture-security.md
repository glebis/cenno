# L0 screen-capture security — verification scenarios

Use these scenarios for L0 itself and rerun them whenever L1a, L1b, or L2 adds
a new capture or storage path. A capture feature is not releasable if it cannot
identify the guard call that satisfies each applicable scenario.

L1a's Accessibility-specific matrix is in
[`2026-07-15-l1a-screen-context-ax.md`](2026-07-15-l1a-screen-context-ax.md).
Run this shared L0 matrix alongside it and alongside every later capture path.

## Automated policy scenarios

| Scenario | Input/state | Expected evidence |
|---|---|---|
| Global stop wins | capture off; denied app; secret text | `capture_disabled`; no content |
| Built-in sensitive app | 1Password, Bitwarden, KeePassXC, or Keychain bundle id | `denied_bundle`; no content |
| User app denial | exact configured bundle id | `denied_bundle`; case or prefix lookalikes remain allowed |
| User host denial | configured host and a subdomain | `denied_host`; `not<host>` remains allowed |
| Secret redaction | PEM private key, AWS `AKIA…`, JWT, long `sk-…` | one placeholder per secret; accurate count |
| False-positive control | ordinary prose, short `sk-` example | text unchanged |
| Provenance | any allowed content | `captured_content` plus `untrusted: true` |
| Redaction opt-out | redaction disabled, allowed source | original text, zero redactions, still untrusted |
| Overlapping reads | two active leases | indicator remains active until both leases drop |
| Toggle race | switch off during a read | final guard check blocks return/storage |

Run:

```bash
cd src-tauri
cargo test --lib capture_guard::tests
cargo test --lib tray::tests
```

## Installed-app scenarios

1. Launch the rebuilt `/Applications/cenno.app` and open its tray menu.
   Confirm `Screen context allowed` is checked.
2. Turn it off, quit, relaunch, and confirm `Screen context off` persists.
   Turn it on again and repeat the relaunch check.
3. Confirm launch alone requests neither Accessibility nor Screen Recording;
   permissions remain lazy until a real L1 capture call.
4. With L1a present, start one read and confirm the tray label changes to
   `Reading screen context…`, then returns to idle. Start overlapping reads and
   confirm it does not return to idle after only the first completes.
5. Focus a denylisted app and a configured denied host. Confirm the typed block
   outcome contains no title, URL, text, image, or OCR-derived content.
6. Focus a document containing an injection-style instruction and test secret.
   Confirm the instruction remains quoted under `captured_content`, the secret
   is redacted, and the receiving agent does not obey the captured instruction.

## Build and regression gates

```bash
cd src-tauri
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cd ..
npm run typecheck:tests
npx vitest run
npm run build
PATH="/usr/bin:$PATH" npx tauri build
```

The bundled build is mandatory for security verification because development
mode does not enforce the production CSP. Inspect the built app entitlements;
L0 must add neither Accessibility/Screen Recording prompts at startup nor any
restricted entitlement.
