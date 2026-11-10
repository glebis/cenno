import { beforeEach, describe, expect, it, vi } from "vitest";

const disk = vi.hoisted(() => ({ config: {} as Record<string, unknown>, fail: false }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string) => {
    if (cmd === "read_config_file") {
      return disk.fail ? Promise.reject(new Error("io")) : Promise.resolve(disk.config);
    }
    // get_user_config = the launch snapshot: never changes in this test.
    if (cmd === "get_user_config") return Promise.resolve({});
    return Promise.resolve(null);
  }),
}));

import {
  getAgentColors,
  getDefaults,
  loadUserConfig,
  refreshUserConfig,
} from "./userConfig";
import { resolvePanelColor } from "./panelColor";

describe("refreshUserConfig (settings saved → panel picks it up live)", () => {
  beforeEach(async () => {
    disk.fail = false;
    disk.config = {};
    await loadUserConfig();
  });

  it("re-reads agent colors and default flow from disk", async () => {
    expect(getAgentColors()).toEqual({});

    disk.config = {
      agent_colors: { codex: "#123456" },
      defaults: { flow: "ema" },
    };
    await refreshUserConfig();

    expect(getAgentColors()).toEqual({ codex: "#123456" });
    expect(getDefaults()?.flow).toBe("ema");
    // …and the next prompt resolves with the new override.
    expect(
      resolvePanelColor({ agent: "Codex", overrides: getAgentColors() }).surface,
    ).toBe("#123456");
  });

  it("keeps the previous values when the read fails", async () => {
    disk.config = { agent_colors: { codex: "#123456" } };
    await refreshUserConfig();
    disk.fail = true;
    await refreshUserConfig();
    expect(getAgentColors()).toEqual({ codex: "#123456" });
  });
});
