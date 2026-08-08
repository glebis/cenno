import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SettingsWindow, { formatGuardianBytes } from "./SettingsWindow";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn<(command: string, args?: unknown) => Promise<unknown>>(),
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, handler: (event: { payload: unknown }) => void) => {
    mocks.listeners.set(event, handler);
    return Promise.resolve(() => mocks.listeners.delete(event));
  }),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn(() => Promise.resolve()),
}));

const status = {
  schemaVersion: 1 as const,
  generatedAt: "2026-08-08T09:30:00Z",
  sample: {
    schemaVersion: 1 as const,
    targetId: "system-volume",
    observedAt: "2026-08-08T09:29:58Z",
    totalBytes: "500000000000",
    availableBytes: "125000000000",
    freeBytes: "120000000000",
    blockSizeBytes: "4096",
  },
  evaluation: {
    severity: "healthy" as const,
    reasons: [] as string[],
    evaluatedAt: "2026-08-08T09:30:00Z",
    latestSampleAt: "2026-08-08T09:29:58Z",
    availableBytes: "125000000000",
    availableBasisPoints: 2500,
  },
};

const candidate = {
  scope: "user-caches",
  kind: "folder" as const,
  relativePath: "Caches/[redacted]/build-output",
  bytes: "12500000000",
};

const diagnostics = {
  schemaVersion: 1 as const,
  generatedAt: "2026-08-08T09:30:00Z",
  report: {
    schemaVersion: 1 as const,
    observedAt: "2026-08-08T09:29:55Z",
    scopes: [
      { id: "user-caches", files: 42, folders: 7, bytes: "15000000000", readable: true },
    ],
    totals: { files: 42, folders: 7, bytes: "15000000000" },
    topCandidates: [candidate],
    partial: false,
    limitsHit: [] as string[],
    errorCount: 0,
  },
  growth: null,
  preview: {
    schemaVersion: 1 as const,
    generatedFrom: "2026-08-08T09:29:55Z",
    previewRef: "opaque-preview-ref",
    estimatedBytes: "12500000000",
    candidates: [candidate],
  },
};

const runs = {
  schemaVersion: 1 as const,
  sequence: 4,
  events: [] as unknown[],
  runs: [
    {
      authorizationId: "auth.imported-1",
      profileId: "bounded-cache-scan",
      profileVersion: 3,
      expiresAt: "2026-08-09T09:30:00Z",
      status: "imported" as const,
      progress: 0,
    },
    {
      authorizationId: "auth.running-1",
      profileId: "downloads-observation",
      profileVersion: 2,
      expiresAt: "2026-08-09T10:30:00Z",
      status: "running" as const,
      progress: 45,
      claimedAt: "2026-08-08T09:31:00Z",
      startedAt: "2026-08-08T09:31:01Z",
    },
    {
      authorizationId: "auth.done-1",
      profileId: "developer-builds",
      profileVersion: 1,
      expiresAt: "2026-08-08T11:30:00Z",
      status: "succeeded" as const,
      progress: 100,
      finishedAt: "2026-08-08T09:35:00Z",
    },
    {
      authorizationId: "auth.expired-1",
      profileId: "bounded-cache-scan",
      profileVersion: 3,
      expiresAt: "2026-08-08T09:40:00.000Z",
      status: "expired" as const,
      progress: 0,
      finishedAt: "2026-08-08T09:40:00.001Z",
      result: { code: "authorization_expired" },
    },
  ],
};

const monitoringPolicy = {
  schemaVersion: 1 as const,
  revision: 7,
  policy: {
    warningBytes: "53687091200",
    criticalBytes: "21474836480",
    warningBasisPoints: 1000,
    criticalBasisPoints: 400,
  },
};

const securityStatus = {
  schemaVersion: 1 as const,
  reportedBy: "disk-guardian" as const,
  execution: { status: "available" as const },
  personalContractsAuthority: {
    status: "active" as const,
    generation: 4,
    activeKey: {
      keyId: "pc-key",
      publicKeyFingerprint: `sha256:${"a".repeat(64)}`,
    },
    pendingKey: null,
    retiredKeyCount: 1,
    revokedKeyCount: 0,
  },
  executorIdentity: {
    status: "active" as const,
    generation: 2,
    executorId: "guardian.local",
    activeKey: {
      keyId: "guardian-key",
      publicKeyFingerprint: `sha256:${"b".repeat(64)}`,
    },
    pendingKey: null,
  },
  profileLifecycle: {
    status: "available" as const,
    head: { generation: 7, digest: `sha256:${"c".repeat(64)}` },
    activeProfileCount: 1,
    retiredProfileCount: 1,
  },
};

function arrangeGuardianResponses(
  statusResponse: unknown = status,
  diagnosticsResponse: unknown = diagnostics,
  runsResponse: unknown = runs,
) {
  mocks.invoke.mockImplementation((command) => {
    if (command === "read_config_file") return Promise.resolve({});
    if (command === "get_launch_at_login") return Promise.resolve(false);
    if (command === "list_audio_outputs") return Promise.resolve([]);
    if (command === "guardian_get_status") {
      return statusResponse instanceof Error
        ? Promise.reject(statusResponse.message)
        : Promise.resolve(statusResponse);
    }
    if (command === "guardian_get_diagnostics") {
      return diagnosticsResponse instanceof Error
        ? Promise.reject(diagnosticsResponse.message)
        : Promise.resolve(diagnosticsResponse);
    }
    if (command === "guardian_get_runs") {
      return runsResponse instanceof Error
        ? Promise.reject(runsResponse.message)
        : Promise.resolve(runsResponse);
    }
    if (command === "guardian_get_monitoring_policy") return Promise.resolve(monitoringPolicy);
    if (command === "guardian_get_security_status") return Promise.resolve(securityStatus);
    if (command === "guardian_update_monitoring_policy") {
      return Promise.resolve({ ...monitoringPolicy, revision: monitoringPolicy.revision + 1 });
    }
    if (command === "guardian_start_scan") {
      return Promise.resolve({
        authorizationId: "auth.imported-1",
        status: "running",
        startedAt: "2026-08-08T09:36:00Z",
      });
    }
    if (command === "guardian_cancel_run") {
      return Promise.resolve({ authorizationId: "auth.running-1", status: "cancelled" });
    }
    if (command === "guardian_request_notification_permission") return Promise.resolve(true);
    if (command === "save_config_file") return Promise.resolve();
    return Promise.reject(`Unexpected command: ${command}`);
  });
}

async function openGuardian() {
  render(<SettingsWindow />);
  await act(async () => {});
  mocks.invoke.mockClear();
  fireEvent.click(screen.getByRole("button", { name: "Guardian" }));
  await screen.findByRole("heading", { name: "Latest observation" });
}

beforeEach(() => {
  mocks.invoke.mockReset();
  mocks.listeners.clear();
  window.history.replaceState({}, "", "/");
  arrangeGuardianResponses();
});

describe("SettingsWindow Disk Guardian", () => {
  it("shows independently loaded Guardian-reported security facts without mutation controls", async () => {
    await openGuardian();

    const heading = screen.getByRole("heading", { name: "Security status" });
    const section = heading.closest("section");
    expect(section).toBeTruthy();
    expect(mocks.invoke).toHaveBeenCalledWith("guardian_get_security_status");
    expect(screen.getByText("Guardian-reported local state")).toBeTruthy();
    expect(screen.getByText("Personal Contracts issuer trusted by Guardian")).toBeTruthy();
    expect(screen.getByText("Guardian executor identity")).toBeTruthy();
    expect(screen.getByText("Accepted profile lifecycle")).toBeTruthy();
    expect(screen.getByText("Trust generation").nextElementSibling?.textContent).toBe("4");
    expect(screen.getByText("Head generation").nextElementSibling?.textContent).toBe("7");
    expect(screen.getByText("1 active · 1 retired")).toBeTruthy();
    expect(screen.getByText(/do not show whether Personal Contracts has pinned Guardian/i)).toBeTruthy();
    expect(screen.getByText(/Open Personal Contracts to verify its Guardian trust and synchronization state/i)).toBeTruthy();
    expect(section?.querySelectorAll("button")).toHaveLength(0);
    expect(section?.textContent).not.toMatch(/private key|pem|journal cursor:\s*\d/i);
  });

  it("keeps security status failure independent from disk observations", async () => {
    arrangeGuardianResponses();
    mocks.invoke.mockImplementation((command) => {
      if (command === "read_config_file") return Promise.resolve({});
      if (command === "get_launch_at_login") return Promise.resolve(false);
      if (command === "list_audio_outputs") return Promise.resolve([]);
      if (command === "guardian_get_status") return Promise.resolve(status);
      if (command === "guardian_get_diagnostics") return Promise.resolve(diagnostics);
      if (command === "guardian_get_runs") return Promise.resolve(runs);
      if (command === "guardian_get_monitoring_policy") return Promise.resolve(monitoringPolicy);
      if (command === "guardian_get_security_status") return Promise.reject("Security status unavailable");
      return Promise.reject(`Unexpected command: ${command}`);
    });

    await openGuardian();

    expect(screen.getByRole("heading", { name: "Security status unavailable" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Latest observation" })).toBeTruthy();
    expect(screen.getByText("125 GB")).toBeTruthy();
  });

  it("renders disk observations while an independent security request is still pending", async () => {
    let resolveSecurity!: (value: typeof securityStatus) => void;
    const delayedSecurity = new Promise<typeof securityStatus>((resolve) => {
      resolveSecurity = resolve;
    });
    mocks.invoke.mockImplementation((command) => {
      if (command === "read_config_file") return Promise.resolve({});
      if (command === "get_launch_at_login") return Promise.resolve(false);
      if (command === "list_audio_outputs") return Promise.resolve([]);
      if (command === "guardian_get_status") return Promise.resolve(status);
      if (command === "guardian_get_diagnostics") return Promise.resolve(diagnostics);
      if (command === "guardian_get_runs") return Promise.resolve(runs);
      if (command === "guardian_get_monitoring_policy") return Promise.resolve(monitoringPolicy);
      if (command === "guardian_get_security_status") return delayedSecurity;
      return Promise.reject(`Unexpected command: ${command}`);
    });

    render(<SettingsWindow />);
    await act(async () => {});
    fireEvent.click(screen.getByRole("button", { name: "Guardian" }));

    expect(await screen.findByRole("heading", { name: "Latest observation" })).toBeTruthy();
    expect(screen.getByText("125 GB")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Security status" })).toBeTruthy();
    expect(screen.getByText("Loading Guardian-reported security state…")).toBeTruthy();

    await act(async () => resolveSecurity(securityStatus));
    expect(await screen.findByText("Guardian-reported local state")).toBeTruthy();
  });

  it("opens directly on Guardian when the validated window URL requests it", async () => {
    window.history.replaceState({}, "", "/?tab=guardian");

    render(<SettingsWindow />);

    expect(await screen.findByRole("heading", { name: "Latest observation" })).toBeTruthy();
    expect(mocks.invoke).toHaveBeenCalledWith("guardian_get_status");
    expect(screen.getByRole("button", { name: "Guardian" }).className).toContain("sw__tab--active");
  });

  it("selects Guardian in an existing window and ignores invalid tab events", async () => {
    render(<SettingsWindow />);
    await act(async () => {});

    const selectTab = mocks.listeners.get("settings:select-tab");
    expect(selectTab).toBeTypeOf("function");

    act(() => selectTab?.({ payload: "cleanup" }));
    expect(screen.queryByRole("heading", { name: "Latest observation" })).toBeNull();

    act(() => selectTab?.({ payload: "guardian" }));
    expect(await screen.findByRole("heading", { name: "Latest observation" })).toBeTruthy();
  });

  it("loads healthy status, diagnostics, and runs only after activation", async () => {
    render(<SettingsWindow />);
    await act(async () => {});

    expect(mocks.invoke).not.toHaveBeenCalledWith("guardian_get_status");
    expect(mocks.invoke).not.toHaveBeenCalledWith("guardian_get_diagnostics");
    expect(mocks.invoke).not.toHaveBeenCalledWith("guardian_get_runs");
    expect(mocks.invoke).not.toHaveBeenCalledWith("guardian_get_monitoring_policy");
    expect(mocks.invoke).not.toHaveBeenCalledWith("guardian_get_security_status");
    mocks.invoke.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Guardian" }));

    expect(await screen.findByText("healthy")).toBeTruthy();
    expect(screen.getByText("125 GB")).toBeTruthy();
    expect(screen.getByText("of 500 GB total")).toBeTruthy();
    expect(screen.getByText("25%")).toBeTruthy();
    expect(screen.getByText("No pressure reasons reported.")).toBeTruthy();
    expect(screen.getByText("15 GB")).toBeTruthy();
    expect(screen.getByText("42")).toBeTruthy();
    expect(screen.getByText("7 folders")).toBeTruthy();
    expect(screen.getByText("Complete")).toBeTruthy();
    expect(screen.getByText("None")).toBeTruthy();
    expect(screen.getByText("No earlier observation is available for comparison.")).toBeTruthy();
    expect(mocks.invoke.mock.calls).toEqual([
      ["guardian_get_status"],
      ["guardian_get_diagnostics"],
      ["guardian_get_runs"],
      ["guardian_get_monitoring_policy"],
      ["guardian_get_security_status"],
    ]);
  });

  it("shows critical severity, reasons, trend, partial limits, errors, and growth", async () => {
    arrangeGuardianResponses(
      {
        ...status,
        evaluation: {
          ...status.evaluation,
          severity: "critical",
          reasons: ["Available space is below the critical threshold."],
          availableBasisPoints: 175,
          trend: {
            windowMs: 86_400_000,
            dropBytes: "3000000000",
            timeToCriticalMs: "172800000",
          },
        },
      },
      {
        ...diagnostics,
        report: {
          ...diagnostics.report,
          partial: true,
          limitsHit: ["max_entries", "time_budget"],
          errorCount: 3,
        },
        growth: {
          schemaVersion: 1,
          previousObservedAt: "2026-08-07T09:29:55Z",
          currentObservedAt: "2026-08-08T09:29:55Z",
          totals: { filesDelta: 12, bytesDelta: "2500000000" },
          candidates: [
            {
              scope: "user-caches",
              relativePath: "Caches/[redacted]/build-output",
              previousBytes: "10000000000",
              currentBytes: "12500000000",
              deltaBytes: "2500000000",
            },
          ],
        },
      },
    );

    await openGuardian();

    expect(screen.getByText("critical")).toBeTruthy();
    expect(screen.getByText("Available space is below the critical threshold.")).toBeTruthy();
    expect(screen.getByText("1.75%")).toBeTruthy();
    expect(screen.getByText("Recent drop")).toBeTruthy();
    expect(screen.getByText("Partial")).toBeTruthy();
    expect(screen.getByText("max_entries, time_budget")).toBeTruthy();
    expect(screen.getByText("Some entries could not be read")).toBeTruthy();
    expect(screen.getAllByText("2.5 GB").length).toBeGreaterThanOrEqual(1);
  });

  it("keeps command failures nonfatal and reports both unavailable sections", async () => {
    arrangeGuardianResponses(
      new Error("Disk Guardian is not available on this system"),
      new Error("Diagnostics permission denied"),
    );

    render(<SettingsWindow />);
    await act(async () => {});
    fireEvent.click(screen.getByRole("button", { name: "Guardian" }));

    expect(await screen.findByRole("heading", { name: "Disk status unavailable" })).toBeTruthy();
    expect(screen.getByText("Disk Guardian is not available on this system")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Diagnostics unavailable" })).toBeTruthy();
    expect(screen.getByText("Diagnostics permission denied")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Refresh" })).toBeTruthy();
  });

  it("manually refreshes status, diagnostics, and runs together", async () => {
    await openGuardian();
    mocks.invoke.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));

    await waitFor(() => expect(screen.getByRole("button", { name: "Refresh" })).toBeTruthy());
    expect(mocks.invoke.mock.calls).toEqual([
      ["guardian_get_status"],
      ["guardian_get_diagnostics"],
      ["guardian_get_runs"],
      ["guardian_get_monitoring_policy"],
      ["guardian_get_security_status"],
    ]);
  });

  it("formats decimal byte strings above Number.MAX_SAFE_INTEGER with BigInt", () => {
    expect(formatGuardianBytes("9007199254740993")).toBe("9.01 PB");
    expect(formatGuardianBytes("-9007199254740993")).toBe("−9.01 PB");
    expect(formatGuardianBytes("999")).toBe("999 B");
  });

  it("renders backend-redacted top candidates without exposing action controls", async () => {
    await openGuardian();

    expect(screen.getAllByText("Caches/[redacted]/build-output")).toHaveLength(2);
    expect(screen.getAllByText("Relative paths only")).toHaveLength(2);
    expect(screen.getAllByText("folder")).toHaveLength(2);
    expect(screen.getAllByText("12.5 GB").length).toBeGreaterThanOrEqual(2);
    expect(screen.queryByRole("textbox", { name: /path|target|interval|retention|trend|cleanup/i })).toBeNull();
    expect(screen.queryByRole("button", { name: /approve|clean|delete|confirm/i })).toBeNull();
  });

  it("renders immutable preview evidence with bounded relative-path candidates", async () => {
    arrangeGuardianResponses(status, {
      ...diagnostics,
      generatedAt: "enclosing diagnostics timestamp",
      preview: {
        ...diagnostics.preview,
        generatedFrom: "source observation timestamp",
      },
    });

    await openGuardian();

    expect(screen.getByRole("heading", { name: "Diagnostic preview" })).toBeTruthy();
    expect(screen.getByText("This immutable diagnostic evidence is not approval and does not perform cleanup.")).toBeTruthy();
    expect(screen.getByText("Generated from")).toBeTruthy();
    expect(screen.getByText("source observation timestamp")).toBeTruthy();
    expect(screen.queryByText("enclosing diagnostics timestamp")).toBeNull();
    expect(screen.getByText("Canonical preview reference")).toBeTruthy();
    expect(screen.getByTitle("opaque-preview-ref")).toBeTruthy();
    expect(screen.getByText("Estimated bytes")).toBeTruthy();
    expect(screen.getAllByText("12.5 GB").length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText("Caches/[redacted]/build-output").length).toBeGreaterThanOrEqual(2);
    expect(screen.queryByRole("link")).toBeNull();
    expect(screen.queryByRole("button", { name: /approve|clean|delete|confirm|import/i })).toBeNull();
  });

  it("keeps an empty preview as read-only evidence without controls", async () => {
    arrangeGuardianResponses(status, {
      ...diagnostics,
      preview: { ...diagnostics.preview, candidates: [] },
    });

    await openGuardian();

    expect(screen.getByText("This diagnostic preview contains no safe relative candidates.")).toBeTruthy();
    expect(screen.queryByRole("link")).toBeNull();
    expect(screen.queryByRole("button", { name: /approve|clean|delete|confirm|import/i })).toBeNull();
  });

  it("caps preview evidence at ten candidates", async () => {
    const previewCandidates = Array.from({ length: 12 }, (_, index) => ({
      ...candidate,
      relativePath: `Caches/[redacted]/candidate-${index + 1}`,
    }));
    arrangeGuardianResponses(status, {
      ...diagnostics,
      preview: { ...diagnostics.preview, candidates: previewCandidates },
    });

    await openGuardian();

    expect(screen.getByText("Caches/[redacted]/candidate-10")).toBeTruthy();
    expect(screen.queryByText("Caches/[redacted]/candidate-11")).toBeNull();
    expect(screen.queryByText("Caches/[redacted]/candidate-12")).toBeNull();
  });

  it("omits malformed or absolute preview paths", async () => {
    arrangeGuardianResponses(status, {
      ...diagnostics,
      preview: {
        ...diagnostics.preview,
        candidates: [
          candidate,
          { ...candidate, relativePath: "/Users/example/private-cache" },
          { ...candidate, relativePath: "../outside-scope" },
        ],
      },
    });

    await openGuardian();

    expect(screen.getAllByText("Caches/[redacted]/build-output")).toHaveLength(2);
    expect(screen.queryByText("/Users/example/private-cache")).toBeNull();
    expect(screen.queryByText("../outside-scope")).toBeNull();
  });

  it("renders imported and recorded runs with actions limited by backend status", async () => {
    await openGuardian();

    expect(screen.getByRole("heading", { name: "Run journal" })).toBeTruthy();
    expect(screen.getAllByText("bounded-cache-scan")).toHaveLength(2);
    expect(screen.getByText("downloads-observation")).toBeTruthy();
    expect(screen.getByText("developer-builds")).toBeTruthy();
    expect(screen.getByText("auth.expired-1")).toBeTruthy();
    expect(screen.getByText("Authorization expired before it could start.")).toBeTruthy();
    expect(screen.getByText("Observed expired")).toBeTruthy();
    expect(screen.getByText("auth.imported-1")).toBeTruthy();
    expect(screen.getByText(/Delivered to Guardian and signature-verified/i)).toBeTruthy();
    expect(screen.getByText(/approve new scan authorizations in Personal Contracts/i)).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "Start scan" })).toHaveLength(1);
    expect(screen.getAllByRole("button", { name: "Cancel" })).toHaveLength(1);
    const expired = screen.getByText("auth.expired-1").closest("li");
    expect(expired?.querySelector("button")).toBeNull();
  });

  it("starts only the imported authorization surfaced by Guardian", async () => {
    await openGuardian();
    mocks.invoke.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Start scan" }));

    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith(
      "guardian_start_scan",
      { authorizationId: "auth.imported-1" },
    ));
  });

  it("suppresses a duplicate start click while the first explicit action is in flight", async () => {
    await openGuardian();
    mocks.invoke.mockClear();
    let finish!: (value: unknown) => void;
    mocks.invoke.mockImplementation((command) => {
      if (command === "guardian_start_scan") {
        return new Promise((resolve) => { finish = resolve; });
      }
      return Promise.reject(`Unexpected command while start is pending: ${command}`);
    });

    const start = screen.getByRole("button", { name: "Start scan" });
    fireEvent.click(start);
    fireEvent.click(start);

    expect(mocks.invoke.mock.calls.filter(([command]) =>
      command === "guardian_start_scan"
    )).toHaveLength(1);
    await act(async () => {
      finish({ authorizationId: "auth.imported-1", status: "running" });
    });
  });

  it("cancels a running authorization then refreshes the journal", async () => {
    await openGuardian();
    mocks.invoke.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith(
      "guardian_cancel_run",
      { authorizationId: "auth.running-1" },
    ));
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith("guardian_get_runs"));
  });

  it("keeps a runs failure independent from healthy status and diagnostics", async () => {
    arrangeGuardianResponses(status, diagnostics, new Error("Run journal denied"));

    await openGuardian();

    expect(screen.getByText("healthy")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Bounded scan summary" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Run journal unavailable" })).toBeTruthy();
    expect(screen.getByText("Run journal denied")).toBeTruthy();
  });

  it("does not expose import, cleanup, authorization creation, or raw input controls", async () => {
    await openGuardian();

    expect(screen.queryByRole("textbox", { name: /path|target|interval|retention|trend|cleanup|raw/i })).toBeNull();
    expect(screen.queryByRole("button", { name: /import|cleanup|clean|delete|approve|create/i })).toBeNull();
    expect(screen.queryByText(/raw json|receipt signature|filesystem path|command/i)).toBeNull();
  });

  it("loads the four Guardian-owned threshold scalars and no operational settings", async () => {
    await openGuardian();

    expect(screen.getByText(/Guardian owns and enforces this monitoring policy/i)).toBeTruthy();
    expect((screen.getByRole("textbox", { name: "Warning bytes" }) as HTMLInputElement).value).toBe("53687091200");
    expect((screen.getByRole("textbox", { name: "Critical bytes" }) as HTMLInputElement).value).toBe("21474836480");
    expect((screen.getByRole("textbox", { name: "Warning basis points" }) as HTMLInputElement).value).toBe("1000");
    expect((screen.getByRole("textbox", { name: "Critical basis points" }) as HTMLInputElement).value).toBe("400");
    expect(screen.queryByRole("textbox", { name: /path|target|interval|retention|trend|cleanup/i })).toBeNull();
  });

  it("validates canonical decimals, bounded basis points, and critical <= warning before saving", async () => {
    await openGuardian();
    mocks.invoke.mockClear();

    const warningBytes = screen.getByRole("textbox", { name: "Warning bytes" });
    fireEvent.change(warningBytes, { target: { value: "01" } });
    fireEvent.click(screen.getByRole("button", { name: "Save thresholds" }));
    expect(await screen.findByText(/canonical decimal/i)).toBeTruthy();
    expect(mocks.invoke).not.toHaveBeenCalledWith("guardian_update_monitoring_policy", expect.anything());

    fireEvent.change(warningBytes, { target: { value: "100" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Critical bytes" }), {
      target: { value: "101" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save thresholds" }));
    expect(await screen.findByText(/critical.*no greater than.*warning/i)).toBeTruthy();

    fireEvent.change(screen.getByRole("textbox", { name: "Critical bytes" }), {
      target: { value: "50" },
    });
    fireEvent.change(screen.getByRole("textbox", { name: "Warning basis points" }), {
      target: { value: "10001" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save thresholds" }));
    expect(await screen.findByText(/basis points.*0.*10000/i)).toBeTruthy();
  });

  it("saves thresholds with the loaded expected revision", async () => {
    await openGuardian();
    mocks.invoke.mockClear();

    fireEvent.change(screen.getByRole("textbox", { name: "Warning bytes" }), {
      target: { value: "60000000000" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save thresholds" }));

    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith(
      "guardian_update_monitoring_policy",
      {
        expectedRevision: 7,
        policy: {
          warningBytes: "60000000000",
          criticalBytes: "21474836480",
          warningBasisPoints: 1000,
          criticalBasisPoints: 400,
        },
      },
    ));
  });

  it("surfaces a CAS conflict without claiming the stale edit was saved", async () => {
    arrangeGuardianResponses();
    mocks.invoke.mockImplementation((command) => {
      if (command === "guardian_get_status") return Promise.resolve(status);
      if (command === "guardian_get_diagnostics") return Promise.resolve(diagnostics);
      if (command === "guardian_get_runs") return Promise.resolve(runs);
      if (command === "guardian_get_monitoring_policy") return Promise.resolve(monitoringPolicy);
      if (command === "guardian_update_monitoring_policy") {
        return Promise.reject("Monitoring policy revision conflict: expected 7, current 8");
      }
      if (command === "read_config_file") return Promise.resolve({});
      if (command === "get_launch_at_login") return Promise.resolve(false);
      if (command === "list_audio_outputs") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    await openGuardian();

    fireEvent.change(screen.getByRole("textbox", { name: "Warning bytes" }), {
      target: { value: "60000000000" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save thresholds" }));

    expect(await screen.findByText(/revision conflict.*reload.*try again/i)).toBeTruthy();
    expect((screen.getByRole("textbox", { name: "Warning bytes" }) as HTMLInputElement).value).toBe("60000000000");
  });

  it("enables transition notifications only through explicit permission and config commands", async () => {
    await openGuardian();
    mocks.invoke.mockClear();

    fireEvent.click(screen.getByRole("checkbox", {
      name: "Notify me when disk status changes to warning or critical",
    }));

    await waitFor(() => expect(mocks.invoke.mock.calls).toEqual([
      ["guardian_request_notification_permission"],
      ["save_config_file", { config: { guardian: { notifications_enabled: true } } }],
    ]));
  });

  it("keeps the persisted notification value and reports a failed config save", async () => {
    await openGuardian();
    mocks.invoke.mockClear();
    mocks.invoke.mockImplementation((command) => {
      if (command === "guardian_request_notification_permission") return Promise.resolve(true);
      if (command === "save_config_file") return Promise.reject("Config storage is unavailable");
      return Promise.reject(`Unexpected command: ${command}`);
    });

    const toggle = screen.getByRole("checkbox", {
      name: "Notify me when disk status changes to warning or critical",
    });
    fireEvent.click(toggle);

    expect(await screen.findByText(/could not save.*config storage is unavailable/i)).toBeTruthy();
    expect((toggle as HTMLInputElement).checked).toBe(false);
  });

  it("prevents a second notification change while the first save is pending", async () => {
    let finishSave: (() => void) | undefined;
    const savePending = new Promise<void>((resolve) => {
      finishSave = resolve;
    });
    await openGuardian();
    mocks.invoke.mockClear();
    mocks.invoke.mockImplementation((command) => {
      if (command === "guardian_request_notification_permission") return Promise.resolve(true);
      if (command === "save_config_file") return savePending;
      return Promise.reject(`Unexpected command: ${command}`);
    });

    const toggle = screen.getByRole("checkbox", {
      name: "Notify me when disk status changes to warning or critical",
    });
    fireEvent.click(toggle);

    await waitFor(() => expect((toggle as HTMLInputElement).disabled).toBe(true));
    fireEvent.click(toggle);
    expect(mocks.invoke.mock.calls).toEqual([
      ["guardian_request_notification_permission"],
      ["save_config_file", { config: { guardian: { notifications_enabled: true } } }],
    ]);

    finishSave?.();
    await waitFor(() => expect((toggle as HTMLInputElement).disabled).toBe(false));
  });
});
