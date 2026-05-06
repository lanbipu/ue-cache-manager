import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { mockApi, listenMock } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn(),
    listProjects: vi.fn(),
    listCredentials: vi.fn(),
    listPsoCacheFiles: vi.fn(),
    startPsoCollection: vi.fn(),
    distributePsoCache: vi.fn(),
    cancelUeJob: vi.fn(),
    getGpuConsistencyMatrix: vi.fn(),
  },
  listenMock: vi.fn(async () => () => {}),
}));

vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import PSOCache from "@/views/PSOCache.vue";

describe("PSOCache view", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((fn) => fn.mockReset());
    listenMock.mockClear();
    mockApi.listMachines.mockResolvedValue([]);
    mockApi.listProjects.mockResolvedValue([]);
    mockApi.listCredentials.mockResolvedValue([]);
    mockApi.listPsoCacheFiles.mockResolvedValue([]);
    mockApi.getGpuConsistencyMatrix.mockResolvedValue({ signatures: [], baseline: null, cells: [] });
  });

  it("shows empty state", async () => {
    const wrapper = mount(PSOCache);
    await flushPromises();
    expect(wrapper.find("[data-pso-cache-empty]").exists()).toBe(true);
  });

  it("opens collect wizard", async () => {
    const wrapper = mount(PSOCache);
    await flushPromises();
    await wrapper.find("[data-pso-collect-btn]").trigger("click");
    expect(wrapper.find("[data-pso-collect-wizard]").exists()).toBe(true);
  });
});
