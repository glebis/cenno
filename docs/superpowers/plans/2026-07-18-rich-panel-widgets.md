# Rich Panel Widgets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `cenno:catalog/v1` five new rendering capabilities — stacked radios/checkbox lists, a display-only table, a multi-row rating matrix, hardened images with named failure states, and selection among up to 10 images with an adaptive grid.

**Architecture:** Four new React views in `src/a2ui/views.tsx` plus one behavioural change to the existing `ImageView`, bound into the catalog through cenno-local `createComponentImplementation` adapters in `src/a2ui/catalog.tsx`. Images stop being a bare `<img>`: a new `src/a2ui/image.ts` classifies the URL, and local paths go through a new bounded Rust command `image::load_image`. Every new component gets structural limits enforced in `src-tauri/src/a2ui_guard.rs` so an agent gets a fixable error instead of a clipped panel.

**Tech Stack:** React 19 + TypeScript, Zod schemas, `@a2ui/web_core` 0.10.7, Vitest + Testing Library, Rust (Tauri 2), `serde_json`.

**Spec:** `docs/superpowers/specs/2026-07-18-rich-panel-widgets-design.md`

## Global Constraints

- Catalog id stays `cenno:catalog/v1`. Component names are exactly `Table`, `RatingMatrix`, `ImageChoice`; `ChoicePicker`, `Image`, `Slider` keep their names.
- No network: `https:`/`http:` image URLs are rejected. `img-src 'self' data:` in `src-tauri/tauri.conf.json:26` is unchanged.
- `MAX_COMPONENTS = 200` and `MAX_BYTES = 256 * 1024` in `src-tauri/src/a2ui_guard.rs:23,25` stay as they are.
- Answer contract unchanged: `AskResponse::Answered { answer: String, via, elapsed_s, muted }`. `via` is `"choice"` for every new widget.
- Hard caps: table ≤ 4 columns and ≤ 12 rows; matrix ≤ 3 rows and ≤ 7 rating points; `ImageChoice` ≤ 10 options and ≤ 5 columns.
- Panel height is clamped to 240–560px by `resize_panel`; no new widget may assume more.
- Generated CSS in `src/styles/` is never edited by hand. New styles go in `src/a2ui/catalog.css` (hand-maintained, alongside the existing `.cenno-chip` / `.cenno-image` rules).
- Style values come from the existing tokens: `--cenno-text`, `--cenno-text-dim`, `--cenno-line`, `--cenno-surface`, `--cenno-radius-control`, `--cenno-radius-pill`, caption size 13px with `.08em` uppercase letter-spacing.
- Every failure string is actionable: name the limit and the fix.

## Review Focus

- **A `multipleSelection` `ImageChoice` whose option value contains a comma.** The answer flattens to a comma-joined string, so values `a,b` and `c` produce `"a,b,c"`, indistinguishable from three separate picks. Expected: the guard rejects a comma in any `ImageChoice` option `value` (Task 8).
- **A local path that exists but is a directory, a FIFO, or a `/dev` node.** Expected: `load_image` refuses before reading, with `not an image (extension …)` or `not a regular file`, and the panel shows that reason rather than hanging (Task 1).
- **A syntactically valid `data:image/png;base64,…` whose bytes do not decode.** Expected: `ImageView` still reaches its error state via `onError`, the box keeps its size, and the prompt stays answerable (Task 3).
- **Ten tiles plus a long question plus a caption line in one panel.** Expected: the grid keeps its adaptive column count and `1fr` tracks (so it compresses rather than collapsing to a sliver), and `resize_panel`'s 560px clamp wins while the panel scrolls (Task 8).
- **A table cell containing a long unbroken token (a hash, a URL).** Expected: the cell wraps or ellipsises inside its column; the table never widens the panel or forces horizontal scroll (Task 6).

---

### Task 1: `load_image` Rust command

**Files:**
- Create: `src-tauri/src/image.rs`
- Modify: `src-tauri/src/lib.rs:965` (add `image::load_image` to `tauri::generate_handler!`)
- Modify: `src-tauri/src/lib.rs` (add `mod image;` beside the other `mod` declarations)
- Test: inline `#[cfg(test)] mod tests` in `src-tauri/src/image.rs`

**Interfaces:**
- Produces: `pub fn load_image_bytes(path: &str, max_bytes: u64) -> Result<(String, Vec<u8>), String>` returning `(mime, bytes)`; `pub fn data_uri(path: &str, max_bytes: Option<u64>) -> Result<String, String>` returning `data:<mime>;base64,<…>`; and the Tauri command `#[tauri::command] pub fn load_image(path: String, max_bytes: Option<u64>) -> Result<String, String>`.
- The pure functions take no `AppHandle`, so tests run without a Tauri runtime.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_tmp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("cenno-image-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(bytes).unwrap();
        p
    }

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0];

    #[test]
    fn accepts_a_real_png_and_returns_a_data_uri() {
        let p = write_tmp("ok.png", PNG);
        let uri = data_uri(p.to_str().unwrap(), None).unwrap();
        assert!(uri.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn missing_file_names_the_reason() {
        let err = data_uri("/definitely/not/here.png", None).unwrap_err();
        assert!(err.contains("file not found"), "{err}");
    }

    #[test]
    fn rejects_a_non_image_extension_before_reading() {
        let p = write_tmp("notes.txt", b"hello");
        let err = data_uri(p.to_str().unwrap(), None).unwrap_err();
        assert!(err.contains("not an image"), "{err}");
        assert!(err.contains(".txt"), "{err}");
    }

    #[test]
    fn rejects_a_directory() {
        let dir = std::env::temp_dir().join("cenno-image-tests");
        let err = data_uri(dir.to_str().unwrap(), None).unwrap_err();
        assert!(err.contains("not an image") || err.contains("not a regular file"), "{err}");
    }

    #[test]
    fn rejects_a_relative_path() {
        let err = data_uri("relative/a.png", None).unwrap_err();
        assert!(err.contains("absolute"), "{err}");
    }

    #[test]
    fn rejects_an_oversized_file_naming_both_sizes() {
        let mut big = PNG.to_vec();
        big.resize(2048, 0);
        let p = write_tmp("big.png", &big);
        let err = data_uri(p.to_str().unwrap(), Some(1024)).unwrap_err();
        assert!(err.contains("too large"), "{err}");
        assert!(err.contains("2.0 KB") && err.contains("1.0 KB"), "{err}");
    }

    #[test]
    fn rejects_bytes_that_disagree_with_the_extension() {
        let p = write_tmp("liar.png", b"not a png at all");
        let err = data_uri(p.to_str().unwrap(), None).unwrap_err();
        assert!(err.contains("not a valid PNG"), "{err}");
    }

    #[test]
    fn clamps_the_cap_to_the_documented_range() {
        // 0 becomes 1 KiB, 99 MiB becomes 4096 KiB
        assert_eq!(clamp_cap(None), 512 * 1024);
        assert_eq!(clamp_cap(Some(0)), 1024);
        assert_eq!(clamp_cap(Some(99 * 1024 * 1024)), 4096 * 1024);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test image::`
Expected: compile error `unresolved import` / `cannot find function data_uri`.

- [ ] **Step 3: Implement `src-tauri/src/image.rs`**

Order matters and is the whole design: extension allowlist → size check → read → magic-byte sniff. Signatures exactly as in Interfaces. `clamp_cap(Option<u64>) -> u64` maps `None` to `512 * 1024` and clamps `Some(n)` into `1024..=4096 * 1024`. Mime from extension: `png`→`image/png`, `jpg`/`jpeg`→`image/jpeg`, `gif`→`image/gif`, `webp`→`image/webp`. Magic checks: PNG `\x89PNG\r\n\x1a\n`, JPEG `\xFF\xD8\xFF`, GIF `GIF8`, WEBP `RIFF` at 0 and `WEBP` at 8. Expand a leading `~` with `std::env::var("HOME")` before the absolute-path check. Human-readable sizes in errors use one decimal and the smaller of KB/MB.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test image::`
Expected: 8 passed.

- [ ] **Step 5: Register the command**

Add `mod image;` with the other module declarations and `image::load_image,` inside the `tauri::generate_handler![…]` list at `src-tauri/src/lib.rs:965`.

- [ ] **Step 6: Verify the app still builds**

Run: `cd src-tauri && cargo check`
Expected: no errors, no new warnings about unused `image` items.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/image.rs src-tauri/src/lib.rs
git commit -m "feat(image): bounded local-image loader returning a data: URI"
```

---

### Task 2: `resolveImageUrl` frontend helper

**Files:**
- Create: `src/a2ui/image.ts`
- Test: `src/a2ui/image.test.ts`

**Interfaces:**
- Consumes: `invoke` from `@tauri-apps/api/core`; the Rust command `load_image(path, max_bytes)`.
- Produces: `export class ImageLoadError extends Error { reason: string }`; `export async function resolveImageUrl(url: string): Promise<string>` returning a usable `src`; `export function clearImageCache(): void`.

- [ ] **Step 1: Write the failing tests**

```ts
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { clearImageCache } from "./image";
import { resolveImageUrl, clearImageCache, ImageLoadError } from "./image";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("resolveImageUrl", () => {
  beforeEach(() => {
    clearImageCache();
    vi.mocked(invoke).mockReset();
  });

  it("passes a well-formed data: URI straight through without invoking", async () => {
    const uri = "data:image/png;base64,iVBORw0KGgo=";
    await expect(resolveImageUrl(uri)).resolves.toBe(uri);
    expect(invoke).not.toHaveBeenCalled();
  });

  it("rejects a malformed data: URI with a named reason", async () => {
    await expect(resolveImageUrl("data:text/html;base64,AAAA")).rejects.toBeInstanceOf(
      ImageLoadError,
    );
    expect(invoke).not.toHaveBeenCalled();
  });

  it("rejects remote URLs without ever fetching them", async () => {
    const err = await resolveImageUrl("https://example.com/a.png").catch((e) => e);
    expect(err).toBeInstanceOf(ImageLoadError);
    expect(err.reason).toContain("remote images are not allowed");
    expect(invoke).not.toHaveBeenCalled();
  });

  it("routes a local path through load_image", async () => {
    vi.mocked(invoke).mockResolvedValue("data:image/png;base64,AAAA");
    await expect(resolveImageUrl("/Users/x/a.png")).resolves.toBe(
      "data:image/png;base64,AAAA",
    );
    expect(invoke).toHaveBeenCalledWith("load_image", {
      path: "/Users/x/a.png",
      maxBytes: undefined,
    });
  });

  it("memoises by path so ten tiles do not re-read the same file", async () => {
    vi.mocked(invoke).mockResolvedValue("data:image/png;base64,AAAA");
    await resolveImageUrl("/Users/x/a.png");
    await resolveImageUrl("/Users/x/a.png");
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("surfaces the Rust error string as the reason", async () => {
    vi.mocked(invoke).mockRejectedValue("file not found: /Users/x/a.png");
    const err = await resolveImageUrl("/Users/x/a.png").catch((e) => e);
    expect(err.reason).toContain("file not found");
  });

  it("clearImageCache forces a fresh read", async () => {
    vi.mocked(invoke).mockResolvedValue("data:image/png;base64,AAAA");
    await resolveImageUrl("/Users/x/a.png");
    clearImageCache();
    await resolveImageUrl("/Users/x/a.png");
    expect(invoke).toHaveBeenCalledTimes(2);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npx vitest run src/a2ui/image.test.ts`
Expected: FAIL — cannot resolve `./image`.

- [ ] **Step 3: Implement `src/a2ui/image.ts`**

Classify by prefix: `data:image/(png|jpeg|gif|webp);base64,` → return as-is; any other `data:` → `ImageLoadError("malformed data: URI — expected data:image/<png|jpeg|gif|webp>;base64,…")`; `https:`/`http:` → `ImageLoadError("remote images are not allowed — embed it as a data: URI or pass a local file path")`; anything else → `invoke("load_image", { path: url, maxBytes: undefined })`. Cache hits and in-flight promises share one `Map<string, Promise<string>>` so concurrent tiles of the same path collapse. A rejected promise is evicted from the map so a retry after a file appears can succeed.

- [ ] **Step 4: Run tests to verify they pass**

Run: `npx vitest run src/a2ui/image.test.ts`
Expected: 7 passed.

- [ ] **Step 5: Commit**

```bash
git add src/a2ui/image.ts src/a2ui/image.test.ts
git commit -m "feat(a2ui): classify image URLs, route local paths through load_image"
```

---

### Task 3: Harden `ImageView`

**Files:**
- Modify: `src/a2ui/views.tsx:407-435` (`ImageView`)
- Modify: `src/a2ui/catalog.css` (append image state rules near line 364)
- Test: `src/a2ui/views.test.tsx` (extend the existing `ImageView` describe block)

**Interfaces:**
- Consumes: `resolveImageUrl`, `ImageLoadError` from `./image`.
- Produces: `ImageView` keeps its current props (`url`, `description`, `fit`, `variant`) so the catalog adapter needs no change.

- [ ] **Step 1: Write the failing tests**

```tsx
describe("ImageView states", () => {
  beforeEach(() => clearImageCache());

  it("shows a shimmering placeholder while a local path resolves", async () => {
    vi.mocked(invoke).mockReturnValue(new Promise(() => {})); // never settles
    render(<ImageView url="/Users/x/a.png" />);
    expect(screen.getByTestId("image-loading")).toBeTruthy();
  });

  it("shows the image once a data: URI resolves", async () => {
    render(<ImageView url="data:image/png;base64,iVBORw0KGgo=" description="logo" />);
    const img = await screen.findByRole("img", { name: "logo" });
    expect(img.getAttribute("src")).toBe("data:image/png;base64,iVBORw0KGgo=");
  });

  it("names the reason when the path fails to load", async () => {
    vi.mocked(invoke).mockRejectedValue("file not found: /Users/x/a.png");
    render(<ImageView url="/Users/x/a.png" />);
    expect(await screen.findByText(/image failed to load — file not found/)).toBeTruthy();
  });

  it("rejects a remote URL in the panel instead of rendering blank", async () => {
    render(<ImageView url="https://example.com/a.png" />);
    expect(await screen.findByText(/remote images are not allowed/)).toBeTruthy();
  });

  it("falls back to the error state when a valid data: URI fails to decode", async () => {
    const { container } = render(<ImageView url="data:image/png;base64,AAAA" />);
    const img = await waitFor(() => {
      const el = container.querySelector("img");
      expect(el).toBeTruthy();
      return el!;
    });
    fireEvent.error(img);
    expect(await screen.findByText(/failed to decode/)).toBeTruthy();
  });

  it("keeps a fixed box so the panel does not jump between states", async () => {
    const { container } = render(<ImageView url="/Users/x/a.png" variant="smallFeature" />);
    expect(container.querySelector(".cenno-image-box--smallFeature")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npx vitest run src/a2ui/views.test.tsx -t "ImageView states"`
Expected: FAIL — no `image-loading` test id, no error text.

- [ ] **Step 3: Implement the three-state `ImageView`**

Keep the outer box a `div.cenno-image-box.cenno-image-box--<variant>` that always occupies the same size as the current `.cenno-image--<variant>` rules. `useState<"loading"|"ok"|"error">` plus a `reason` string; `useEffect` keyed on `url` calls `resolveImageUrl`, setting `ok`/`error` and ignoring results after unmount. Render `data-testid="image-loading"` shimmer, `<img>` with `onError` → `error("failed to decode")`, or the error line. Copy for the error line: `image failed to load — <reason>`, caption size, `--cenno-text-dim`. Shimmer uses the same gradient keyframe pattern already used elsewhere; wrap it in `@media (prefers-reduced-motion: reduce)` to a static tint.

- [ ] **Step 4: Run tests to verify they pass**

Run: `npx vitest run src/a2ui/views.test.tsx`
Expected: all pass, including the pre-existing `ImageView` test.

- [ ] **Step 5: Commit**

```bash
git add src/a2ui/views.tsx src/a2ui/catalog.css src/a2ui/views.test.tsx
git commit -m "feat(a2ui): image loading, ok, and named-error states"
```

---

### Task 4: Guard rules for `Image`

**Files:**
- Modify: `src-tauri/src/a2ui_guard.rs` (new check inside `validate_a2ui` after the `updateComponents` walk; new tests in `mod tests`)
- Test: `src-tauri/src/a2ui_guard.rs` inline tests

**Interfaces:**
- Produces: `fn check_component(component: &Value) -> Result<(), String>` called for every entry of every `updateComponents.components` array. Task 6, 7 and 8 each add one arm to this function.

- [ ] **Step 1: Write the failing tests**

```rust
fn envelope_with(component: Value) -> Value {
    json!([
        create_surface(),
        { "version": "v0.9", "updateComponents": { "surfaceId": "main",
            "components": [ { "id": "root", "component": "Column", "children": ["c"] }, component ] } }
    ])
}

#[test]
fn https_image_rejects_with_the_fix() {
    let err = validate_a2ui(&envelope_with(json!({
        "id": "c", "component": "Image", "url": "https://example.com/a.png" })))
        .unwrap_err();
    assert!(err.contains("remote images are not allowed"), "{err}");
    assert!(err.contains("data: URI"), "{err}");
}

#[test]
fn malformed_data_uri_rejects() {
    let err = validate_a2ui(&envelope_with(json!({
        "id": "c", "component": "Image", "url": "data:text/html;base64,AAAA" })))
        .unwrap_err();
    assert!(err.contains("data:image/"), "{err}");
}

#[test]
fn local_path_and_good_data_uri_pass() {
    for url in [
        "/Users/x/a.png",
        "data:image/jpeg;base64,AAAA",
        "data:image/webp;base64,AAAA",
    ] {
        validate_a2ui(&envelope_with(json!({
            "id": "c", "component": "Image", "url": url }))).unwrap();
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test a2ui_guard::tests::https_image`
Expected: FAIL — `unwrap_err()` panics because the payload is accepted.

- [ ] **Step 3: Implement `check_component`**

Walk every component in each `updateComponents.components` array. Match on the `component` string; unknown names return `Ok(())` (the renderer's problem, as today). The `Image` arm reads `url`: a non-string is `Image.url must be a string`; a `data:` prefix must match `^data:image/(png|jpeg|gif|webp);base64,`; an `http`/`https` prefix is the remote-image error; anything else (path) passes.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test a2ui_guard::`
Expected: all pass, existing envelope tests included.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/a2ui_guard.rs
git commit -m "feat(guard): reject remote and malformed image URLs"
```

---

### Task 5: `CheckListView` behind `ChoicePicker`

**Files:**
- Modify: `src/a2ui/views.tsx` (new `CheckListView` beside `ChipsView` at line 119)
- Modify: `src/a2ui/catalog.tsx:246-275` (`CennoChoicePicker` picks a view from `displayStyle`)
- Modify: `src/a2ui/catalog.css` (list styles after the `.cenno-chips` block at line 101)
- Test: `src/a2ui/views.test.tsx`, `src/a2ui/catalog.test.tsx` if a catalog-level assertion fits

**Interfaces:**
- Produces: `export function CheckListView({ choices, selected, variant, onSelect }: { choices: { label: string; value: string }[]; selected?: string[]; variant?: "single" | "multiple"; onSelect: (value: string) => void })`.
- The catalog adapter calls it when `props.displayStyle === "checkbox"`; `undefined` and `"chips"` keep `ChipsView` so no shipped payload restyles.

- [ ] **Step 1: Write the failing tests**

```tsx
describe("CheckListView", () => {
  const choices = [
    { label: "Deep work", value: "deep" },
    { label: "Meetings", value: "meet" },
  ];

  it("renders radios with aria-checked for a single-select list", () => {
    render(<CheckListView choices={choices} selected={["meet"]} variant="single" onSelect={vi.fn()} />);
    const radios = screen.getAllByRole("radio");
    expect(radios).toHaveLength(2);
    expect(radios[1].getAttribute("aria-checked")).toBe("true");
    expect(radios[0].getAttribute("aria-checked")).toBe("false");
  });

  it("reports the value, not the label, on click", () => {
    const onSelect = vi.fn();
    render(<CheckListView choices={choices} variant="single" onSelect={onSelect} />);
    fireEvent.click(screen.getByRole("radio", { name: "Deep work" }));
    expect(onSelect).toHaveBeenCalledWith("deep");
  });

  it("renders checkboxes and toggles independently in multi-select", () => {
    const onSelect = vi.fn();
    render(<CheckListView choices={choices} selected={["deep"]} variant="multiple" onSelect={onSelect} />);
    expect(screen.getAllByRole("checkbox")[0].getAttribute("aria-checked")).toBe("true");
    fireEvent.click(screen.getByRole("checkbox", { name: "Meetings" }));
    expect(onSelect).toHaveBeenCalledWith("meet");
  });

  it("moves selection with the arrow keys in a single-select list", () => {
    const onSelect = vi.fn();
    render(<CheckListView choices={choices} variant="single" onSelect={onSelect} />);
    fireEvent.keyDown(screen.getByRole("radio", { name: "Deep work" }), { key: "ArrowDown" });
    expect(onSelect).toHaveBeenCalledWith("meet");
  });

  it("selects the focused row on Space and Enter", () => {
    const onSelect = vi.fn();
    render(<CheckListView choices={choices} variant="single" onSelect={onSelect} />);
    const first = screen.getByRole("radio", { name: "Deep work" });
    fireEvent.keyDown(first, { key: " " });
    fireEvent.keyDown(first, { key: "Enter" });
    expect(onSelect).toHaveBeenCalledTimes(2);
  });

  it("keeps a 44px tap target per row", () => {
    const { container } = render(<CheckListView choices={choices} variant="single" onSelect={vi.fn()} />);
    expect(container.querySelectorAll(".cenno-checklist__row")).toHaveLength(2);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npx vitest run src/a2ui/views.test.tsx -t "CheckListView"`
Expected: FAIL — `CheckListView` is not exported.

- [ ] **Step 3: Implement `CheckListView` and wire the adapter**

One `div[role="radiogroup"]` (single) or `div[role="group"]` (multiple) containing `button[role="radio"|"checkbox"][aria-checked]` rows, each with a drawn mark (circle or square, 18px, `1px var(--cenno-line)`, filled with `--cenno-text` when checked) and a label. Handle `Space`/`Enter` on the row and `ArrowUp`/`ArrowDown` to move focus and, in single-select, select the neighbour. Rows are `display:flex; align-items:center; gap:12px; min-height:44px`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `npx vitest run src/a2ui/views.test.tsx`
Expected: all pass.

- [ ] **Step 5: Verify no existing payload changed appearance**

Run: `npx vitest run src/a2ui` — the existing `ChipsView` tests must still pass untouched, proving `displayStyle` omission keeps chips.

- [ ] **Step 6: Commit**

```bash
git add src/a2ui/views.tsx src/a2ui/catalog.tsx src/a2ui/catalog.css src/a2ui/views.test.tsx
git commit -m "feat(a2ui): honour displayStyle with a radio/checkbox list view"
```

---

### Task 6: `TableView`

**Files:**
- Modify: `src/a2ui/views.tsx` (new `TableView`)
- Modify: `src/a2ui/catalog.tsx` (new `TableApi` + `CennoTable`, added to the `cennoCatalog` array at line 444)
- Modify: `src/a2ui/catalog.css` (table styles appended)
- Modify: `src-tauri/src/a2ui_guard.rs` (Table arm in `check_component`)
- Test: `src/a2ui/views.test.tsx`, `src-tauri/src/a2ui_guard.rs`

**Interfaces:**
- Produces: `export function TableView({ header, columns, rows, dense, caption }: { header?: string; columns: { label: string; align?: "start" | "end" }[]; rows: string[][]; dense?: boolean; caption?: string })`; `export const TableApi` with `name: "Table"`.

- [ ] **Step 1: Write the failing tests**

```tsx
describe("TableView", () => {
  const columns = [
    { label: "Plan", align: "start" as const },
    { label: "Cost", align: "end" as const },
  ];
  const rows = [["free", "$0"], ["pro", "$20"]];

  it("renders a real table with a header row and one row per record", () => {
    render(<TableView columns={columns} rows={rows} caption="Plans compared" />);
    expect(screen.getByRole("table", { name: "Plans compared" })).toBeTruthy();
    expect(screen.getAllByRole("columnheader")).toHaveLength(2);
    expect(screen.getAllByRole("row")).toHaveLength(3); // header + 2
  });

  it("applies per-column alignment so numbers line up", () => {
    render(<TableView columns={columns} rows={rows} />);
    expect(screen.getByRole("columnheader", { name: "Cost" }).className).toContain("is-end");
  });

  it("renders an em dash for a missing cell instead of a hole", () => {
    render(<TableView columns={columns} rows={[["free"]]} />);
    expect(screen.getByText("—")).toBeTruthy();
  });

  it("wraps a long unbroken token instead of widening the panel", () => {
    const long = "a".repeat(80);
    const { container } = render(
      <TableView columns={[{ label: "Hash" }]} rows={[[long]]} />,
    );
    expect(container.querySelector(".cenno-table")?.className).toContain("cenno-table");
    expect(container.querySelector(".cenno-table__cell")?.getAttribute("style"))
      .toContain("break-word");
  });
});
```

Guard tests in `src-tauri/src/a2ui_guard.rs`:

```rust
#[test]
fn table_with_five_columns_rejects_naming_the_cap() {
    let cols: Vec<Value> = (0..5).map(|i| json!({ "label": format!("c{i}") })).collect();
    let err = validate_a2ui(&envelope_with(json!({
        "id": "c", "component": "Table", "columns": cols, "rows": [] }))).unwrap_err();
    assert!(err.contains("5 columns") && err.contains("maximum is 4"), "{err}");
}

#[test]
fn table_row_longer_than_the_header_rejects() {
    let err = validate_a2ui(&envelope_with(json!({
        "id": "c", "component": "Table",
        "columns": [{ "label": "a" }, { "label": "b" }],
        "rows": [["1", "2", "3"]] }))).unwrap_err();
    assert!(err.contains("row 0") && err.contains("3 cells"), "{err}");
}

#[test]
fn table_over_twelve_rows_rejects() {
    let rows: Vec<Value> = (0..13).map(|i| json!([format!("{i}")])).collect();
    let err = validate_a2ui(&envelope_with(json!({
        "id": "c", "component": "Table",
        "columns": [{ "label": "n" }], "rows": rows }))).unwrap_err();
    assert!(err.contains("maximum is 12"), "{err}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npx vitest run src/a2ui/views.test.tsx -t "TableView"` and `cd src-tauri && cargo test a2ui_guard::tests::table_`
Expected: FAIL on both.

- [ ] **Step 3: Implement `TableView`, `TableApi`, and the guard arm**

`TableView` renders a `<table className="cenno-table">` with an optional `<caption>` (caption size, `--cenno-text-dim`), `<thead>` from `columns` (uppercase, letter-spacing `.08em`, caption size, hairline bottom border, `is-end` class for `align: "end"`), and one `<tr>` per row. Cells `text-align` follows the column; `overflow-wrap: anywhere` keeps a long token inside its column. Missing cells render `—`. Rows are separated by hairlines in `--cenno-line` at low opacity; no zebra striping, no outer border.

`TableApi` schema: `header` (DynamicString, optional), `columns` (array of `{label: DynamicString, align?: "start"|"end"}`, `.min(1)`), `rows` (array of array of DynamicString), `dense` (boolean, optional), `caption` (DynamicString, optional), `.strict()` with `CommonProps` spread as in the sibling APIs. The guard arm checks: `columns.len() in 1..=4`, `rows.len() <= 12`, every row's length `== columns.len()`, `align ∈ {start,end}` when present.

- [ ] **Step 4: Run tests to verify they pass**

Run: `npx vitest run src/a2ui` and `cd src-tauri && cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/a2ui/views.tsx src/a2ui/catalog.tsx src/a2ui/catalog.css src/a2ui/views.test.tsx src-tauri/src/a2ui_guard.rs
git commit -m "feat(a2ui): display-only Table with 4-column, 12-row caps"
```

---

### Task 7: `RatingMatrix`

**Files:**
- Modify: `src/a2ui/views.tsx` (new `MatrixView`)
- Modify: `src/a2ui/catalog.tsx` (new `RatingMatrixApi` + `CennoRatingMatrix`, added to the catalog array)
- Modify: `src/a2ui/catalog.css` (matrix styles appended)
- Modify: `src-tauri/src/a2ui_guard.rs` (RatingMatrix arm)
- Test: `src/a2ui/views.test.tsx`, `src-tauri/src/a2ui_guard.rs`

**Interfaces:**
- Consumes: `SliderView` (existing, `src/a2ui/views.tsx:287`) for `mode: "sliders"`.
- Produces: `export function MatrixView({ rows, mode, min, max, minLabel, maxLabel, value, onSelect }: { rows: { value: string; label: string }[]; mode: "matrix" | "sliders"; min: number; max: number; minLabel?: string; maxLabel?: string; value?: Record<string, number>; onSelect: (key: string, n: number) => void })`.

- [ ] **Step 1: Write the failing tests**

```tsx
describe("MatrixView", () => {
  const rows = [
    { value: "pace", label: "Pace" },
    { value: "useful", label: "Usefulness" },
  ];

  it("renders one row of numeral targets per item in matrix mode", () => {
    render(<MatrixView rows={rows} mode="matrix" min={1} max={3} onSelect={vi.fn()} />);
    expect(screen.getAllByRole("radiogroup")).toHaveLength(2);
    expect(screen.getAllByRole("radio")).toHaveLength(6);
  });

  it("reports the row value with the chosen number", () => {
    const onSelect = vi.fn();
    render(<MatrixView rows={rows} mode="matrix" min={1} max={3} onSelect={onSelect} />);
    fireEvent.click(screen.getAllByRole("radio", { name: "3" })[1]);
    expect(onSelect).toHaveBeenCalledWith("useful", 3);
  });

  it("marks the bound values as checked", () => {
    render(<MatrixView rows={rows} mode="matrix" min={1} max={3}
      value={{ pace: 2 }} onSelect={vi.fn()} />);
    expect(screen.getAllByRole("radio")[1].getAttribute("aria-checked")).toBe("true");
  });

  it("renders a labelled slider per item in sliders mode", () => {
    const onSelect = vi.fn();
    render(<MatrixView rows={rows} mode="sliders" min={0} max={10}
      minLabel="low" maxLabel="high" onSelect={onSelect} />);
    const sliders = screen.getAllByRole("slider");
    expect(sliders).toHaveLength(2);
    fireEvent.change(sliders[0], { target: { value: "7" } });
    expect(onSelect).toHaveBeenCalledWith("pace", 7);
  });

  it("shows end labels once, shared by every row", () => {
    render(<MatrixView rows={rows} mode="sliders" min={0} max={10}
      minLabel="not at all" maxLabel="completely" onSelect={vi.fn()} />);
    expect(screen.getAllByText("not at all")).toHaveLength(1);
    expect(screen.getAllByText("completely")).toHaveLength(1);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npx vitest run src/a2ui/views.test.tsx -t "MatrixView"`
Expected: FAIL — not exported.

- [ ] **Step 3: Implement `MatrixView`**

Shared header row of numerals only in `matrix` mode when `min`/`max` fit (each row still gets its own `role="radiogroup"` named by the row label, with `aria-label` on each numeral so a screen reader says "Usefulness, 3"). `sliders` mode reuses `SliderView` per row with the row label as its accessible name, and renders the shared end labels once below the stack. Row label column is 96px, right-aligned in `matrix` mode so the numerals line up. Every row reports `(value, n)` immediately; the adapter calls `props.setValue({ ...value, [key]: n })` and `props.selectAction?.()`.

- [ ] **Step 4: Implement `RatingMatrixApi` and the guard arm**

Schema: `rows` (array of `{value: string, label?: DynamicString}`, `.min(1)`), `mode` (`"matrix" | "sliders"`), `min`/`max` (numbers), `minLabel`/`maxLabel` (DynamicString optional), `value` (DynamicObject/DynamicNumber record as used by the data model), `selectAction`, `.strict()`. Guard: `rows.len() in 1..=3`, `max - min + 1 <= 7`, `mode ∈ {matrix, sliders}`, every row has a non-empty `value`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `npx vitest run src/a2ui` and `cd src-tauri && cargo test`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/a2ui/views.tsx src/a2ui/catalog.tsx src/a2ui/catalog.css src/a2ui/views.test.tsx src-tauri/src/a2ui_guard.rs
git commit -m "feat(a2ui): RatingMatrix for evaluating several items at once"
```

---

### Task 8: `ImageChoice`

**Files:**
- Modify: `src/a2ui/views.tsx` (new `ImageChoiceView`)
- Modify: `src/a2ui/catalog.tsx` (new `ImageChoiceApi` + `CennoImageChoice`, added to the catalog array)
- Modify: `src/a2ui/catalog.css` (grid, tile, mark, and failure styles appended)
- Modify: `src-tauri/src/a2ui_guard.rs` (ImageChoice arm, including the comma check)
- Test: `src/a2ui/views.test.tsx`, `src-tauri/src/a2ui_guard.rs`

**Interfaces:**
- Consumes: `ImageView`'s resolve path indirectly — tiles call `resolveImageUrl` themselves so a failed tile can render its own compact reason without the variant box.
- Produces: `export function ImageChoiceView({ options, selected, variant, fit, columns, onSelect }: { options: { value: string; label?: string; image: string }[]; selected?: string[]; variant?: "mutuallyExclusive" | "multipleSelection"; fit?: "cover" | "contain"; columns?: number; onSelect: (value: string) => void })`; `export const imageChoiceColumns = (n: number): number`.

- [ ] **Step 1: Write the failing tests**

```tsx
describe("imageChoiceColumns", () => {
  it("maps every count 1..10 to the documented column count", () => {
    const expected = [1, 2, 3, 2, 3, 3, 4, 4, 3, 5];
    for (let n = 1; n <= 10; n++) {
      expect(imageChoiceColumns(n)).toBe(expected[n - 1]);
    }
  });

  it("never returns a column count that orphans a single tile", () => {
    for (let n = 1; n <= 10; n++) {
      const cols = imageChoiceColumns(n);
      const lastRow = n % cols;
      expect(lastRow).not.toBe(1);
    }
  });
});

describe("ImageChoiceView", () => {
  const opts = [
    { value: "v1", label: "v1 · bold", image: "data:image/png;base64,AAAA" },
    { value: "v2", label: "v2 · quiet", image: "data:image/png;base64,BBBB" },
  ];

  it("renders one radio per option with the label under the tile", () => {
    render(<ImageChoiceView options={opts} variant="mutuallyExclusive" onSelect={vi.fn()} />);
    expect(screen.getAllByRole("radio")).toHaveLength(2);
    expect(screen.getByRole("radio", { name: /v1 · bold/ })).toBeTruthy();
  });

  it("reports the option value on click", () => {
    const onSelect = vi.fn();
    render(<ImageChoiceView options={opts} onSelect={onSelect} />);
    fireEvent.click(screen.getByRole("radio", { name: /v2/ }));
    expect(onSelect).toHaveBeenCalledWith("v2");
  });

  it("uses a square mark for multipleSelection", () => {
    const { container } = render(
      <ImageChoiceView options={opts} variant="multipleSelection" onSelect={vi.fn()} />,
    );
    expect(container.querySelectorAll(".cenno-ichoice__mark--square")).toHaveLength(2);
  });

  it("keeps a failed tile in the grid and selectable", async () => {
    vi.mocked(invoke).mockRejectedValue("file not found: /x/a.png");
    const onSelect = vi.fn();
    render(
      <ImageChoiceView
        options={[{ value: "broken", label: "broken", image: "/x/a.png" }]}
        onSelect={onSelect}
      />,
    );
    expect(await screen.findByText(/failed to load — file not found/)).toBeTruthy();
    fireEvent.click(screen.getByRole("radio", { name: /broken/ }));
    expect(onSelect).toHaveBeenCalledWith("broken");
  });

  it("applies the adaptive column count and 1fr tracks so tiles compress instead of collapsing", () => {
    const opts10 = Array.from({ length: 10 }, (_, i) => ({
      value: `v${i}`, image: "data:image/png;base64,AAAA",
    }));
    const { container } = render(<ImageChoiceView options={opts10} onSelect={vi.fn()} />);
    const grid = container.querySelector<HTMLElement>(".cenno-ichoice")!;
    expect(grid.style.getPropertyValue("--cenno-ichoice-cols")).toBe("5");
    expect(getComputedStyle(grid).gridTemplateColumns).toMatch(/1fr/);
  });
});
```

Guard tests:

```rust
#[test]
fn image_choice_over_ten_options_rejects_with_two_fixes() {
    let opts: Vec<Value> = (0..11)
        .map(|i| json!({ "value": format!("v{i}"), "image": "data:image/png;base64,AAAA" }))
        .collect();
    let err = validate_a2ui(&envelope_with(json!({
        "id": "c", "component": "ImageChoice", "options": opts }))).unwrap_err();
    assert!(err.contains("11 options") && err.contains("maximum is 10"), "{err}");
    assert!(err.contains("ask_sequence"), "{err}");
}

#[test]
fn image_choice_value_with_a_comma_rejects() {
    let err = validate_a2ui(&envelope_with(json!({
        "id": "c", "component": "ImageChoice", "variant": "multipleSelection",
        "options": [{ "value": "a,b", "image": "data:image/png;base64,AAAA" }] }))).unwrap_err();
    assert!(err.contains("comma"), "{err}");
    assert!(err.contains("unambiguous"), "{err}");
}

#[test]
fn image_choice_rejects_a_remote_option_image() {
    let err = validate_a2ui(&envelope_with(json!({
        "id": "c", "component": "ImageChoice",
        "options": [{ "value": "v1", "image": "https://example.com/a.png" }] }))).unwrap_err();
    assert!(err.contains("remote images are not allowed"), "{err}");
}

#[test]
fn image_choice_unknown_variant_rejects() {
    let err = validate_a2ui(&envelope_with(json!({
        "id": "c", "component": "ImageChoice", "variant": "radio",
        "options": [{ "value": "v1", "image": "data:image/png;base64,AAAA" }] }))).unwrap_err();
    assert!(err.contains("mutuallyExclusive"), "{err}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npx vitest run src/a2ui/views.test.tsx -t "ImageChoice"` and `cd src-tauri && cargo test a2ui_guard::tests::image_choice`
Expected: FAIL on both.

- [ ] **Step 3: Implement `imageChoiceColumns` and `ImageChoiceView`**

`imageChoiceColumns` is a lookup on the table in the spec: `[1,2,3,2,3,3,4,4,3,5]` indexed by `n - 1`, clamped to `1..=10`. The view sets `--cenno-ichoice-cols` on `.cenno-ichoice` and CSS uses `grid-template-columns: repeat(var(--cenno-ichoice-cols), 1fr)` — the override prop wins when present. Each tile is a `button[role="radio"|"checkbox"][aria-checked]` containing a square box (`aspect-ratio: 1/1`, `1px transparent` border, `2px var(--cenno-text)` when selected so nothing shifts), a `resolveImageUrl`-backed image with the same three states as `ImageView` but a compact error line (caption size, `--cenno-text-dim`, dashed `--cenno-line` border), a top-right mark (round or square), and the optional label underneath in caption size. Tiles keep `cover` unless `fit === "contain"`.

- [ ] **Step 4: Implement `ImageChoiceApi` and the guard arm**

Schema: `options` (array of `{value: string, label?: DynamicString, image: DynamicString}`, `.min(1).max(10)`), `variant` (`"mutuallyExclusive" | "multipleSelection"`, optional), `fit` (`"cover" | "contain"`, optional), `columns` (number 1–5, optional), `selectAction`, `.strict()`. The adapter mirrors `CennoChoicePicker`'s selection logic exactly, so `multipleSelection` answers flatten through `src/PromptPanel.tsx:151` unchanged. Guard: option count 1–10, every option has a non-empty `value` and an `image` passing the `Image` URL rules, no `value` contains `,`, `variant` and `fit` in their enums when present, `columns` in 1–5 when present.

- [ ] **Step 5: Run tests to verify they pass**

Run: `npx vitest run src/a2ui` and `cd src-tauri && cargo test`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/a2ui/views.tsx src/a2ui/catalog.tsx src/a2ui/catalog.css src/a2ui/views.test.tsx src-tauri/src/a2ui_guard.rs
git commit -m "feat(a2ui): ImageChoice — up to 10 images, adaptive grid, no orphan rows"
```

---

### Task 9: Document the widgets for agents and for QA

**Files:**
- Modify: `skills/cenno/SKILL.md:305-460` (widget advisor table + new payload examples)
- Modify: `docs/design/CONTROLS.md` (record the new presentations)
- Modify: `scripts/demo.sh` (one demo per new widget)

**Interfaces:**
- Consumes: the final schemas from Tasks 5–8 — every example in the SKILL must be copy-paste valid against the guard.

- [ ] **Step 1: Add the advisor rows**

Extend the table at `skills/cenno/SKILL.md:308` with: several items each needing a rating → `a2ui` **RatingMatrix** (`mode:"matrix"`); several items each on a continuum → **RatingMatrix** (`mode:"sliders"`); comparing structured records → **Table** (≤4 columns, ≤12 rows); picking one of up to 10 visual artifacts → **ImageChoice**; a stacked list of mutually exclusive options → **ChoicePicker** with `displayStyle:"checkbox"`.

- [ ] **Step 2: Add one copy-paste payload per new component**

Each example is a full three-message A2UI array with `createSurface`, `updateComponents`, `updateDataModel`, matching the style of the existing examples at `skills/cenno/SKILL.md:451+`. Include the caps in prose beside each: table 4×12, matrix 3 rows × 7 points, ImageChoice 10 options / ~25 KB per tile, no commas in option values.

- [ ] **Step 3: Correct the image paragraph**

Replace the "prefer data: URIs" note at `skills/cenno/SKILL.md:328` with the actual rule: `data:` URIs and local paths both work; local paths are read through `load_image` and are capped at 512 KiB; remote URLs are rejected with a named error; a failed image keeps its tile and stays selectable.

- [ ] **Step 4: Add demo prompts**

In `scripts/demo.sh`, add `table`, `matrix`, `sliders`, `images`, and `image-error` cases with a literal `a2ui` payload each, and add them to the `all` loop.

- [ ] **Step 5: Verify the demos render**

Run: `./scripts/demo.sh table` (and the other four) against a running app; confirm each panel renders, the answer returns, and the image-error panel shows its named reason.

- [ ] **Step 6: Commit**

```bash
git add skills/cenno/SKILL.md docs/design/CONTROLS.md scripts/demo.sh
git commit -m "docs(skill): document table, matrix, image choice, and image rules"
```

---

## Verification gate (run before declaring the plan complete)

```bash
cd src-tauri && cargo test && cd ..
npx vitest run
npm run typecheck:tests
PATH="/usr/bin:$PATH" npx tauri build --no-bundle
```

The bundled build matters: CSP is only enforced there, so the image path (data: URIs and `load_image`) is only truly tested by the release binary.
