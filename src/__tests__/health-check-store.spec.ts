import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useHealthCheckStore } from "@/stores/healthCheck";

const { runHealthCheck, listHealthResultsForRun } = vi.hoisted(() => ({
  runHealthCheck: vi.fn(),
  listHealthResultsForRun: vi.fn(),
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: { runHealthCheck, listHealthResultsForRun },
}));

describe("health check store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("indexes results by machine", async () => {
    runHealthCheck.mockResolvedValue({ scan_run_id: 7, results: [{ id: 1, scan_run_id: 7, machine_id: 2, machine_results: { smb: { status: "healthy", message: "ok", sample: "", remediation: "" } } }] });
    const store = useHealthCheckStore();
    await store.run({ machine_ids: [2], credential_alias: "cred", project_paths: [] });
    expect(store.scanRunId).toBe(7);
    expect(store.rowsByMachine[2].smb.status).toBe("healthy");
    expect(store.summary.healthy).toBe(1);
  });
});
