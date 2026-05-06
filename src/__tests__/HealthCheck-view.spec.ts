import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { mount } from "@vue/test-utils";
import HealthCheck from "@/views/HealthCheck.vue";

vi.mock("@/services/tauri", () => ({
  tauriApi: {
    listMachines: vi.fn(async () => []),
    getGpuConsistencyMatrix: vi.fn(async () => ({ signatures: [], baseline: null, cells: [] })),
  },
}));

describe("HealthCheck view", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("renders full check action", () => {
    const wrapper = mount(HealthCheck);
    expect(wrapper.find("[data-open-health-wizard-btn]").exists()).toBe(true);
  });
});
