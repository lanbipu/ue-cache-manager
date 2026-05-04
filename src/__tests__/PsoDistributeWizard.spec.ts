import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import PsoDistributeWizard from "@/components/modals/PsoDistributeWizard.vue";

const { mockApi, listenMock } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn(),
    listCredentials: vi.fn(),
    getGpuConsistencyMatrix: vi.fn(),
    distributePsoCache: vi.fn(),
    startPsoCollection: vi.fn(),
    listPsoCacheFiles: vi.fn(),
    cancelUeJob: vi.fn(),
  },
  listenMock: vi.fn(async () => () => {}),
}));

vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

const file = {
  id: 1,
  project_id: 10,
  source_machine_id: 1,
  file_path: "D:\\Proj\\Saved\\CollectedPSOs\\x.upipelinecache",
  file_name: "x.upipelinecache",
  size_bytes: 100,
  gpu_signature: "nvidia:RTX 3080:535.98",
  ue_version: "5.4",
  collected_at: null,
};

describe("PsoDistributeWizard", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((fn) => fn.mockReset());
    listenMock.mockClear();
    mockApi.listMachines.mockResolvedValue([
      { id: 1, hostname: "SOURCE", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null },
      { id: 2, hostname: "TARGET", ip: "1.1.1.2", role: "render", status: "online", last_seen_at: null },
    ]);
    mockApi.listCredentials.mockResolvedValue([]);
    mockApi.getGpuConsistencyMatrix.mockResolvedValue({
      signatures: [{ signature: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" }, count: 2 }],
      baseline: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" },
      cells: [
        {
          machine_id: 1,
          hostname: "SOURCE",
          signature: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" },
          status: "match",
        },
        {
          machine_id: 2,
          hostname: "TARGET",
          signature: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" },
          status: "match",
        },
      ],
    });
    mockApi.distributePsoCache.mockResolvedValue({ job_id: "dist-1", plan: [] });
  });

  it("renders when open with a file", async () => {
    const wrapper = mount(PsoDistributeWizard, {
      props: { open: true, file },
    });
    await flushPromises();
    expect(wrapper.find("[data-pso-dist-wizard]").exists()).toBe(true);
    expect(wrapper.find("[data-pso-dist-file-name]").text()).toContain("x.upipelinecache");
    expect(wrapper.find("[data-pso-dist-target-row]").text()).toContain("TARGET");
  });
});
