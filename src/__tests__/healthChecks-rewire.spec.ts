import { describe, it, expect, vi, beforeEach } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    runHealthCheck: vi.fn(),
    listHealthResultsForRun: vi.fn(),
    getGpuConsistencyMatrix: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));

import { useDiagnosticsStore } from "@/stores/diagnostics";
import { useGpuConsistencyStore } from "@/stores/gpuConsistency";
import { useHealthCheckStore } from "@/stores/healthCheck";

describe("Health Check rewire", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((fn) => fn.mockReset());
  });

  it("derives gpu_consistency from gpuConsistency store", async () => {
    mockApi.getGpuConsistencyMatrix.mockResolvedValue({
      signatures: [{ signature: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" }, count: 1 }],
      baseline: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" },
      cells: [
        {
          machine_id: 1,
          hostname: "A",
          signature: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" },
          status: "match",
        },
        { machine_id: 2, hostname: "B", signature: null, status: "unknown" },
      ],
    });
    mockApi.runHealthCheck.mockResolvedValue({
      scan_run_id: 7,
      results: [
        { id: 1, scan_run_id: 7, machine_id: 1, machine_results: {} },
        { id: 2, scan_run_id: 7, machine_id: 2, machine_results: {} },
      ],
    });
    const gpu = useGpuConsistencyStore();
    const health = useHealthCheckStore();
    await gpu.load();
    await health.run({ machine_ids: [1, 2], credential_alias: "cred", project_paths: [] });
    expect(health.rowsByMachine[1].gpu_consistency.status).toBe("healthy");
    expect(health.rowsByMachine[2].gpu_consistency.status).toBe("unknown");
  });

  it("derives pso_precaching from open R008-R010 diagnostics findings", async () => {
    mockApi.runHealthCheck.mockResolvedValue({
      scan_run_id: 8,
      results: [{ id: 1, scan_run_id: 8, machine_id: 1, machine_results: {} }],
    });
    const diagnostics = useDiagnosticsStore();
    diagnostics.scanRunId = 99;
    diagnostics.findings = [
      {
        id: 1,
        scan_run_id: 99,
        machine_id: 1,
        rule_id: "R008",
        severity: "critical",
        category: "PSO",
        file_path: "D:\\Proj\\Config\\ConsoleVariables.ini",
        section: "ConsoleVariables",
        key_name: "r.PSOPrecaching",
        line_number: null,
        snippet_before: "",
        snippet_after: null,
        recommended_action: "set",
        recommended_value: "1",
        symptom: "",
        rationale: "",
        fixed_at: null,
        skipped_at: null,
      },
    ];
    const health = useHealthCheckStore();
    await health.run({ machine_ids: [1], credential_alias: "cred", project_paths: [] });
    expect(health.rowsByMachine[1].pso_precaching.status).toBe("critical");
    expect(health.rowsByMachine[1].pso_precaching.message).toContain("R008");
  });
});
