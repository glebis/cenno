import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import {
  TextView,
  ScaleView,
  ChipsView,
  TextFieldView,
  ButtonView,
  DotsView,
} from "./views";

describe("TextView", () => {
  it("renders markdown strong", () => {
    render(<TextView markdown="is **bold**" role="body" />);
    expect(screen.getByText("bold").tagName).toBe("STRONG");
  });
});

describe("ScaleView", () => {
  it("renders n targets and reports selection", () => {
    const onSelect = vi.fn();
    render(
      <ScaleView
        min={1}
        max={7}
        minLabel="not at all"
        maxLabel="completely"
        onSelect={onSelect}
      />,
    );
    expect(screen.getAllByRole("button")).toHaveLength(7);
    expect(screen.getByText("not at all")).toBeTruthy();
    expect(screen.getByText("completely")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "5" }));
    expect(onSelect).toHaveBeenCalledWith(5);
  });
});

describe("ChipsView", () => {
  it("renders choices as buttons and reports the chosen label", () => {
    const onSelect = vi.fn();
    render(
      <ChipsView
        choices={["Deep work", "Meetings", "Email", "Wandering"]}
        onSelect={onSelect}
      />,
    );
    expect(screen.getAllByRole("button")).toHaveLength(4);
    fireEvent.click(screen.getByRole("button", { name: "Deep work" }));
    expect(onSelect).toHaveBeenCalledWith("Deep work");
  });
});

describe("TextFieldView", () => {
  it("submits on Enter, shows mic stub when voice", () => {
    const onSubmit = vi.fn();
    render(<TextFieldView voice onSubmit={onSubmit} />);
    const mic = screen.getByTitle("voice arrives in plan 3");
    expect(mic).toBeTruthy();
    expect((mic as HTMLButtonElement).disabled).toBe(true);
    const input = screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "deep work" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSubmit).toHaveBeenCalledWith("deep work");
  });

  it("has no mic stub without voice and guards IME composition", () => {
    const onSubmit = vi.fn();
    render(<TextFieldView onSubmit={onSubmit} />);
    expect(screen.queryByTitle("voice arrives in plan 3")).toBeNull();
    const input = screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "おはよう" } });
    // Enter that confirms an IME composition must NOT submit
    fireEvent.keyDown(input, { key: "Enter", isComposing: true });
    expect(onSubmit).not.toHaveBeenCalled();
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSubmit).toHaveBeenCalledWith("おはよう");
  });
});

describe("ButtonView", () => {
  it("fires action", () => {
    const onClick = vi.fn();
    render(<ButtonView onClick={onClick}>Send</ButtonView>);
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("does not fire when disabled", () => {
    const onClick = vi.fn();
    render(
      <ButtonView onClick={onClick} disabled>
        Send
      </ButtonView>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    expect(onClick).not.toHaveBeenCalled();
  });
});

// Adapters are covered by the Task 6 integration; this only proves the
// catalog module loads and registers the full component set.
describe("cennoCatalog", () => {
  it("exposes the cenno component set under cenno:catalog/v1", async () => {
    const { cennoCatalog } = await import("./catalog");
    expect(cennoCatalog.id).toBe("cenno:catalog/v1");
    expect([...cennoCatalog.components.keys()].sort()).toEqual([
      "Button",
      "ChoicePicker",
      "Column",
      "Dots",
      "Row",
      "Scale",
      "Text",
      "TextField",
    ]);
  });
});

describe("DotsView", () => {
  it("renders total dots with current active", () => {
    render(<DotsView step={2} total={3} />);
    const dots = screen.getAllByRole("listitem");
    expect(dots).toHaveLength(3);
    expect(dots[0].getAttribute("aria-current")).toBeNull();
    expect(dots[1].getAttribute("aria-current")).toBe("step");
    expect(dots[2].getAttribute("aria-current")).toBeNull();
  });
});
