import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import App from "./App";

// Cold-start scenario: the `prompt` event fired before the webview mounted,
// so `listen` never delivers anything — the panel must recover by pulling
// `pending_prompts` after registering the listener.
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string) =>
    cmd === "pending_prompts"
      ? Promise.resolve([
          {
            id: "p_0",
            request: {
              title: "How are you feeling?",
              body_md: "",
              input: { kind: "choice" },
              choices: ["great", "good", "okay", "low", "rough"],
              flow: "mood",
            },
          },
        ])
      : Promise.resolve(false),
  ),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({ hide: vi.fn() })),
}));

describe("App cold-start replay", () => {
  it("renders a pending prompt pulled on mount when no event ever fires", async () => {
    render(<App />);
    expect(await screen.findByText("How are you feeling?")).toBeTruthy();
    expect(screen.getByRole("button", { name: "great" })).toBeTruthy();
  });
});
