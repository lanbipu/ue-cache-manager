import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

// vi.hoisted ensures mockApi is initialized before vi.mock factory runs
const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    testPowerShellBridge: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import Dashboard from "@/views/Dashboard.vue";

describe("Dashboard view", () => {
  beforeEach(() => {
    mockApi.testPowerShellBridge.mockReset();
  });

  it("clicking the bridge test button calls the backend and shows result", async () => {
    mockApi.testPowerShellBridge.mockResolvedValue({
      received: "hello",
      timestamp: "2026-05-01T12:00:00",
      machine: "TESTPC",
    });
    const wrapper = mount(Dashboard);

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
    const wrapper = mount(Dashboard);

    await wrapper.find("[data-bridge-test-btn]").trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("not on windows");
  });
});
