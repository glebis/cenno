# Contributing to cenno

## Prerequisites

- Rust stable
- Node.js 20+
- Xcode Command Line Tools (macOS)

## Development Setup

```bash
git clone https://github.com/glebis/cenno.git
cd cenno
npm install
npm run dev & npm run tauri dev   # live frontend + app
```

Useful commands:

```bash
cargo test                 # Rust (run in src-tauri/)
npx vitest run             # frontend
npx tsc --noEmit           # frontend typecheck
npm run typecheck:tests    # test-file typecheck
npm run tokens             # rebuild CSS from tokens/tokens.json
./scripts/demo.sh all      # fire one demo prompt of each kind
```

Note: a plain `cargo build` binary loads the Vite dev server (port 1430) and
shows a blank page unless `npm run dev` is running. CSP is only enforced in
bundled builds — test security changes against `npx tauri build` output.

## Project Conventions

- **Design tokens only**: UI styles consume semantic theme vars and token
  vars (see docs/design/TOKENS.md); never hardcode colors or sizes.
- **Catalog controls**: adding a UI control follows the recipe in
  docs/design/CONTROLS.md — tests first, view + adapter + registration.
- **Tests first**: changes come with tests; the existing suites
  (`cargo test`, `npx vitest run`) must stay green.

## Licensing Of Contributions

cenno is licensed under the Apache License 2.0. Unless you explicitly say
otherwise in writing, contributions intentionally submitted to this
repository are accepted under the same Apache-2.0 terms.

By contributing, you certify that you have the right to submit the work and
that it does not include code copied from unlicensed or license-incompatible
sources.

## Reporting Issues

- Bugs and feature requests: GitHub issues.
- Security issues: see [SECURITY.md](SECURITY.md) — report privately.
