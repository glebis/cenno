import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  PANEL_MAX_HEIGHT,
  PANEL_MIN_HEIGHT,
  clampPanelHeight,
  desiredPanelHeight,
  observePanelContent,
  syncPanelHeight,
} from "./panelResize";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

/** A root + content pair with a stubbed natural content height (jsdom has
 *  no layout, so scrollHeight must be faked). */
function makePanel(contentHeight: number, rootPadding?: string) {
  const root = document.createElement("div");
  if (rootPadding) {
    root.style.paddingTop = rootPadding;
    root.style.paddingBottom = rootPadding;
  }
  const content = document.createElement("div");
  Object.defineProperty(content, "scrollHeight", {
    value: contentHeight,
    configurable: true,
  });
  root.appendChild(content);
  return { root, content };
}

beforeEach(() => {
  vi.mocked(invoke).mockClear();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("clampPanelHeight", () => {
  it("clamps to the [240, 560] band and rejects non-finite input", () => {
    expect(clampPanelHeight(100)).toBe(PANEL_MIN_HEIGHT);
    expect(clampPanelHeight(380)).toBe(380);
    expect(clampPanelHeight(9000)).toBe(PANEL_MAX_HEIGHT);
    expect(clampPanelHeight(NaN)).toBe(PANEL_MIN_HEIGHT);
    expect(clampPanelHeight(Infinity)).toBe(PANEL_MIN_HEIGHT);
  });
});

describe("desiredPanelHeight", () => {
  it("adds the root's vertical padding to the content's natural height", () => {
    const { root, content } = makePanel(300, "16px");
    expect(desiredPanelHeight(content, root)).toBe(300 + 32);
  });

  it("clamps a tall prompt to the max and a short one to the min", () => {
    const tall = makePanel(700);
    expect(desiredPanelHeight(tall.content, tall.root)).toBe(PANEL_MAX_HEIGHT);
    const short = makePanel(80);
    expect(desiredPanelHeight(short.content, short.root)).toBe(PANEL_MIN_HEIGHT);
  });
});

describe("syncPanelHeight", () => {
  it("invokes resize_panel with the clamped height for a tall prompt", () => {
    const { root, content } = makePanel(700, "16px");
    expect(syncPanelHeight(content, root, 240)).toBe(true);
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("resize_panel", {
      height: PANEL_MAX_HEIGHT,
    });
  });

  it("requests a shrink when the content got shorter than the window", () => {
    const { root, content } = makePanel(200, "16px");
    expect(syncPanelHeight(content, root, 560)).toBe(true);
    expect(invoke).toHaveBeenCalledWith("resize_panel", { height: 240 });
  });

  it("skips the call when within 4px of the current height (no resize loop)", () => {
    const { root, content } = makePanel(338, "16px"); // desired = 370
    expect(syncPanelHeight(content, root, 368)).toBe(false);
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe("observePanelContent", () => {
  it("measures once after mount via rAF and invokes resize_panel", () => {
    // Synchronous rAF: the debounce collapses to an immediate measure.
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
      cb(0);
      return 1;
    });
    vi.stubGlobal("cancelAnimationFrame", () => {});
    const { root, content } = makePanel(700, "16px");
    const cleanup = observePanelContent(content, root);
    expect(invoke).toHaveBeenCalledWith("resize_panel", {
      height: PANEL_MAX_HEIGHT,
    });
    cleanup(); // must not throw without a ResizeObserver (jsdom has none)
  });
});
