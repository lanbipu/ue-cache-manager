import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import HealthCheck from "@/views/HealthCheck.vue";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn(async () => []),
    getGpuConsistencyMatrix: vi.fn(async () => ({ signatures: [], baseline: null, cells: [] })),
    testPowerShellBridge: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

describe("HealthCheck view", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockApi.testPowerShellBridge.mockReset();
  });

  it("renders full check action", () => {
    const wrapper = mount(HealthCheck);
    expect(wrapper.find("[data-open-health-wizard-btn]").exists()).toBe(true);
  });

  it("clicking the bridge test button calls the backend and shows result", async () => {
    mockApi.testPowerShellBridge.mockResolvedValue({
      received: "hello",
      timestamp: "2026-05-01T12:00:00",
      machine: "TESTPC",
    });
    const wrapper = mount(HealthCheck);

    await wrapper.find("[data-bridge-test-btn]").trigger("click");
    await flushPromises();

    expect(mockApi.testPowerShellBridge).toHaveBeenCalledWith("hello from UECM");
    const result = wrapper.find("[data-bridge-result]");
    expect(result.exists()).toBe(true);
    expect(result.text()).toContain("TESTPC");
    expect(result.text()).toContain("hello");
  });

  it("shows error when bridge call fails", async () => {
    mockApi.testPowerShellBridge.mockRejectedValue({
      code: "POWERSHELL",
      message: "not on windows",
    });
    const wrapper = mount(HealthCheck);

    await wrapper.find("[data-bridge-test-btn]").trigger("click");
    await flushPromises();

    expect(wrapper.find("[data-bridge-result]").text()).toContain("not on windows");
  });
});
