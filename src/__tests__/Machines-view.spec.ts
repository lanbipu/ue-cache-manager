import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn(),
    addMachine: vi.fn(),
    deleteMachine: vi.fn(),
    getMachineDetail: vi.fn(),
    refreshMachine: vi.fn(),
    bootstrapWinrm: vi.fn(),
    getWinrmBootstrapScript: vi.fn(),
    listCredentials: vi.fn(),
    scanNetwork: vi.fn(),
    addDiscoveredMachine: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import Machines from "@/views/Machines.vue";

describe("Machines view", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m: any) => m.mockReset());
    mockApi.listCredentials.mockResolvedValue([]);
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
  });

  it("clicking Scan button reveals discovery wizard", async () => {
    mockApi.listMachines.mockResolvedValue([]);
    const wrapper = mount(Machines);
    await flushPromises();
    await wrapper.find("[data-discover-btn]").trigger("click");
    // DiscoveryWizard renders via Teleport; in tests Teleport is stubbed
    // so the modal renders inside the wrapper rather than on document.body.
    expect(wrapper.html()).toContain("data-modal");
  });

  it("clicking a row calls selectMachine", async () => {
    mockApi.listMachines.mockResolvedValue([
      { id: 5, hostname: "RENDER-05", ip: "192.168.10.25", role: "render", status: "online", last_seen_at: null },
    ]);
    mockApi.getMachineDetail.mockResolvedValue({
      machine: { id: 5, hostname: "RENDER-05", ip: "192.168.10.25", role: "render", status: "online", last_seen_at: null },
      ue_installs: [],
      gpus: [],
    });
    const wrapper = mount(Machines);
    await flushPromises();
    await wrapper.find("[data-machine-row]").trigger("click");
    await flushPromises();
    expect(mockApi.getMachineDetail).toHaveBeenCalledWith(5);
  });
});
