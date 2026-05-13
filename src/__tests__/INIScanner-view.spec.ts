import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn(async () => []),
    listCredentials: vi.fn(async () => []),
    listFindingsForRun: vi.fn(async () => []),
    listRecentIniRuns: vi.fn(async () => []),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import INIScanner from "@/views/INIScanner.vue";

describe("INIScanner view", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m: any) => m.mockReset?.());
    mockApi.listMachines.mockResolvedValue([]);
    mockApi.listCredentials.mockResolvedValue([]);
  });

  it("renders empty state when no scan run yet", () => {
    const w = mount(INIScanner);
    expect(w.text()).toMatch(/Run an INI scan/i);
  });
  it("shows scan button", () => {
    const w = mount(INIScanner);
    expect(w.find("[data-open-ini-scan-btn]").exists()).toBe(true);
  });
});
