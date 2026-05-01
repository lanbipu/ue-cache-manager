import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    scanNetwork: vi.fn(),
    addDiscoveredMachine: vi.fn(),
    listMachines: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import DiscoveryWizard from "@/components/modals/DiscoveryWizard.vue";

describe("DiscoveryWizard", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m: any) => m.mockReset());
    mockApi.listMachines.mockResolvedValue([]);
  });

  it("renders nothing when open=false", () => {
    const wrapper = mount(DiscoveryWizard, { props: { open: false } });
    expect(document.body.querySelector("[data-modal]")).toBeNull();
  });

  it("scan button calls scanNetwork with the CIDR input value", async () => {
    mockApi.scanNetwork.mockResolvedValue({ probed: [] });
    const wrapper = mount(DiscoveryWizard, { props: { open: true } });
    await flushPromises();
    await wrapper.find("[data-cidr-input]").setValue("10.0.0.0/30");
    await wrapper.find("[data-scan-btn]").trigger("click");
    await flushPromises();
    expect(mockApi.scanNetwork).toHaveBeenCalledWith("10.0.0.0/30");
  });

  it("renders probed hosts after scan", async () => {
    mockApi.scanNetwork.mockResolvedValue({
      probed: [
        { ip: "192.168.10.21", winrm_open: true, smb_open: true },
        { ip: "192.168.10.22", winrm_open: false, smb_open: true },
      ],
    });
    const wrapper = mount(DiscoveryWizard, { props: { open: true } });
    await flushPromises();
    await wrapper.find("[data-scan-btn]").trigger("click");
    await flushPromises();
    const rows = wrapper.findAll("[data-probed-row]");
    expect(rows.length).toBe(2);
  });

  it("Add button calls addDiscoveredMachine for the row's IP", async () => {
    mockApi.scanNetwork.mockResolvedValue({
      probed: [{ ip: "192.168.10.21", winrm_open: true, smb_open: true }],
    });
    mockApi.addDiscoveredMachine.mockResolvedValue(7);
    const wrapper = mount(DiscoveryWizard, { props: { open: true } });
    await flushPromises();
    await wrapper.find("[data-scan-btn]").trigger("click");
    await flushPromises();
    await wrapper.find("[data-add-btn]").trigger("click");
    await flushPromises();
    expect(mockApi.addDiscoveredMachine).toHaveBeenCalledWith("192.168.10.21", null);
  });
});
