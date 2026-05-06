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

  it("scanInis wraps request", async () => {
    mockInvoke.mockResolvedValue({ scan_run_id: 9, summary: {}, findings: [] });
    await tauriApi.scanInis({ machine_ids: [1], credential_alias: "cred", project_paths: [], user_profile_path: null });
    expect(mockInvoke).toHaveBeenCalledWith("scan_inis", {
      request: { machine_ids: [1], credential_alias: "cred", project_paths: [], user_profile_path: null },
    });
  });

  it("runHealthCheck wraps request", async () => {
    mockInvoke.mockResolvedValue({ scan_run_id: 10, results: [] });
    await tauriApi.runHealthCheck({ machine_ids: [1], credential_alias: "cred", project_paths: ["E:\\Proj"] });
    expect(mockInvoke).toHaveBeenCalledWith("run_health_check", {
      request: { machine_ids: [1], credential_alias: "cred", project_paths: ["E:\\Proj"] },
    });
  });

  it("startPsoCollection passes collection args", async () => {
    mockInvoke.mockResolvedValue({ job_id: "job-1", source_machine_id: 1, project_id: 7 });
    await tauriApi.startPsoCollection({
      sourceMachineId: 1,
      projectId: 7,
      ueVersion: "5.4",
      resolutionW: 1280,
      resolutionH: 720,
      windowed: true,
      maxMinutes: 10,
      operatorCredentialAlias: "winrm",
    });
    expect(mockInvoke).toHaveBeenCalledWith("start_pso_collection", {
      sourceMachineId: 1,
      projectId: 7,
      ueVersion: "5.4",
      resolutionW: 1280,
      resolutionH: 720,
      windowed: true,
      maxMinutes: 10,
      operatorCredentialAlias: "winrm",
    });
  });

  it("distributePsoCache wraps backend request shape", async () => {
    mockInvoke.mockResolvedValue({ job_id: "dist-1", plan: [] });
    await tauriApi.distributePsoCache({
      fileId: 5,
      targetMachineIds: [2, 3],
      namedShareUnc: "\\\\SOURCE\\PSO",
      operatorCredentialAlias: "winrm",
      sourceSmbCredentialAlias: "share",
      forceGpuMismatch: false,
    });
    expect(mockInvoke).toHaveBeenCalledWith("distribute_pso_cache", {
      request: {
        file_id: 5,
        target_machine_ids: [2, 3],
        named_share_unc: "\\\\SOURCE\\PSO",
        operator_credential_alias: "winrm",
        source_smb_credential_alias: "share",
        force_gpu_mismatch: false,
      },
    });
  });

  it("listPsoCacheFiles passes optional filters", async () => {
    mockInvoke.mockResolvedValue([]);
    await tauriApi.listPsoCacheFiles(7, {
      sourceMachineId: 2,
      gpuSignature: " NVIDIA:RTX 3080 :535.98 ",
    });
    expect(mockInvoke).toHaveBeenCalledWith("list_pso_cache_files", {
      projectId: 7,
      sourceMachineId: 2,
      gpuSignature: " NVIDIA:RTX 3080 :535.98 ",
    });
  });

  it("verifyPsoPrecaching wraps scanner request", async () => {
    mockInvoke.mockResolvedValue({ scan_run_id: 11, summary: {}, findings: [] });
    await tauriApi.verifyPsoPrecaching({
      machine_ids: [4],
      credential_alias: "cred",
      project_paths: ["E:\\Proj"],
      user_profile_path: null,
    });
    expect(mockInvoke).toHaveBeenCalledWith("verify_pso_precaching", {
      request: {
        machine_ids: [4],
        credential_alias: "cred",
        project_paths: ["E:\\Proj"],
        user_profile_path: null,
      },
    });
  });

  it("getGpuConsistencyMatrix invokes matrix command", async () => {
    mockInvoke.mockResolvedValue({ signatures: [], baseline: null, cells: [] });
    await tauriApi.getGpuConsistencyMatrix();
    expect(mockInvoke).toHaveBeenCalledWith("get_gpu_consistency_matrix");
  });
});
