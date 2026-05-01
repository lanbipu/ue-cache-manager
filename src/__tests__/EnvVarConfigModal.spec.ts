import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    getMachineEnvVar: vi.fn(),
    setMachineEnvVar: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import EnvVarConfigModal from "@/components/modals/EnvVarConfigModal.vue";

describe("EnvVarConfigModal", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m: any) => m.mockReset());
  });

  it("reads current value when opened", async () => {
    mockApi.getMachineEnvVar.mockResolvedValue("\\\\HOST\\DDC");
    const wrapper = mount(EnvVarConfigModal, {
      props: { open: false, machineId: 5, varName: "UE-SharedDataCachePath" },
    });
    await wrapper.setProps({ open: true });
    await flushPromises();
    expect(mockApi.getMachineEnvVar).toHaveBeenCalledWith(5, "UE-SharedDataCachePath");
    const currentEl = wrapper.find("[data-current-value]");
    expect(currentEl.text()).toContain("\\\\HOST\\DDC");
  });

  it("apply submits the new value via setMachineEnvVar", async () => {
    mockApi.getMachineEnvVar.mockResolvedValue(null);
    mockApi.setMachineEnvVar.mockResolvedValue(undefined);
    const wrapper = mount(EnvVarConfigModal, {
      props: { open: false, machineId: 5, varName: "UE-SharedDataCachePath" },
    });
    await wrapper.setProps({ open: true });
    await flushPromises();

    await wrapper.find("[data-new-value]").setValue("\\\\HOST\\NewDDC");
    await wrapper.find("[data-apply-btn]").trigger("click");
    await flushPromises();

    expect(mockApi.setMachineEnvVar).toHaveBeenCalledWith(5, "UE-SharedDataCachePath", "\\\\HOST\\NewDDC");
  });
});
