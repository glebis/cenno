import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import PromptPanel from "./PromptPanel";

describe("PromptPanel", () => {
  const prompt = { id: "p_1", title: "Check-in", body_md: "How is **focus**?", input: { kind: "text" } };

  it("renders title and markdown body", () => {
    render(<PromptPanel prompt={prompt} onAnswer={() => {}} />);
    expect(screen.getByText("Check-in")).toBeTruthy();
    expect(screen.getByText("focus").tagName).toBe("STRONG");
  });

  it("submits typed answer", () => {
    const onAnswer = vi.fn();
    render(<PromptPanel prompt={prompt} onAnswer={onAnswer} />);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "deep" } });
    fireEvent.click(screen.getByRole("button", { name: /send/i }));
    expect(onAnswer).toHaveBeenCalledWith("p_1", "deep", "text");
  });
});
