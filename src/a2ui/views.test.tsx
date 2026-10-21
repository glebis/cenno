import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { beforeEach, describe, it, expect, vi } from "vitest";
import { openUrl } from "@tauri-apps/plugin-opener";
import { invoke } from "@tauri-apps/api/core";
import { clearImageCache } from "./image";
import {
  TextView,
  ScaleView,
  ChipsView,
  CheckListView,
  ChoicePickerView,
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

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

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

describe("CheckListView", () => {
  const choices = [
    { label: "Deep work", value: "deep" },
    { label: "Meetings", value: "meet" },
  ];

  it("renders radios with aria-checked for a single-select list", () => {
    render(
      <CheckListView
        choices={choices}
        selected={["meet"]}
        variant="single"
        onSelect={vi.fn()}
      />,
    );
    const radios = screen.getAllByRole("radio");
    expect(radios).toHaveLength(2);
    expect(radios[1].getAttribute("aria-checked")).toBe("true");
    expect(radios[0].getAttribute("aria-checked")).toBe("false");
  });

  it("reports the value, not the label, on click", () => {
    const onSelect = vi.fn();
    render(
      <CheckListView choices={choices} variant="single" onSelect={onSelect} />,
    );
    fireEvent.click(screen.getByRole("radio", { name: "Deep work" }));
    expect(onSelect).toHaveBeenCalledWith("deep");
  });

  it("renders checkboxes and toggles independently in multi-select", () => {
    const onSelect = vi.fn();
    render(
      <CheckListView
        choices={choices}
        selected={["deep"]}
        variant="multiple"
        onSelect={onSelect}
      />,
    );
    expect(
      screen.getAllByRole("checkbox")[0].getAttribute("aria-checked"),
    ).toBe("true");
    fireEvent.click(screen.getByRole("checkbox", { name: "Meetings" }));
    expect(onSelect).toHaveBeenCalledWith("meet");
  });

  it("moves selection with the arrow keys in a single-select list", () => {
    const onSelect = vi.fn();
    render(
      <CheckListView choices={choices} variant="single" onSelect={onSelect} />,
    );
    fireEvent.keyDown(screen.getByRole("radio", { name: "Deep work" }), {
      key: "ArrowDown",
    });
    expect(onSelect).toHaveBeenCalledWith("meet");
  });

  it("selects the focused row on Space and Enter", () => {
    const onSelect = vi.fn();
    render(
      <CheckListView choices={choices} variant="single" onSelect={onSelect} />,
    );
    const first = screen.getByRole("radio", { name: "Deep work" });
    fireEvent.keyDown(first, { key: " " });
    fireEvent.keyDown(first, { key: "Enter" });
    expect(onSelect).toHaveBeenCalledTimes(2);
  });

  it("wraps arrow navigation at both ends", () => {
    const onSelect = vi.fn();
    render(
      <CheckListView choices={choices} variant="single" onSelect={onSelect} />,
    );
    // ArrowDown from the last row wraps to the first.
    fireEvent.keyDown(screen.getByRole("radio", { name: "Meetings" }), {
      key: "ArrowDown",
    });
    expect(onSelect).toHaveBeenLastCalledWith("deep");
    // ArrowUp from the first row wraps to the last.
    fireEvent.keyDown(screen.getByRole("radio", { name: "Deep work" }), {
      key: "ArrowUp",
    });
    expect(onSelect).toHaveBeenLastCalledWith("meet");
  });

  it("moves focus without selecting in a multi-select list", () => {
    const onSelect = vi.fn();
    render(
      <CheckListView
        choices={choices}
        variant="multiple"
        onSelect={onSelect}
      />,
    );
    const first = screen.getByRole("checkbox", { name: "Deep work" });
    first.focus();
    fireEvent.keyDown(first, { key: "ArrowDown" });
    expect(document.activeElement).toBe(
      screen.getByRole("checkbox", { name: "Meetings" }),
    );
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("keeps a single tab stop in a single-select list", () => {
    render(
      <CheckListView
        choices={choices}
        selected={["meet"]}
        variant="single"
        onSelect={vi.fn()}
      />,
    );
    expect(
      screen.getAllByRole("radio").map((r) => r.getAttribute("tabindex")),
    ).toEqual(["-1", "0"]);
  });

  it("falls back to the first row as the tab stop when nothing is selected", () => {
    render(
      <CheckListView choices={choices} variant="single" onSelect={vi.fn()} />,
    );
    expect(
      screen.getAllByRole("radio").map((r) => r.getAttribute("tabindex")),
    ).toEqual(["0", "-1"]);
  });

  it("keeps every row tabbable in a multi-select list", () => {
    render(
      <CheckListView
        choices={choices}
        variant="multiple"
        onSelect={vi.fn()}
      />,
    );
    expect(
      screen.getAllByRole("checkbox").map((r) => r.getAttribute("tabindex")),
    ).toEqual(["0", "0"]);
  });

  it("wraps the group in a radiogroup (single) or group (multiple)", () => {
    const { unmount } = render(
      <CheckListView
        choices={choices}
        variant="single"
        label="How was today?"
        onSelect={vi.fn()}
      />,
    );
    expect(
      screen.getByRole("radiogroup", { name: "How was today?" }),
    ).toBeTruthy();
    unmount();
    render(
      <CheckListView
        choices={choices}
        variant="multiple"
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getByRole("group")).toBeTruthy();
  });

  it("keeps a 44px tap target per row", () => {
    const { container } = render(
      <CheckListView choices={choices} variant="single" onSelect={vi.fn()} />,
    );
    expect(container.querySelectorAll(".cenno-checklist__row")).toHaveLength(2);
  });
});

// displayStyle → view. `undefined` is what an omitted displayStyle parses to
// (`.optional()` wraps `.default("checkbox")`, so the schema default never
// materialises), and it must keep the shipped chip pills.
describe("ChoicePickerView (displayStyle)", () => {
  const choices = [
    { label: "Deep work", value: "deep" },
    { label: "Meetings", value: "meet" },
  ];

  it("keeps chips when displayStyle is omitted", () => {
    const { container } = render(
      <ChoicePickerView choices={choices} onSelect={vi.fn()} />,
    );
    expect(container.querySelector(".cenno-chips")).toBeTruthy();
    expect(container.querySelector(".cenno-checklist")).toBeNull();
  });

  it("keeps chips for an explicit displayStyle 'chips'", () => {
    const { container } = render(
      <ChoicePickerView
        choices={choices}
        displayStyle="chips"
        onSelect={vi.fn()}
      />,
    );
    expect(container.querySelector(".cenno-chips")).toBeTruthy();
    expect(container.querySelector(".cenno-checklist")).toBeNull();
  });

  it("renders the radio list for displayStyle 'checkbox'", () => {
    render(
      <ChoicePickerView
        choices={choices}
        displayStyle="checkbox"
        variant="mutuallyExclusive"
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getAllByRole("radio")).toHaveLength(2);
  });

  it("renders checkboxes for displayStyle 'checkbox' + multipleSelection", () => {
    render(
      <ChoicePickerView
        choices={choices}
        displayStyle="checkbox"
        variant="multipleSelection"
        selected={["deep"]}
        onSelect={vi.fn()}
      />,
    );
    const boxes = screen.getAllByRole("checkbox");
    expect(boxes).toHaveLength(2);
    expect(boxes[0].getAttribute("aria-checked")).toBe("true");
  });

  it("keeps the words variant on chips even when displayStyle is 'checkbox'", () => {
    render(
      <ChoicePickerView
        choices={choices}
        displayStyle="checkbox"
        variant="words"
        onSelect={vi.fn()}
      />,
    );
    const first = screen.getAllByRole("button")[0];
    expect(first.className).toContain("cenno-word");
    expect(first.className).not.toContain("cenno-checklist__row");
  });
});

describe("TextFieldView", () => {
  it("submits on Enter, shows the mic when voice", () => {
    const onSubmit = vi.fn();
    const onMicToggle = vi.fn();
    render(
      <TextFieldView
        voice
        label="Your reply"
        onSubmit={onSubmit}
        onMicToggle={onMicToggle}
      />,
    );
    const mic = screen.getByRole("button", { name: "Dictate your answer" });
    expect(mic.getAttribute("aria-pressed")).toBe("false");
    fireEvent.click(mic);
    expect(onMicToggle).toHaveBeenCalledTimes(1);
    // the label is the input's accessible name (placeholder alone is fragile)
    const input = screen.getByRole("textbox", { name: "Your reply" });
    fireEvent.change(input, { target: { value: "deep work" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSubmit).toHaveBeenCalledWith("deep work");
  });

  it("recording flips the mic state; the field stays editable", () => {
    render(
      <TextFieldView
        voice
        recording
        label="Your reply"
        onSubmit={vi.fn()}
        onMicToggle={vi.fn()}
      />,
    );
    const mic = screen.getByRole("button", { name: "Stop dictation" });
    expect(mic.getAttribute("aria-pressed")).toBe("true");
    expect(mic.className).toContain("cenno-field__mic--recording");
    const input = screen.getByRole("textbox", { name: "Your reply" });
    expect((input as HTMLInputElement).disabled).toBe(false);
  });

  it("voiceError renders the graceful error caption", () => {
    render(
      <TextFieldView
        voice
        voiceError="Microphone permission denied"
        label="Your reply"
        onSubmit={vi.fn()}
        onMicToggle={vi.fn()}
      />,
    );
    expect(screen.getByRole("alert").textContent).toContain(
      "Microphone permission denied",
    );
  });

  it("mic without a toggle handler renders disabled", () => {
    render(<TextFieldView voice label="Your reply" onSubmit={vi.fn()} />);
    const mic = screen.getByRole("button", { name: "Dictate your answer" });
    expect((mic as HTMLButtonElement).disabled).toBe(true);
  });

  it("has no mic without voice and guards IME composition", () => {
    const onSubmit = vi.fn();
    render(<TextFieldView onSubmit={onSubmit} />);
    expect(
      screen.queryByRole("button", { name: "Dictate your answer" }),
    ).toBeNull();
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

const DATA_URI = "data:image/png;base64,iVBORw0KGgo=";

describe("ImageView", () => {
  it("renders the image with src, alt, fit, and variant", async () => {
    const { container } = render(
      <ImageView
        url={DATA_URI}
        description="two design options"
        fit="contain"
        variant="largeFeature"
      />,
    );
    const img = (await screen.findByRole("img", {
      name: "two design options",
    })) as HTMLImageElement;
    expect(img.getAttribute("src")).toBe(DATA_URI);
    // The variant now sizes the fixed BOX (was: the <img> itself), so the
    // panel cannot jump when the state changes.
    expect(container.querySelector(".cenno-image-box--largeFeature")).toBeTruthy();
    expect(img.style.objectFit).toBe("contain");
  });

  it("is hidden from assistive tech without a description", async () => {
    const { container } = render(<ImageView url={DATA_URI} />);
    await waitFor(() => expect(container.querySelector("img")).toBeTruthy());
    expect(screen.queryByRole("img")).toBeNull(); // alt="" → presentation
  });
});

describe("ImageView states", () => {
  beforeEach(() => {
    clearImageCache();
    vi.mocked(invoke).mockReset();
  });

  it("shows a placeholder while a local path resolves", async () => {
    vi.mocked(invoke).mockReturnValue(new Promise(() => {})); // never settles
    render(<ImageView url="/Users/x/a.png" />);
    expect(screen.getByTestId("image-loading")).toBeTruthy();
  });

  it("shows the image once a data: URI resolves", async () => {
    render(<ImageView url={DATA_URI} description="logo" />);
    const img = await screen.findByRole("img", { name: "logo" });
    expect(img.getAttribute("src")).toBe(DATA_URI);
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
    expect(await screen.findByText(/image failed to load — failed to decode/)).toBeTruthy();
  });

  it("keeps a fixed box so the panel does not jump between states", async () => {
    const { container } = render(<ImageView url="/Users/x/a.png" variant="smallFeature" />);
    expect(container.querySelector(".cenno-image-box--smallFeature")).toBeTruthy();
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
