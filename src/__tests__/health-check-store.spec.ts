import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useHealthCheckStore } from "@/stores/healthCheck";
import { PROBE_LAYER_MAP } from "@/services/tauri";

const { runHealthCheck, listHealthResultsForRun } = vi.hoisted(() => ({
  runHealthCheck: vi.fn(),
  listHealthResultsForRun: vi.fn(),
}));

vi.mock("@/services/tauri", async () => {
  const actual = await vi.importActual<typeof import("@/services/tauri")>("@/services/tauri");
  return {
    ...actual,
    tauriApi: { runHealthCheck, listHealthResultsForRun },
  };
});

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

describe("probesByLayer", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("exposes PROBE_LAYER_MAP from services/tauri", () => {
    expect(PROBE_LAYER_MAP.tcp_5985).toBe("l1_port");
  });

  it("groups outcomes by L1/L2/L3 for a single machine", () => {
    const store = useHealthCheckStore();
    store.results = [{
      scan_run_id: 1,
      machine_id: 42,
      machine_results: {
        tcp_5985: { status: "healthy", message: "", sample: "", remediation: "" },
        lanman_server: { status: "healthy", message: "", sample: "", remediation: "" },
        share_reachable: { status: "critical", message: "stopped", sample: "Stopped",
                           remediation: "Run uecm-cli share create --host <h>" },
      },
    }] as any;

    const grouped = store.probesByLayer[42];
    expect(grouped.l1_port).toHaveLength(1);
    expect(grouped.l1_port[0].key).toBe("tcp_5985");
    expect(grouped.l2_bootstrap).toHaveLength(1);
    // rowsByMachine always injects derived pso_precaching + gpu_consistency,
    // both mapped to l3_business — so length is 3, not 1.
    expect(grouped.l3_business).toHaveLength(3);
    const l3Keys = grouped.l3_business.map((p) => p.key).sort();
    expect(l3Keys).toEqual(["gpu_consistency", "pso_precaching", "share_reachable"]);
    const shareReachable = grouped.l3_business.find((p) => p.key === "share_reachable")!;
    expect(shareReachable.outcome.remediation).toContain("share create");
  });

  it("warns when an unknown probe key appears", () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const store = useHealthCheckStore();
    store.results = [{
      scan_run_id: 1,
      machine_id: 42,
      machine_results: {
        unknown_probe_xyz: { status: "warning", message: "?", sample: "", remediation: "" },
      },
    }] as any;
    // Access probesByLayer to trigger the computed
    const _ = store.probesByLayer;
    expect(warnSpy).toHaveBeenCalledWith(
      expect.stringContaining("unknown_probe_xyz")
    );
    warnSpy.mockRestore();
  });
});
