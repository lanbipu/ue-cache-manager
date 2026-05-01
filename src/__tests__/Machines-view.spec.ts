import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";

// vi.hoisted ensures mockApi is initialized before vi.mock factory runs
const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn(),
    addMachine: vi.fn(),
    deleteMachine: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import Machines from "@/views/Machines.vue";

describe("Machines view", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockApi.listMachines.mockReset();
    mockApi.addMachine.mockReset();
    mockApi.deleteMachine.mockReset();
  });

  it("shows empty state when no machines", async () => {
    mockApi.listMachines.mockResolvedValue([]);
    const wrapper = mount(Machines);
    await flushPromises();
    expect(wrapper.text()).toContain("No machines");
  });

  it("renders a row for each machine", async () => {
    mockApi.listMachines.mockResolvedValue([
      { id: 1, hostname: "RENDER-01", ip: "192.168.10.21", role: "render", status: "online", last_seen_at: null },
      { id: 2, hostname: "HOST-NAS", ip: "192.168.10.2", role: "host", status: "online", last_seen_at: null },
    ]);
    const wrapper = mount(Machines);
    await flushPromises();
    const rows = wrapper.findAll("[data-machine-row]");
    expect(rows).toHaveLength(2);
    expect(wrapper.text()).toContain("RENDER-01");
    expect(wrapper.text()).toContain("HOST-NAS");
  });

  it("adds a machine via form", async () => {
    mockApi.listMachines.mockResolvedValue([]);
    mockApi.addMachine.mockResolvedValue(99);
    const wrapper = mount(Machines);
    await flushPromises();

    await wrapper.find("[data-input-hostname]").setValue("NEW-PC");
    await wrapper.find("[data-input-ip]").setValue("192.168.10.99");
    await wrapper.find("[data-add-btn]").trigger("click");
    await flushPromises();

    expect(mockApi.addMachine).toHaveBeenCalledWith("NEW-PC", "192.168.10.99");
  });
});
