import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import PromptPanel, { Prompt } from "./PromptPanel";

const base: Prompt = {
  id: "p_1",
  title: "Check-in",
  body_md: "How is **focus**?",
  input: { kind: "text" },
};

describe("PromptPanel", () => {
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
    const chips = screen.getAllByRole("button");
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
});
