import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn(async () => []),
    listCredentials: vi.fn(async () => []),
    listHealthResultsForRun: vi.fn(async () => []),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import HealthCheck from "@/views/HealthCheck.vue";

describe("HealthCheck view", () => {
  beforeEach(() => { setActivePinia(createPinia()); });
  it("renders empty state with run button", () => {
    const w = mount(HealthCheck);
    expect(w.find("[data-open-health-wizard-btn]").exists()).toBe(true);
  });
});
