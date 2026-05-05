import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useHealthCheckStore } from "@/stores/healthCheck";

vi.mock("@/services/tauri", () => ({
  tauriApi: {
    runHealthCheck: vi.fn(async () => ({ scan_run_id: 9, healthy: 70, warning: 4, critical: 2, offline: 0, total: 76 })),
    listHealthResultsForRun: vi.fn(async () => ([
      { scan_run_id: 9, machine_id: 11, machine_results: {
          smb: { status: "healthy", message: "ok", sample: "Running" },
          system_write: { status: "critical", message: "fail", sample: "" },
      } },
    ])),
  },
}));

describe("useHealthCheckStore", () => {
  beforeEach(() => { setActivePinia(createPinia()); });

  it("runs and stores rows by machine", async () => {
    const s = useHealthCheckStore();
    await s.run([11], {}, "UECM:winrm:LANPC");
    expect(s.scanRunId).toBe(9);
    expect(s.rowsByMachine[11].smb.status).toBe("healthy");
    expect(s.rowsByMachine[11].system_write.status).toBe("critical");
  });

  it("computes per-check totals", async () => {
    const s = useHealthCheckStore();
    await s.run([11], {}, "UECM:winrm:LANPC");
    expect(s.summary.critical).toBe(2);
    expect(s.summary.healthy).toBe(70);
  });
});
