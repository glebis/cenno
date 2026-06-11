# AGENTS.md

Guidance for AI agents (and humans) working on this codebase.

## What this is

cenno is a macOS menu-bar app (Tauri 2: Rust backend + React/TypeScript frontend) that lets MCP-capable AI agents ask the user questions through small, non-activating floating panels. The agent calls an `ask_user` MCP tool, a panel slides in without stealing keyboard focus, and the answer comes back as structured data (`{answer, via, elapsed_s}`). Every exchange is recorded in a local SQLite database. See [README.md](README.md) for the user-facing story.

## Structure

```
src/                    React frontend (panels)
  PromptPanel.tsx       the floating panel UI
  a2ui/                 A2UI v0.9 rich-layout rendering
  styles/               CSS generated from tokens — do not edit by hand
src-tauri/src/          Rust backend
  mcp.rs                MCP server (ask_user tool, stdio bridge)
  protocol.rs           prompt/answer types shared across the boundary
  bridge.rs             frontend <-> backend wiring
  db.rs                 SQLite history (~/Library/Application Support/app.cenno/cenno.db)
  tray.rs               menu-bar UI, pause/quiet-mode policy
  suppress.rs           fullscreen detection, prompt queueing/replay
  cli.rs                `cenno ask`, `cenno export`, `--tray`, `--mcp-stdio`
  updater.rs            GitHub-releases updater (user-initiated only)
  a2ui_guard.rs         A2UI payload validation at the boundary
tokens/tokens.json      W3C DTCG design tokens — single source of truth for styling
docs/design/            TOKENS.md (design system), BRAND.md (the mark)
docs/superpowers/       spec → plan → review trail the app was built from
skills/cenno/           agent skill teaching ask_user etiquette + setup
scripts/                smoke.sh, demo.sh (fire test prompts), visual-qa.sh
```

## Build & test

Requirements: Rust stable, Node 20+, Xcode Command Line Tools.

```bash
npm install
npm run dev & npm run tauri dev     # live frontend + app for development

cargo test                          # Rust tests (run inside src-tauri/)
npx vitest run                      # frontend tests
npm run typecheck:tests             # typecheck the test suite
npm run tokens                      # rebuild CSS from tokens/tokens.json (validates first)

npx tauri build --no-bundle         # unsigned release binary → src-tauri/target/release/cenno
./scripts/demo.sh all               # fire one demo prompt of each kind against a running app
```

### Gotchas

- A plain `cargo build` binary loads the Vite dev server (port 1430) and shows a blank page unless `npm run dev` is running.
- `npx tauri build` fails at bundling with `failed to run xattr` when a conda/python `xattr` shadows the system one — prefix the build with `PATH="/usr/bin:$PATH"`.
- "Panel doesn't appear" during manual testing is usually fullscreen quiet mode working as designed (stderr says `suppressed (paused or fullscreen) — queued for replay`), not a bug.
- CSP is only enforced in bundled builds — test security changes against `npx tauri build` output, never `tauri dev`.
- Never edit generated CSS in `src/styles/` directly; change `tokens/tokens.json` and run `npm run tokens`.
- The app's data is the user's: `cenno.db` is `0600`, local-only. Don't add network calls — the only permitted network access is the user-initiated updater (and a one-time SpeechTranscriber model download for voice).
- `voice.rs` bridges a Swift package (`swift/`, SpeechTranscriber) via swift-rs. `cargo test`/`cargo run` need the system Swift runtime on the loader path; build.rs bakes an `-rpath /usr/lib/swift` so they work without a `DYLD_*` env (which macOS strips).
- External config lives in `~/.cenno/` (`config.json` + `tokens.json`); loader is `config.rs`, applied in `lib.rs` setup and exposed to the webview via `get_user_config`/`get_user_tokens` (frontend: `src/userConfig.ts`). See [docs/CONFIG.md](docs/CONFIG.md).

## Releasing

Full steps in [README.md → Releasing an update](README.md#releasing-an-update): bump versions in `src-tauri/tauri.conf.json` + `package.json`, build signed with the updater key, publish the DMG + `cenno.app.tar.gz` + `.sig` + `latest.json` on the GitHub release.

Also bump the Homebrew cask after each release: `version` and `sha256` in [`glebis/homebrew-tap`](https://github.com/glebis/homebrew-tap) `Casks/cenno.rb` (sha256 of the new DMG).

## Conventions

- Bundle identifier is `app.cenno`; keep paths user-agnostic (no hardcoded home directories).
- Validate everything that crosses the agent boundary (see `a2ui_guard.rs`, `protocol.rs`) — prompts are untrusted input.
- [SECURITY.md](SECURITY.md) has the threat model; [AUTHORSHIP.md](AUTHORSHIP.md) records authorship.
