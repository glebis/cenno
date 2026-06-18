Findings (ordered by severity):

- **BLOCKER**
1. [companion/Sources/iPhone/CennoComponentCatalog.swift](/Users/glebkalinin/ai_projects/cenno/companion/Sources/iPhone/CennoComponentCatalog.swift) (Task 8) — `CustomComponentCatalog` contract is likely mismatched: the plan uses `-> some View` while the reference contract in your own plan/spec shows `associatedtype Output: View` and `-> Output`. This is a compile risk that blocks the target from building.
  - Fix: implement the protocol with a concrete `typealias Output = AnyView` and return `AnyView(...)`, or match whatever exact signature the vendored `CustomComponentCatalog` requires.
- **BLOCKER**
2. [companion/Sources/iPhone/CennoComponentCatalog.swift](/Users/glebkalinin/ai_projects/cenno/companion/Sources/iPhone/CennoComponentCatalog.swift) (Task 8) — `dc.resolve(.dataBinding(path: ""))` does not match the documented `DataContext` API surface (`resolveDynamicValue(_:)`, `stringBinding`, `doubleBinding`, `set`). This is likely an immediate compile/runtime API mismatch.
  - Fix: replace with API-proven path via `resolveDynamicValue` (or equivalent exact method from the vendored `a2ui-swift`), and avoid fallback on synthetic empty binding paths.

- **SHOULD-FIX**
3. [companion/Sources/Shared/A2UIDesugar.swift](/Users/glebkalinin/ai_projects/cenno/companion/Sources/Shared/A2UIDesugar.swift) (Task 4) — fidelity gap vs `src/a2ui/desugar.ts`: custom widget/template expansion (`widgets` parameter + custom kind fallback path) is dropped. `desugar.ts` includes this behavior and tests include it (`custom widget: input.kind matching a configured template...`).
  - Fix: either add widget support in Swift (by decoding/attaching config templates into `PromptPayload`) or explicitly document and gate this as unsupported behavior (with tests asserting that unsupported kinds fall back to text).

4. [companion/Sources/Shared/A2UIAnswerBridge.swift](/Users/glebkalinin/ai_projects/cenno/companion/Sources/Shared/A2UIAnswerBridge.swift) (Task 6) — value serialization is stricter than `PromptPanel.tsx`: non-string/non-number/bool/null values collapse to `""` instead of `String(raw)`.
  - Fix: after array-unwrapping/null normalization, preserve JS-like stringification semantics for all remaining JSONValue types (including fallback `String(describing:)` behavior), while keeping `null/undefined -> ""`.

5. [companion/Sources/Shared/A2UIPromptView.swift](/Users/glebkalinin/ai_projects/cenno/companion/Sources/iPhone/A2UIPromptView.swift) + [companion/Sources/Shared/A2UIMessageBuilder.swift](/Users/glebkalinin/ai_projects/cenno/companion/Sources/Shared/A2UIMessageBuilder.swift) (Tasks 7/9) — no parity fallback for malformed raw `a2ui` payloads.
  - PromptPanel on web desugars/falls back to `desugar(prompt)` if render/build fails. iOS currently surfaces build error instead of fallback.
  - Fix: on passthrough decode/process failure, attempt `A2UIDesugar.messages(for:)` and render fallback; keep build error only if both fail.

6. [companion/Sources/Shared/CennoComponentRemap.swift](/Users/glebkalinin/ai_projects/cenno/companion/Sources/Shared/CennoComponentRemap.swift) + [companion/Sources/iPhone/CennoComponentCatalog.swift](/Users/glebkalinin/ai_projects/cenno/companion/Sources/iPhone/CennoComponentCatalog.swift) (Tasks 5/8) — architecture risk #1 remains unproven: basic Button/rendering of `CennoText` labels and custom children in basic containers is assumed, not validated.
  - Fix: add a targeted render-level test before full integration (or in a small preview + instrumentation) that asserts `Button`/`Row`/`Column` containing remapped leaf nodes render and submit.
  - If it fails, broaden remap to include `Button` (or custom wrappers for button/containers) as the plan’s own contingency notes.

- **NICE-TO-KNOW**
7. [companion/project.yml](/Users/glebkalinin/ai_projects/cenno/companion/project.yml) (Task 1/4) — XcodeGen shape for local package + `CennoSharedTests` target is generally sound, but keep an eye on generated scheme: ensure `.test.targets` stays attached to `CennoiPhone` and that `xcodebuild test` is always run against that scheme.
  - Fix: add a one-line CI-equivalent check after generation (`xcodebuild test -project ... -scheme CennoiPhone -only-testing:CennoSharedTests/...`) to detect scheme wiring regressions early.

- **NICE-TO-KNOW**
8. [docs/superpowers/plans/2026-06-13-ios-a2ui-renderer.md](/Users/glebkalinin/ai_projects/cenno/docs/superpowers/plans/2026-06-13-ios-a2ui-renderer.md) Task 1 uses `rm -rf` for vendored package cleanup.
  - Fix: if you execute this locally, use your standard `trash` workflow per repository conventions.

Overall verdict: **Not ready to implement as-is.** The plan is mostly directionally correct and complete, but it has blocking compile/API mismatches and a few correctness gaps (custom-widget parity, fallback parity, and non-trivial answer stringify edge behavior) that should be fixed before implementation proceeds.
