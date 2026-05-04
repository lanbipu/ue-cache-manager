import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { mount } from "@vue/test-utils";
import INIScanner from "@/views/INIScanner.vue";

vi.mock("@/services/tauri", () => ({
  tauriApi: {
    listMachines: vi.fn(async () => []),
    listCredentials: vi.fn(async () => []),
  },
}));

describe("INI Scanner view", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("renders run scan action", () => {
    const wrapper = mount(INIScanner);
    expect(wrapper.find("[data-open-ini-scan-btn]").exists()).toBe(true);
  });
});
