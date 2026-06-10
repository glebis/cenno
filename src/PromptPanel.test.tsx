import { render, screen, fireEvent, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, it, expect, vi, type MockInstance } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import PromptPanel, { Prompt } from "./PromptPanel";
import { PANEL_MAX_HEIGHT } from "./panelResize";

// PromptPanel's mount effect measures the content and invokes resize_panel
// (panelResize.ts); outside Tauri the real invoke would throw.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

const base: Prompt = {
  id: "p_1",
  title: "Check-in",
  body_md: "How is **focus**?",
  input: { kind: "text" },
};

describe("PromptPanel", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockClear();
  });

  it("renders title and markdown body", () => {
    render(<PromptPanel prompt={base} onAnswer={() => {}} />);
    expect(screen.getByText("Check-in")).toBeTruthy();
    expect(screen.getByText("focus").tagName).toBe("STRONG");
  });

  it("submits typed answer", () => {
    const onAnswer = vi.fn();
    render(<PromptPanel prompt={base} onAnswer={onAnswer} />);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "deep" } });
    fireEvent.click(screen.getByRole("button", { name: /send/i }));
    expect(onAnswer).toHaveBeenCalledWith("p_1", "deep", "text");
  });

  it("renders choice chips and answers on tap", () => {
    const onAnswer = vi.fn();
    const prompt: Prompt = {
      ...base,
      id: "p_2",
      input: { kind: "choice" },
      choices: ["Deep work", "Email"],
    };
    render(<PromptPanel prompt={prompt} onAnswer={onAnswer} />);
    // Scope to the surface so the always-present chrome ✕ doesn't count.
    const surface = document.querySelector(".prompt-panel__content")!;
    const chips = within(surface as HTMLElement).getAllByRole("button");
    expect(chips).toHaveLength(2);
    expect(screen.getByRole("button", { name: "Deep work" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Email" }));
    expect(onAnswer).toHaveBeenCalledWith("p_2", "Email", "choice");
  });

  it("renders 7 scale numerals and answers with the stringified numeral", () => {
    const onAnswer = vi.fn();
    const prompt: Prompt = { ...base, id: "p_3", input: { kind: "scale" } };
    const { container } = render(
      <PromptPanel prompt={prompt} onAnswer={onAnswer} />,
    );
    expect(container.querySelectorAll(".cenno-scale__num")).toHaveLength(7);
    fireEvent.click(screen.getByRole("button", { name: "5" }));
    expect(onAnswer).toHaveBeenCalledWith("p_3", "5", "choice");
  });

  it("renders confirm Yes/No; No answers 'no'", () => {
    const onAnswer = vi.fn();
    const prompt: Prompt = { ...base, id: "p_4", input: { kind: "confirm" } };
    render(<PromptPanel prompt={prompt} onAnswer={onAnswer} />);
    expect(screen.getByRole("button", { name: "Yes" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "No" }));
    expect(onAnswer).toHaveBeenCalledWith("p_4", "no", "choice");
  });

  it("renders progress dots", () => {
    const prompt: Prompt = {
      ...base,
      id: "p_5",
      progress: { step: 2, total: 3 },
    };
    const { container } = render(
      <PromptPanel prompt={prompt} onAnswer={() => {}} />,
    );
    expect(container.querySelectorAll(".cenno-dot")).toHaveLength(3);
  });

  it("sets data-flow on the panel root from prompt.flow", () => {
    const prompt: Prompt = { ...base, id: "p_6", flow: "mood" };
    const { container } = render(
      <PromptPanel prompt={prompt} onAnswer={() => {}} />,
    );
    expect(container.querySelector('[data-flow="mood"]')).toBeTruthy();
  });

  it("renders a native a2ui payload instead of the desugared prompt", () => {
    const prompt: Prompt = {
      ...base,
      id: "p_7",
      a2ui: [
        {
          version: "v0.9",
          createSurface: { surfaceId: "main", catalogId: "cenno:catalog/v1" },
        },
        {
          version: "v0.9",
          updateComponents: {
            surfaceId: "main",
            components: [
              { id: "root", component: "Column", children: ["custom"] },
              { id: "custom", component: "Text", text: "custom surface" },
            ],
          },
        },
      ],
    };
    render(<PromptPanel prompt={prompt} onAnswer={() => {}} />);
    expect(screen.getByText("custom surface")).toBeTruthy();
    // The desugared title must NOT render — the native payload replaces it.
    expect(screen.queryByText("Check-in")).toBeNull();
  });

  it("defaults data-flow to question when flow is absent", () => {
    const { container } = render(
      <PromptPanel prompt={base} onAnswer={() => {}} />,
    );
    expect(container.querySelector('[data-flow="question"]')).toBeTruthy();
  });

  it("renders an a2ui payload that targets a non-'main' surfaceId", () => {
    const prompt: Prompt = {
      ...base,
      id: "p_8",
      a2ui: [
        { createSurface: { surfaceId: "alt", catalogId: "cenno:catalog/v1" } },
        {
          updateComponents: {
            surfaceId: "alt",
            components: [
              { id: "root", component: "Column", children: ["t"] },
              { id: "t", component: "Text", text: "alt surface" },
            ],
          },
        },
      ],
    };
    render(<PromptPanel prompt={prompt} onAnswer={() => {}} />);
    expect(screen.getByText("alt surface")).toBeTruthy();
  });

  it("falls back to the desugared prompt when a guard-valid a2ui payload cannot render", () => {
    const errorSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const onAnswer = vi.fn();
    // Guard-shape-valid (array, createSurface with the right catalog,
    // components array) but render-breaking: no component connects to the
    // "root" id the renderer mounts.
    const prompt: Prompt = {
      ...base,
      id: "p_9",
      a2ui: [
        { createSurface: { surfaceId: "main", catalogId: "cenno:catalog/v1" } },
        {
          updateComponents: {
            surfaceId: "main",
            components: [{ id: "orphan", component: "Text", text: "never shown" }],
          },
        },
      ],
    };
    render(<PromptPanel prompt={prompt} onAnswer={onAnswer} />);
    // Falls back to desugar(prompt): title renders instead of a blank panel.
    expect(screen.getByText("Check-in")).toBeTruthy();
    expect(screen.queryByText("never shown")).toBeNull();
    expect(errorSpy).toHaveBeenCalled();
    // The fallback is fully usable: submit still answers the prompt.
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "ok" } });
    fireEvent.click(screen.getByRole("button", { name: /send/i }));
    expect(onAnswer).toHaveBeenCalledWith("p_9", "ok", "text");
    errorSpy.mockRestore();
  });

  describe("panel chrome", () => {
    it("renders the cenno wordmark and a Dismiss button", () => {
      render(<PromptPanel prompt={base} onAnswer={() => {}} />);
      expect(screen.getByText("cenno")).toBeTruthy();
      expect(screen.getByRole("button", { name: "Dismiss" })).toBeTruthy();
    });

    it("calls onDismiss with the prompt id when ✕ is clicked", () => {
      const onDismiss = vi.fn();
      render(
        <PromptPanel prompt={base} onAnswer={() => {}} onDismiss={onDismiss} />,
      );
      fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
      expect(onDismiss).toHaveBeenCalledWith("p_1");
    });

    it("renders chrome around a native a2ui payload too", () => {
      const prompt: Prompt = {
        ...base,
        id: "p_chrome_a2ui",
        a2ui: [
          { createSurface: { surfaceId: "main", catalogId: "cenno:catalog/v1" } },
          {
            updateComponents: {
              surfaceId: "main",
              components: [
                { id: "root", component: "Column", children: ["t"] },
                { id: "t", component: "Text", text: "rich surface" },
              ],
            },
          },
        ],
      };
      render(<PromptPanel prompt={prompt} onAnswer={() => {}} />);
      expect(screen.getByText("rich surface")).toBeTruthy();
      expect(screen.getByText("cenno")).toBeTruthy();
      expect(screen.getByRole("button", { name: "Dismiss" })).toBeTruthy();
    });
  });

  describe("content-driven panel height", () => {
    // jsdom has no layout: stub the content wrapper's natural height and
    // make rAF synchronous so the post-mount measure runs inside render().
    let scrollHeightSpy: MockInstance<() => number>;

    beforeEach(() => {
      scrollHeightSpy = vi.spyOn(Element.prototype, "scrollHeight", "get");
      vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
        cb(0);
        return 1;
      });
      vi.stubGlobal("cancelAnimationFrame", () => {});
    });

    afterEach(() => {
      vi.unstubAllGlobals();
      scrollHeightSpy.mockRestore();
    });

    it("invokes resize_panel with a fitting height for a tall prompt", () => {
      // A tall EMA-style prompt (long title/body + scale + dots) measures
      // well past the max — the request must arrive already clamped.
      scrollHeightSpy.mockReturnValue(700);
      const prompt: Prompt = {
        ...base,
        id: "p_tall",
        flow: "ema",
        input: { kind: "scale" },
        progress: { step: 2, total: 5 },
      };
      render(<PromptPanel prompt={prompt} onAnswer={() => {}} />);
      expect(invoke).toHaveBeenCalledWith("resize_panel", {
        height: PANEL_MAX_HEIGHT,
      });
    });

    it("does not invoke resize_panel when content already fits the window", () => {
      // Desired (300, already in-band) within 4px of the current window
      // height (302) — no native resize round-trip.
      vi.stubGlobal("innerHeight", 302);
      scrollHeightSpy.mockReturnValue(300);
      render(<PromptPanel prompt={{ ...base, id: "p_fit" }} onAnswer={() => {}} />);
      expect(invoke).not.toHaveBeenCalledWith(
        "resize_panel",
        expect.anything(),
      );
    });
  });
});
