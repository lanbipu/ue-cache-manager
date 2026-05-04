import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { mockApi, listenMock } = vi.hoisted(() => ({
  mockApi: {
    listProjects: vi.fn(),
    listProjectLocations: vi.fn(),
    listMachines: vi.fn(),
    listCredentials: vi.fn(),
    generateDdcPak: vi.fn(),
    distributeDdcPak: vi.fn(),
    cancelUeJob: vi.fn(),
  },
  listenMock: vi.fn(async () => () => {}),
}));

vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import DDCPak from "@/views/DDCPak.vue";

describe("DDCPak view", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((fn) => fn.mockReset());
    listenMock.mockClear();
    mockApi.listProjects.mockResolvedValue([]);
    mockApi.listProjectLocations.mockResolvedValue([]);
    mockApi.listMachines.mockResolvedValue([]);
    mockApi.listCredentials.mockResolvedValue([]);
  });

  it("shows empty job state", async () => {
    const wrapper = mount(DDCPak);
    await flushPromises();
    expect(wrapper.find("[data-ddc-pak-empty]").exists()).toBe(true);
  });

  it("opens generate wizard", async () => {
    const wrapper = mount(DDCPak);
    await flushPromises();
    await wrapper.find("[data-open-ddc-pak-wizard]").trigger("click");
    expect(wrapper.find("[data-ddc-pak-wizard]").exists()).toBe(true);
  });
});
