# A2UI React renderer spike — findings

Date: 2026-06-09/10 · Packages: `@a2ui/react@0.10.0`, `@a2ui/web_core@0.10.0` · Harness: `spike/a2ui/` (throwaway, not wired into the app)

**Verification method:** every answer below was verified by execution, not from the README. `npx vite build` passes; the harness was run under `vite dev` and driven headlessly (DOM inspection, `getComputedStyle`, synthetic input events, button clicks) via a CDP-controlled Chrome. Hooks exposed on `window.__spike` (see `spike/a2ui/src/App.tsx`).

## Decision: (a) adopt `@a2ui/react` with a custom catalog

Rationale (5 lines):

1. The custom-catalog API is real and small: reuse the protocol-level Zod `ComponentApi`, supply our own React implementation via `createComponentImplementation`, compose with `new Catalog(id, components, functions)` — our Button replacement worked first try.
2. Theming is exactly the token pipeline we want: defaults ship at `:where(:root)` (zero specificity), so plain CSS custom properties on a wrapper element override everything — verified by computed style.
3. Incremental per-component patching by id is first-class (`updateComponents` with one component), with fine-grained signal-based re-render — verified in the DOM.
4. The hard parts we'd otherwise hand-roll — two-way data binding (`{path}`), action context resolution (`context: {draft: {path: '/draft'}}` → `{draft: "hello agent"}` delivered to the host callback), validation `checks`, logic functions — all worked in the harness; a home-grown interpreter would re-implement all of this for no gain.
5. Risks are modest and contained: version field is unvalidated at runtime (we must validate at our MCP boundary anyway), Text needs a `MarkdownContext` provider, and the lib is pre-1.0 — acceptable for a catalog we fully control.

---

## Q1: Can we register our own React component for a catalog type (replace Button)? — YES (verified)

Reuse the schema, swap the implementation, compose a catalog under our own id:

```tsx
// spike/a2ui/src/catalog.tsx
import {Catalog} from '@a2ui/web_core/v0_9';
import {ButtonApi, BASIC_FUNCTIONS} from '@a2ui/web_core/v0_9/basic_catalog';
import {createComponentImplementation, Text, Card, Column, Row, TextField} from '@a2ui/react/v0_9';

export const CennoButton = createComponentImplementation(ButtonApi, ({props, buildChild}) => (
  <button data-testid="cenno-button" onClick={props.action} disabled={props.isValid === false}>
    <span>cenno::</span>
    {props.child ? buildChild(props.child) : null}
  </button>
));

export const cennoCatalog = new Catalog(
  'cenno:catalog/v1',                                   // our own catalog id
  [Text, Card, Column, Row, TextField, CennoButton],    // stock components + our Button
  BASIC_FUNCTIONS,
);
```

The agent payload references the catalog id in `createSurface: {surfaceId, catalogId: 'cenno:catalog/v1'}`. Verified in the browser: the rendered button contains our marker (`cenno::Send`), `props.action` arrives as a ready-to-call function, `buildChild(id)` renders the child component. Defining a wholly new component type (own name + own Zod schema using `CommonSchemas.DynamicString` etc.) uses the identical path, so a cenno-specific catalog is straightforward.

Caveat: `Catalog.components` is a `ReadonlyMap` built in the constructor — you compose a new catalog rather than mutating `basicCatalog`. That's fine (arguably better).

## Q2: Do CSS custom properties on a wrapper element style the components? — YES (verified by computed style)

Basic-catalog components reference `--a2ui-*` vars inline with fallbacks (e.g. `background: var(--a2ui-card-background, var(--a2ui-color-surface, #fff))`), and `injectBasicCatalogStyles()` defines defaults at `:where(:root)` — zero specificity, so any ancestor declaration wins via normal inheritance.

Harness wrapper:

```tsx
<div id="themed-wrapper" style={{
  '--a2ui-color-primary': 'rgb(255, 0, 128)',
  '--a2ui-color-on-primary': 'rgb(0, 255, 0)',
  '--a2ui-border-radius': '13px',
  '--a2ui-color-surface': 'rgb(10, 20, 30)',
} as React.CSSProperties}>
```

Measured with `getComputedStyle` in the running page:

| Element | Property | Computed value |
|---|---|---|
| our Button | background-color | `rgb(255, 0, 128)` |
| our Button | color | `rgb(0, 255, 0)` |
| our Button | border-radius | `13px` |
| stock Card | background-color | `rgb(10, 20, 30)` |

Token vocabulary available: `--a2ui-color-{primary,secondary,surface,background,input,border,...}` (+ `on-*`, `*-hover`, `*-light/dark`), `--a2ui-spacing-{xs..xl}`, `--a2ui-font-size-{xs..2xl}`, `--a2ui-font-family-*`, `--a2ui-border-radius`, `--a2ui-grid-base`, per-component vars like `--a2ui-card-*`. Our token pipeline can map straight onto these on the surface wrapper. Verdict: token pipeline viable.

## Q3: Can the payload be patched incrementally (one component by id)? — YES (verified in DOM)

`updateComponents` with a single-element `components` array updates just that node:

```ts
processor.processMessages([{
  version: 'v0.9',
  updateComponents: {
    surfaceId: 'main',
    components: [{id: 'body', component: 'Text', text: 'PATCHED CLEAN-CYCLE'}],
  },
}]);
```

Verified: the `body` div's text changed to `PATCHED CLEAN-CYCLE`. Crucially for streaming, a separate run typed `"typed before patch"` into the TextField **first**, then patched the sibling `body` component: the body updated, the input kept its value, and the input was the **same DOM node** before/after (`inputAfter === input` → `true`) — fine-grained signal-based re-render (`@preact/signals-core` + Generic Binder), no remount of untouched siblings.

Two semantics to know (from `message-processor.js`, confirmed by behavior):

- **Properties are replaced wholesale, not merged**: `existing.properties = properties`. A patch must carry the full property set *for that component* (but only that component).
- If `component` (type) differs from the existing one, the node is removed and recreated.

Also verified (bonus, relevant to cenno's flow): `updateDataModel` patches by path, the TextField two-way binds into the data model, and clicking Button delivered `{name: 'submit', surfaceId: 'main', sourceComponentId: 'submit', context: {draft: 'hello agent'}}` to the host's action handler — i.e. streaming agent → UI updates and UI → agent actions both work through one processor.

## Q4: Does the renderer pin a spec version, and what breaks on mismatched payloads? — Pinned at the import level, IGNORED at runtime (verified)

- **Package vs protocol:** `@a2ui/react@0.10.0` ships two protocol implementations as separate entry points: `@a2ui/react/v0_8` (legacy `BeginRendering`/`SurfaceUpdate` era) and `@a2ui/react/v0_9` (current). There is **no v0_10 protocol** — package 0.10 is "v0.9 native". You pin the spec by choosing the import path; the two stacks don't interoperate.
- **Runtime:** the v0_9 `MessageProcessor.processMessage` dispatches purely on which key is present (`createSurface` | `updateComponents` | `updateDataModel` | `deleteSurface`) and **never checks the `version` field**. The `z.ZodLiteral<"v0.9">` schemas exist but are TypeScript/validation artifacts, not enforced on `processMessages`.

Executed evidence:

| Payload fed to v0_9 processor | Result |
|---|---|
| v0.8-shaped `{version: 'v0.8', beginRendering: {...}}` | **Silently ignored** — no throw, no render, no error surface |
| v0.9-shaped `{version: 'v0.42', updateComponents: {...}}` | **Silently applied** — DOM showed `PATCHED-BY-WRONG-VERSION` |

So "what breaks" is: nothing loudly. Old-protocol payloads vanish without a trace, and wrong version tags are accepted. For cenno this means **we must validate version + shape at our own boundary** (MCP server / desugaring layer) — which Task 3 (protocol types) was going to do anyway. Minor related wart: `@a2ui/markdown-it@0.0.4` declares a peer-ish dep on `web_core ^0.9.2` while 0.10.0 is installed (`npm ls` flags invalid; works in practice).

## Other findings

- **Text + markdown:** stock `Text` renders raw markdown (`## Hello…` literally, class `no-markdown-renderer`) unless the host wraps the tree in `MarkdownContext.Provider` with a renderer (they ship `@a2ui/markdown-it`). Plan 2 must either provide the provider or use our own Text.
- **Styles:** basic catalog needs `injectBasicCatalogStyles()` (web_core) and uses CSS Modules internally (Vite handles this out of the box). Our own components are free of this.
- **SSR note:** runtime verification was done in a real browser; the renderer uses `useSyncExternalStore` + signals, so client rendering is the supported path (fine for Tauri webview).
- **Bundle cost:** harness production build is 355 kB / 103 kB gzip including React — acceptable for a desktop webview.
- Harness files: `spike/a2ui/src/{App,catalog,main}.tsx`, `spike/a2ui/src/messages.ts`. Reproduce: `cd spike/a2ui && npm i && npx vite` then poke `window.__spike.{patch,feedV08,feedWrongVersion}` in devtools.
