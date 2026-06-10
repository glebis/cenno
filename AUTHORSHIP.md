# Authorship

*Last updated: 2026-06-10*

## Human Author

**Gleb Kalinin** (Berlin, Germany) — Product concept, creative direction, and
review. Contact: glebis@gmail.com

The product idea, interaction model, and acceptance decisions are
human-authored. This includes:

- The core concept: a focus-preserving way for MCP-capable agents to ask the
  human questions and receive structured answers
- Product decisions: non-activating panels, one question per screen,
  suppression policy (pause / fullscreen quiet mode), local-only history,
  privacy posture (no network calls except user-initiated update checks)
- Brand and visual direction: the *cenno* name and etymology, Reporter-style
  full-bleed flow colors, the gesture-pictogram brand language
  (docs/design/BRAND.md)
- Iterative review and acceptance of designs, plans, and implementations at
  every stage

## AI Implementation

cenno was implemented end-to-end by **Claude** (Anthropic) AI agents under
the human direction above — including architecture proposals, specs, plans,
code, tests, and code reviews. The complete spec → plan → review trail is
preserved in [docs/superpowers/](docs/superpowers/) as contemporaneous
evidence of the process; development session logs are retained locally.

The human author provided:

- The product brief and constraints that shaped all output
- Selection among proposed designs and architectures
- Iterative review, rejection, and refinement of generated work
- Final acceptance of what ships

## Copyright Notice

Product concept and creative direction copyright (c) 2026-present Gleb
Kalinin. Implementation was assisted by Claude (Anthropic) under human
direction and review. Provider output terms are not treated as a substitute
for source provenance, license compatibility review, or human authorship
documentation.

## AI Provider Output Terms

cenno's release posture does not rely on provider terms alone, but the
project records the relevant output-rights claims for transparency:

- Anthropic states that its Commercial Terms let customers "retain ownership
  rights" over generated outputs:
  https://www.anthropic.com/news/expanded-legal-protections-api-improvements

These terms support commercial use of assisted implementation output, but
they do not eliminate the need for human authorship, provenance review,
dependency-license review, or checks for copied public code.

## Why This File Exists

Copyright protection for AI-assisted works depends on human authorship and
jurisdiction-specific originality standards. This file documents the human
creative process behind cenno so the project can distinguish human concept,
selection, arrangement, and review from machine-assisted implementation.
The docs/superpowers/ trail provides additional evidence of how decisions
were made.
