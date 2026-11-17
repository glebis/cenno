import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  loadSecretFormats,
  resetSecretFormatsForTests,
  verdictFor,
  type SecretFormatInfo,
} from "./secretFormat";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const FORMATS: SecretFormatInfo[] = [
  {
    id: "openai_api_key",
    label: "OpenAI API key",
    pattern: "^sk-(proj-|svcacct-|admin-)?[A-Za-z0-9_-]{20,}$",
    hint: "OpenAI keys start sk-",
  },
  {
    id: "github_pat",
    label: "GitHub token",
    pattern: "^gh[pousr]_[A-Za-z0-9]{36,}$",
    hint: "GitHub tokens start gh p/o/u/s/r _",
  },
];

function catalogue(rows: SecretFormatInfo[]): Map<string, SecretFormatInfo> {
  return new Map(rows.map((f) => [f.id, f]));
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  resetSecretFormatsForTests();
});

describe("verdictFor", () => {
  const formats = catalogue(FORMATS);

  it("says nothing when the field is empty", () => {
    expect(verdictFor(formats, "openai_api_key", "")).toEqual({
      state: "empty",
      message: "",
    });
  });

  it("matches a value of the declared shape", () => {
    const v = verdictFor(formats, "openai_api_key", "sk-proj-abcdefghij0123456789");
    expect(v.state).toBe("match");
    expect(v.message).toContain("OpenAI API key");
  });

  it("warns on a mismatch but promises the value is still saved", () => {
    // The word "anyway" is the load-bearing bit: without it the user reads the
    // warning as a blocked save and goes chasing a key format.
    const v = verdictFor(formats, "openai_api_key", "totally-not-a-key");
    expect(v.state).toBe("mismatch");
    expect(v.message).toContain("Saving anyway");
    expect(v.message).toContain("OpenAI keys start sk-");
  });

  it("has no opinion when no format was declared", () => {
    expect(verdictFor(formats, null, "anything").state).toBe("unknown");
    expect(verdictFor(formats, undefined, "anything").state).toBe("unknown");
  });

  it("has no opinion when the catalogue is unavailable", () => {
    // An unreachable catalogue is not evidence that the key is wrong.
    expect(verdictFor(null, "openai_api_key", "sk-proj-abcdefghij0123456789"))
      .toEqual({ state: "unknown", message: "" });
  });

  it("has no opinion about an id the catalogue does not know", () => {
    expect(verdictFor(formats, "nope", "anything").state).toBe("unknown");
  });

  it("treats an uncompilable pattern as no opinion, not a warning", () => {
    // Rust validates patterns on its own side, so this should not happen — but
    // a JS-incompatible pattern must never turn into a scary red warning.
    const broken = catalogue([
      { id: "broken", label: "Broken", pattern: "(", hint: "n/a" },
    ]);
    expect(verdictFor(broken, "broken", "x").state).toBe("unknown");
  });
});

describe("loadSecretFormats", () => {
  it("fetches the catalogue once and caches it", async () => {
    vi.mocked(invoke).mockResolvedValue(FORMATS);
    const a = await loadSecretFormats();
    const b = await loadSecretFormats();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("secret_formats");
    expect(a?.get("openai_api_key")?.label).toBe("OpenAI API key");
    expect(b).toBe(a);
  });

  it("coalesces concurrent fetches into one invoke", async () => {
    vi.mocked(invoke).mockResolvedValue(FORMATS);
    await Promise.all([loadSecretFormats(), loadSecretFormats()]);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("resolves to null when the backend is absent (plain browser preview)", async () => {
    vi.mocked(invoke).mockRejectedValue(new Error("no backend"));
    await expect(loadSecretFormats()).resolves.toBeNull();
  });

  it("retries after a failed fetch instead of caching the failure", async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new Error("boom"));
    expect(await loadSecretFormats()).toBeNull();
    vi.mocked(invoke).mockResolvedValue(FORMATS);
    expect((await loadSecretFormats())?.size).toBe(2);
  });
});
