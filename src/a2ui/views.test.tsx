import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  TextView,
  ScaleView,
  ChipsView,
  TextFieldView,
  ButtonView,
  DotsView,
  SliderView,
  DateTimeView,
  ImageView,
} from "./views";

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn().mockResolvedValue(undefined),
}));

describe("TextView", () => {
  it("renders markdown strong", () => {
    render(<TextView markdown="is **bold**" role="body" />);
    expect(screen.getByText("bold").tagName).toBe("STRONG");
  });

  it("opens markdown links externally instead of navigating the panel", () => {
    // A plain <a href> click would replace the whole webview with the
    // linked page (Task 9 visual QA caught the panel turning into
    // example.com). Default must be prevented and the opener plugin used.
    render(<TextView markdown="see [the plan](https://example.com/plan)" />);
    const link = screen.getByRole("link", { name: "the plan" });
    const click = new MouseEvent("click", { bubbles: true, cancelable: true });
    link.dispatchEvent(click);
    expect(click.defaultPrevented).toBe(true);
    expect(openUrl).toHaveBeenCalledWith("https://example.com/plan");
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

  it("names the group from range and end labels", () => {
    render(
      <ScaleView
        min={1}
        max={7}
        minLabel="not at all"
        maxLabel="completely"
        onSelect={() => {}}
      />,
    );
    expect(
      screen.getByRole("group", { name: "1 (not at all) to 7 (completely)" }),
    ).toBeTruthy();
  });

  it("names the group from the range alone without end labels", () => {
    render(<ScaleView min={1} max={5} onSelect={() => {}} />);
    expect(screen.getByRole("group", { name: "1 to 5" })).toBeTruthy();
  });
});

describe("ChipsView", () => {
  const choices = [
    { label: "Deep work", value: "deep_work" },
    { label: "Meetings", value: "meetings" },
    { label: "Email", value: "email" },
    { label: "Wandering", value: "wandering" },
  ];

  it("renders choices as buttons and reports the chosen value", () => {
    const onSelect = vi.fn();
    render(<ChipsView choices={choices} onSelect={onSelect} />);
    expect(screen.getAllByRole("button")).toHaveLength(4);
    fireEvent.click(screen.getByRole("button", { name: "Deep work" }));
    expect(onSelect).toHaveBeenCalledWith("deep_work");
  });

  it("shows pressed state on every selected chip (multi-select)", () => {
    render(
      <ChipsView
        choices={choices}
        selected={["deep_work", "email"]}
        onSelect={() => {}}
      />,
    );
    expect(screen.getAllByRole("button", { pressed: true })).toHaveLength(2);
    const pressed = (name: string) =>
      screen.getByRole("button", { name }).getAttribute("aria-pressed");
    expect(pressed("Deep work")).toBe("true");
    expect(pressed("Email")).toBe("true");
    expect(pressed("Meetings")).toBe("false");
    expect(pressed("Wandering")).toBe("false");
  });

  const moods = [
    { label: "Awful", value: "awful" },
    { label: "Bad", value: "bad" },
    { label: "Okay", value: "okay" },
    { label: "Good", value: "good" },
    { label: "Great", value: "great" },
  ];

  it("words variant: renders bare-word buttons WITHOUT the pill chip class", () => {
    render(<ChipsView choices={moods} variant="words" onSelect={() => {}} />);
    const buttons = screen.getAllByRole("button");
    expect(buttons).toHaveLength(5);
    for (const b of buttons) {
      expect(b.className).toContain("cenno-word");
      expect(b.className).not.toContain("cenno-chip");
    }
  });

  it("words variant: clicking a word still reports the option value", () => {
    const onSelect = vi.fn();
    render(<ChipsView choices={moods} variant="words" onSelect={onSelect} />);
    fireEvent.click(screen.getByRole("button", { name: "Great" }));
    expect(onSelect).toHaveBeenCalledWith("great");
  });

  it("words variant: multi-select aria-pressed still works", () => {
    render(
      <ChipsView
        choices={moods}
        variant="words"
        selected={["okay"]}
        onSelect={() => {}}
      />,
    );
    expect(
      screen.getByRole("button", { name: "Okay" }).getAttribute("aria-pressed"),
    ).toBe("true");
    expect(
      screen.getByRole("button", { name: "Bad" }).getAttribute("aria-pressed"),
    ).toBe("false");
  });
});

describe("TextFieldView", () => {
  it("submits on Enter, shows mic stub when voice", () => {
    const onSubmit = vi.fn();
    render(<TextFieldView voice label="Your reply" onSubmit={onSubmit} />);
    const mic = screen.getByTitle("voice arrives in plan 3");
    expect(mic).toBeTruthy();
    expect((mic as HTMLButtonElement).disabled).toBe(true);
    // the label is the input's accessible name (placeholder alone is fragile)
    const input = screen.getByRole("textbox", { name: "Your reply" });
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

  it("quiet Send is text-only (no primary pill class) and still fires", () => {
    const onClick = vi.fn();
    render(
      <ButtonView variant="quiet" onClick={onClick}>
        Send
      </ButtonView>,
    );
    const btn = screen.getByRole("button", { name: "Send" });
    expect(btn.className).toContain("cenno-button--quiet");
    expect(btn.className).not.toContain("cenno-button--primary");
    fireEvent.click(btn);
    expect(onClick).toHaveBeenCalledTimes(1);
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
      "DateTimeInput",
      "Dots",
      "Image",
      "Row",
      "Scale",
      "Slider",
      "Text",
      "TextField",
    ]);
  });
});

describe("SliderView", () => {
  it("renders a range input with bounds, step, and end labels", () => {
    render(
      <SliderView
        min={0}
        max={10}
        step={1}
        value={5}
        minLabel="little"
        maxLabel="a lot"
        onChange={() => {}}
        onCommit={() => {}}
      />,
    );
    const slider = screen.getByRole("slider");
    expect(slider.getAttribute("min")).toBe("0");
    expect(slider.getAttribute("max")).toBe("10");
    expect(slider.getAttribute("step")).toBe("1");
    expect((slider as HTMLInputElement).value).toBe("5");
    expect(screen.getByText("little")).toBeTruthy();
    expect(screen.getByText("a lot")).toBeTruthy();
  });

  it("names the slider from range and end labels when no label is given", () => {
    render(
      <SliderView
        min={0}
        max={10}
        minLabel="little"
        maxLabel="a lot"
        onChange={() => {}}
        onCommit={() => {}}
      />,
    );
    expect(
      screen.getByRole("slider", { name: "0 (little) to 10 (a lot)" }),
    ).toBeTruthy();
  });

  it("reports value changes while dragging without committing", () => {
    const onChange = vi.fn();
    const onCommit = vi.fn();
    render(
      <SliderView
        min={0}
        max={10}
        value={5}
        onChange={onChange}
        onCommit={onCommit}
      />,
    );
    fireEvent.change(screen.getByRole("slider"), { target: { value: "7" } });
    expect(onChange).toHaveBeenCalledWith(7);
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("commits the current value on Enter (keyboard path)", () => {
    const onCommit = vi.fn();
    render(
      <SliderView
        min={0}
        max={10}
        value={5}
        onChange={() => {}}
        onCommit={onCommit}
      />,
    );
    const slider = screen.getByRole("slider");
    fireEvent.change(slider, { target: { value: "7" } });
    fireEvent.keyDown(slider, { key: "Enter" });
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith(7);
  });

  it("does not commit on arrow keys (value still being chosen)", () => {
    const onCommit = vi.fn();
    render(
      <SliderView
        min={0}
        max={10}
        value={5}
        onChange={() => {}}
        onCommit={onCommit}
      />,
    );
    const slider = screen.getByRole("slider");
    // jsdom doesn't implement native range keyboard stepping; simulate the
    // value change the arrow press would cause, then the key events.
    fireEvent.change(slider, { target: { value: "6" } });
    fireEvent.keyDown(slider, { key: "ArrowRight" });
    fireEvent.keyUp(slider, { key: "ArrowRight" });
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("commits the settled value on pointer release", () => {
    const onCommit = vi.fn();
    render(
      <SliderView
        min={0}
        max={10}
        value={5}
        onChange={() => {}}
        onCommit={onCommit}
      />,
    );
    const slider = screen.getByRole("slider");
    fireEvent.change(slider, { target: { value: "7" } });
    fireEvent.pointerUp(slider);
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith(7);
  });
});

describe("DateTimeView", () => {
  it("renders a date input with bounds and reports changes", () => {
    const onChange = vi.fn();
    render(
      <DateTimeView
        kind="date"
        label="Remind me on"
        min="2026-06-10"
        max="2026-12-31"
        onChange={onChange}
        onSubmit={() => {}}
      />,
    );
    const input = screen.getByLabelText("Remind me on") as HTMLInputElement;
    expect(input.type).toBe("date");
    expect(input.getAttribute("min")).toBe("2026-06-10");
    expect(input.getAttribute("max")).toBe("2026-12-31");
    fireEvent.change(input, { target: { value: "2026-06-15" } });
    expect(onChange).toHaveBeenCalledWith("2026-06-15");
  });

  it("renders time and datetime-local kinds", () => {
    const { rerender } = render(
      <DateTimeView kind="time" label="t" onChange={() => {}} />,
    );
    expect((screen.getByLabelText("t") as HTMLInputElement).type).toBe("time");
    rerender(<DateTimeView kind="datetime" label="t" onChange={() => {}} />);
    expect((screen.getByLabelText("t") as HTMLInputElement).type).toBe(
      "datetime-local",
    );
  });

  it("submits the current value on Enter", () => {
    const onSubmit = vi.fn();
    render(
      <DateTimeView
        kind="date"
        label="Remind me on"
        value="2026-06-15"
        onChange={() => {}}
        onSubmit={onSubmit}
      />,
    );
    fireEvent.keyDown(screen.getByLabelText("Remind me on"), { key: "Enter" });
    expect(onSubmit).toHaveBeenCalledWith("2026-06-15");
  });
});

describe("ImageView", () => {
  it("renders the image with src, alt, fit, and variant", () => {
    render(
      <ImageView
        url="https://example.com/pic.png"
        description="two design options"
        fit="contain"
        variant="largeFeature"
      />,
    );
    const img = screen.getByRole("img", {
      name: "two design options",
    }) as HTMLImageElement;
    expect(img.src).toBe("https://example.com/pic.png");
    expect(img.className).toContain("cenno-image--largeFeature");
    expect(img.style.objectFit).toBe("contain");
  });

  it("is hidden from assistive tech without a description", () => {
    render(<ImageView url="https://example.com/pic.png" />);
    expect(screen.queryByRole("img")).toBeNull(); // alt="" → presentation
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
