import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";

// vi.hoisted ensures mockApi is initialized before vi.mock factory runs
const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    testPowerShellBridge: vi.fn(),
    listMachines: vi.fn(),
    listProjects: vi.fn(),
    getGpuConsistencyMatrix: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import Dashboard from "@/views/Dashboard.vue";

describe("Dashboard view", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockApi.testPowerShellBridge.mockReset();
    mockApi.listMachines.mockReset();
    mockApi.listProjects.mockReset();
    mockApi.getGpuConsistencyMatrix.mockReset();
    mockApi.listMachines.mockResolvedValue([]);
    mockApi.listProjects.mockResolvedValue([]);
    mockApi.getGpuConsistencyMatrix.mockResolvedValue({ signatures: [], baseline: null, cells: [] });
  });

  const stubs = {
    RouterLink: { props: ["to"], template: "<a><slot /></a>" },
  };

  it("loads machine stats from the machines store instead of mock data", async () => {
    mockApi.listMachines.mockResolvedValue([
      { id: 1, hostname: "ONLY-REAL", ip: "10.0.0.1", role: "render", status: "online", last_seen_at: null },
      { id: 2, hostname: "OFFLINE-REAL", ip: "10.0.0.2", role: "render", status: "offline", last_seen_at: null },
    ]);
    const wrapper = mount(Dashboard, { global: { stubs } });
    await flushPromises();

    expect(mockApi.listMachines).toHaveBeenCalled();
    expect(wrapper.find("[data-dashboard-kpi]").exists()).toBe(true);
    expect(wrapper.text()).toContain("1 online");
    expect(wrapper.text()).not.toContain("6 online");
  });

  it("clicking the bridge test button calls the backend and shows result", async () => {
    mockApi.testPowerShellBridge.mockResolvedValue({
      received: "hello",
      timestamp: "2026-05-01T12:00:00",
      machine: "TESTPC",
    });
    const wrapper = mount(Dashboard, { global: { stubs } });

    await wrapper.find("[data-bridge-test-btn]").trigger("click");
    await flushPromises();

    expect(mockApi.testPowerShellBridge).toHaveBeenCalledWith("hello from UECM");
    expect(wrapper.text()).toContain("TESTPC");
    expect(wrapper.text()).toContain("hello");
  });

  it("shows error when bridge call fails", async () => {
    mockApi.testPowerShellBridge.mockRejectedValue({
      code: "POWERSHELL",
      message: "not on windows",
    });
    const wrapper = mount(Dashboard, { global: { stubs } });

    await wrapper.find("[data-bridge-test-btn]").trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("not on windows");
  });
});
