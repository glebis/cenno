import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
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
