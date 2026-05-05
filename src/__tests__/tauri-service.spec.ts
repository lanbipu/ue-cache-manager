import { describe, it, expect, vi, beforeEach } from "vitest";

// vi.hoisted ensures mockInvoke is initialized before vi.mock factory runs
const { mockInvoke } = vi.hoisted(() => ({ mockInvoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: mockInvoke,
}));

import { tauriApi, type Machine } from "@/services/tauri";

describe("tauri service", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("listMachines invokes the correct command", async () => {
    const fake: Machine[] = [
      {
        id: 1,
        hostname: "RENDER-01",
        ip: "192.168.10.21",
        role: "render",
        status: "online",
        last_seen_at: null,
      },
    ];
    mockInvoke.mockResolvedValue(fake);
    const result = await tauriApi.listMachines();
    expect(mockInvoke).toHaveBeenCalledWith("list_machines");
    expect(result).toEqual(fake);
  });

  it("addMachine passes hostname and ip", async () => {
    mockInvoke.mockResolvedValue(42);
    const id = await tauriApi.addMachine("RENDER-02", "192.168.10.22");
    expect(mockInvoke).toHaveBeenCalledWith("add_machine", {
      hostname: "RENDER-02",
      ip: "192.168.10.22",
    });
    expect(id).toBe(42);
  });

  it("deleteMachine passes id", async () => {
    mockInvoke.mockResolvedValue(undefined);
    await tauriApi.deleteMachine(7);
    expect(mockInvoke).toHaveBeenCalledWith("delete_machine", { id: 7 });
  });

  it("testPowerShellBridge passes message", async () => {
    const fake = { received: "hi", timestamp: "2026-01-01", machine: "PC" };
    mockInvoke.mockResolvedValue(fake);
    const result = await tauriApi.testPowerShellBridge("hi");
    expect(mockInvoke).toHaveBeenCalledWith("test_powershell_bridge", { message: "hi" });
    expect(result).toEqual(fake);
  });

  // Diagnostics — INI scanner
  it("scanInis passes all required args", async () => {
    const fake = { scan_run_id: 3, critical: 1, warning: 2, healthy: 4 };
    mockInvoke.mockResolvedValue(fake);
    const result = await tauriApi.scanInis([11], { 11: ["C:\\proj"] }, "C:\\Users\\X", "UECM:winrm:PC");
    expect(mockInvoke).toHaveBeenCalledWith("scan_inis", {
      machineIds: [11],
      projectPathsPerMachine: { 11: ["C:\\proj"] },
      userProfile: "C:\\Users\\X",
      credentialAlias: "UECM:winrm:PC",
    });
    expect(result).toEqual(fake);
  });

  it("listFindingsForRun passes scanRunId", async () => {
    mockInvoke.mockResolvedValue([]);
    await tauriApi.listFindingsForRun(3);
    expect(mockInvoke).toHaveBeenCalledWith("list_findings_for_run", { scanRunId: 3 });
  });

  it("listRecentIniRuns passes limit", async () => {
    mockInvoke.mockResolvedValue([]);
    await tauriApi.listRecentIniRuns(10);
    expect(mockInvoke).toHaveBeenCalledWith("list_recent_ini_runs", { limit: 10 });
  });

  it("applyFinding passes findingId and credentialAlias", async () => {
    mockInvoke.mockResolvedValue("C:\\f.ini.bak");
    const result = await tauriApi.applyFinding(42, "UECM:winrm:PC");
    expect(mockInvoke).toHaveBeenCalledWith("apply_finding", { findingId: 42, credentialAlias: "UECM:winrm:PC" });
    expect(result).toBe("C:\\f.ini.bak");
  });

  it("skipFinding passes findingId", async () => {
    mockInvoke.mockResolvedValue(undefined);
    await tauriApi.skipFinding(42);
    expect(mockInvoke).toHaveBeenCalledWith("skip_finding", { findingId: 42 });
  });

  // Diagnostics — Health check
  it("runHealthCheck passes all required args", async () => {
    const fake = { scan_run_id: 9, healthy: 70, warning: 4, critical: 2, offline: 0, total: 76 };
    mockInvoke.mockResolvedValue(fake);
    const result = await tauriApi.runHealthCheck([11], { 11: ["C:\\proj"] }, "UECM:winrm:PC");
    expect(mockInvoke).toHaveBeenCalledWith("run_health_check", {
      machineIds: [11],
      projectPathsPerMachine: { 11: ["C:\\proj"] },
      credentialAlias: "UECM:winrm:PC",
    });
    expect(result).toEqual(fake);
  });

  it("listRecentHealthRuns passes limit", async () => {
    mockInvoke.mockResolvedValue([]);
    await tauriApi.listRecentHealthRuns(5);
    expect(mockInvoke).toHaveBeenCalledWith("list_recent_health_runs", { limit: 5 });
  });

  it("listHealthResultsForRun passes scanRunId", async () => {
    mockInvoke.mockResolvedValue([]);
    await tauriApi.listHealthResultsForRun(9);
    expect(mockInvoke).toHaveBeenCalledWith("list_health_results_for_run", { scanRunId: 9 });
  });
});
