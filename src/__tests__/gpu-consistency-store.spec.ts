import { describe, it, expect, vi, beforeEach } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    getGpuConsistencyMatrix: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));

import { useGpuConsistencyStore } from "@/stores/gpuConsistency";

describe("gpuConsistency store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockApi.getGpuConsistencyMatrix.mockReset();
  });

  it("computes baseline label and deviation counts", async () => {
    mockApi.getGpuConsistencyMatrix.mockResolvedValue({
      signatures: [
        { signature: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" }, count: 2 },
        { signature: { vendor: "nvidia", model: "RTX 4090", driver: "560.00" }, count: 1 },
      ],
      baseline: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" },
      cells: [
        {
          machine_id: 1,
          hostname: "A",
          signature: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" },
          status: "match",
        },
        {
          machine_id: 2,
          hostname: "B",
          signature: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" },
          status: "match",
        },
        {
          machine_id: 3,
          hostname: "C",
          signature: { vendor: "nvidia", model: "RTX 4090", driver: "560.00" },
          status: "deviation",
        },
        { machine_id: 4, hostname: "D", signature: null, status: "unknown" },
      ],
    });
    const store = useGpuConsistencyStore();
    await store.load();
    expect(store.baselineLabel).toContain("RTX 3080");
    expect(store.deviationCount).toBe(1);
    expect(store.unknownCount).toBe(1);
  });
});
