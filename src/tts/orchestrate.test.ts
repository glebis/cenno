import { describe, expect, it } from "vitest";
import { orchestratePrompt, type PlayerConfig, type SpeakIo, type SpeakablePrompt } from "./orchestrate";

const prompt = (over: Partial<SpeakablePrompt> = {}): SpeakablePrompt => ({
  id: "p1",
  title: "Deploy?",
  body_md: "Ship it now?",
  urgency: "High",
  ...over,
});

const cfg = (over: Partial<PlayerConfig> = {}): PlayerConfig => ({
  enabled: true,
  minUrgency: "high",
  ...over,
});

/** Fake IO that records the order of every side effect. */
function fakeIo(over: Partial<SpeakIo> = {}) {
  const calls: string[] = [];
  const io: SpeakIo = {
    readFresh: () => {
      calls.push("readFresh");
      return Promise.resolve(cfg());
    },
    speak: () => {
      calls.push("speak");
      return Promise.resolve();
    },
    panelReady: () => {
      calls.push("panelReady");
    },
    setSpeaking: (v) => {
      calls.push(`setSpeaking(${v})`);
    },
    cancelled: () => false,
    ...over,
  };
  return { io, calls };
}

describe("orchestratePrompt — silent prompts show immediately", () => {
  it("signals panelReady without speaking when TTS is disabled", async () => {
    const { io, calls } = fakeIo({ readFresh: () => Promise.resolve(cfg({ enabled: false })) });
    await orchestratePrompt(prompt(), cfg(), io);
    expect(calls).not.toContain("speak");
    expect(calls).toContain("panelReady");
  });

  it("signals panelReady without speaking when urgency is below threshold", async () => {
    const { io, calls } = fakeIo();
    await orchestratePrompt(prompt({ urgency: "Low" }), cfg(), io);
    expect(calls).not.toContain("speak");
    expect(calls).toContain("panelReady");
  });

  it("signals panelReady when the prompt has no speakable text", async () => {
    const { io, calls } = fakeIo();
    await orchestratePrompt(prompt({ title: "", body_md: "", say: "" }), cfg(), io);
    expect(calls).not.toContain("speak");
    expect(calls).toContain("panelReady");
  });
});

describe("orchestratePrompt — spoken prompts show at playback start", () => {
  it("speaks, then signals panelReady only after speak resolves", async () => {
    const { io, calls } = fakeIo();
    await orchestratePrompt(prompt(), cfg(), io);
    expect(calls.indexOf("speak")).toBeGreaterThanOrEqual(0);
    expect(calls.indexOf("panelReady")).toBeGreaterThan(calls.indexOf("speak"));
    expect(calls).toContain("setSpeaking(true)");
  });

  it("still signals panelReady when speak rejects (never hold the panel hostage)", async () => {
    const { io, calls } = fakeIo({ speak: () => Promise.reject(new Error("engine gone")) });
    await orchestratePrompt(prompt(), cfg(), io);
    expect(calls).toContain("panelReady");
  });

  it("falls back to the snapshot config when the fresh read rejects", async () => {
    const { io, calls } = fakeIo({ readFresh: () => Promise.reject(new Error("no fs")) });
    await orchestratePrompt(prompt(), cfg({ enabled: true, minUrgency: "high" }), io);
    expect(calls).toContain("speak");
  });
});

describe("orchestratePrompt — cancellation stands down silently", () => {
  it("does nothing further when cancelled during the config read", async () => {
    let done = false;
    const { io, calls } = fakeIo({ cancelled: () => done });
    io.readFresh = () => {
      calls.push("readFresh");
      done = true;
      return Promise.resolve(cfg());
    };
    await orchestratePrompt(prompt(), cfg(), io);
    expect(calls).not.toContain("speak");
    expect(calls).not.toContain("panelReady");
  });

  it("does not signal panelReady when cancelled during synthesis", async () => {
    let done = false;
    const { io, calls } = fakeIo({ cancelled: () => done });
    io.speak = () => {
      calls.push("speak");
      done = true; // user answered/dismissed while audio was being prepared
      return Promise.resolve();
    };
    await orchestratePrompt(prompt(), cfg(), io);
    expect(calls).toContain("speak");
    expect(calls).not.toContain("panelReady");
  });
});
