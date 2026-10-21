import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { resolveImageUrl, clearImageCache, ImageLoadError } from "./image";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("resolveImageUrl", () => {
  beforeEach(() => {
    clearImageCache();
    vi.mocked(invoke).mockReset();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
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

  it("rejects SVG data: URIs, which are the one XSS-adjacent form", async () => {
    // SVG can carry script, so the raster allowlist must exclude it.
    const err = await resolveImageUrl("data:image/svg+xml;base64,PHN2Zz4=").catch((e) => e);
    expect(err).toBeInstanceOf(ImageLoadError);
    expect(err.reason).toContain("data:image/");
    expect(invoke).not.toHaveBeenCalled();
  });

  it("rejects remote URLs without ever fetching them", async () => {
    const fetchSpy = vi.fn();
    vi.stubGlobal("fetch", fetchSpy);
    const err = await resolveImageUrl("https://example.com/a.png").catch((e) => e);
    expect(err).toBeInstanceOf(ImageLoadError);
    expect(err.reason).toContain("remote images are not allowed");
    expect(invoke).not.toHaveBeenCalled();
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("rejects plain http too, not only https", async () => {
    // An implementation that only checked https: would pass every other test.
    const err = await resolveImageUrl("http://example.com/a.png").catch((e) => e);
    expect(err).toBeInstanceOf(ImageLoadError);
    expect(err.reason).toContain("remote images are not allowed");
    expect(invoke).not.toHaveBeenCalled();
  });

  it("treats the scheme case-insensitively but never mangles the payload", async () => {
    // RFC 3986: schemes are case-insensitive. Uppercase MIME is still rejected.
    const err = await resolveImageUrl("DATA:IMAGE/PNG;base64,AA").catch((e) => e);
    expect(err).toBeInstanceOf(ImageLoadError);
    expect(err.reason).toContain("data:image/");

    const remote = await resolveImageUrl("HTTPS://example.com/a.png").catch((e) => e);
    expect(remote.reason).toContain("remote images are not allowed");

    const good = "data:image/png;base64,AAA";
    await expect(resolveImageUrl(`  ${good}  `)).resolves.toBe(good);
    expect(invoke).not.toHaveBeenCalled();
  });

  it("routes a local path through load_image with maxBytes omitted from the wire", async () => {
    vi.mocked(invoke).mockResolvedValue("data:image/png;base64,AAAA");
    await expect(resolveImageUrl("/Users/x/a.png")).resolves.toBe(
      "data:image/png;base64,AAAA",
    );
    expect(invoke).toHaveBeenCalledTimes(1);
    const [command, args] = vi.mocked(invoke).mock.calls[0];
    expect(command).toBe("load_image");
    // `toHaveBeenCalledWith` ignores undefined-valued keys, so assert the shape
    // that actually crosses the IPC boundary: JSON.stringify drops an undefined
    // maxBytes and Tauri reads the absent key as `None` for Option<u64>.
    expect(JSON.parse(JSON.stringify(args))).toEqual({ path: "/Users/x/a.png" });
  });

  it("memoises by path so ten tiles do not re-read the same file", async () => {
    vi.mocked(invoke).mockResolvedValue("data:image/png;base64,AAAA");
    await resolveImageUrl("/Users/x/a.png");
    await resolveImageUrl("/Users/x/a.png");
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("collapses concurrent callers onto one read", async () => {
    vi.mocked(invoke).mockResolvedValue("data:image/png;base64,AAAA");
    await Promise.all([
      resolveImageUrl("/Users/x/a.png"),
      resolveImageUrl("/Users/x/a.png"),
      resolveImageUrl("/Users/x/a.png"),
    ]);
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

  it("a stale rejection does not evict a newer entry for the same path", async () => {
    // The cache is cleared mid-render, so an in-flight read can reject AFTER a
    // newer promise for the same path is already cached. Deleting by key alone
    // would drop the newer entry and force a third read.
    let rejectFirst: (e: unknown) => void = () => {};
    vi.mocked(invoke)
      .mockImplementationOnce(
        () => new Promise((_res, rej) => { rejectFirst = rej; }) as Promise<string>,
      )
      .mockResolvedValue("data:image/png;base64,BBBB");

    const first = resolveImageUrl("/Users/x/a.png").catch((e) => e);
    clearImageCache();
    const second = resolveImageUrl("/Users/x/a.png");

    rejectFirst("file not found: /Users/x/a.png");
    await first;
    await expect(second).resolves.toBe("data:image/png;base64,BBBB");

    // The newer entry must still be cached: a third caller reuses it.
    await resolveImageUrl("/Users/x/a.png");
    expect(invoke).toHaveBeenCalledTimes(2);
  });

  it("rejects a non-string url with the named error rather than a TypeError", async () => {
    const err = await resolveImageUrl(42 as unknown as string).catch((e) => e);
    expect(err).toBeInstanceOf(ImageLoadError);
    expect(err.reason).toContain("must be a string");
  });
});
