import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    readIniSection: vi.fn(),
    setIniKey: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import IniEditModal from "@/components/modals/IniEditModal.vue";

describe("IniEditModal", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m: any) => m.mockReset());
  });

  it("loads keys when Read button clicked", async () => {
    mockApi.readIniSection.mockResolvedValue([{ name: "Path", value: "../Content" }]);
    const wrapper = mount(IniEditModal, { props: { open: false, machineId: 5 } });
    await wrapper.setProps({ open: true });
    await flushPromises();

    await wrapper.find("[data-ini-path]").setValue("C:\\Proj\\Config\\DefaultEngine.ini");
    await wrapper.find("[data-ini-section]").setValue("Core.System");
    await wrapper.find("[data-ini-read-btn]").trigger("click");
    await flushPromises();

    expect(mockApi.readIniSection).toHaveBeenCalledWith(5, "C:\\Proj\\Config\\DefaultEngine.ini", "Core.System");
    const rows = wrapper.findAll("[data-ini-row]");
    expect(rows.length).toBe(1);
  });

  it("apply calls setIniKey and shows backup path", async () => {
    mockApi.readIniSection.mockResolvedValue([]);
    mockApi.setIniKey.mockResolvedValue({ backup_path: "C:\\Proj\\Config\\DefaultEngine.ini.uecm-bak-1700000000" });
    const wrapper = mount(IniEditModal, { props: { open: false, machineId: 5 } });
    await wrapper.setProps({ open: true });
    await flushPromises();

    await wrapper.find("[data-ini-path]").setValue("C:\\Proj\\Config\\DefaultEngine.ini");
    await wrapper.find("[data-ini-section]").setValue("Core.System");
    await wrapper.find("[data-ini-key]").setValue("Paths");
    await wrapper.find("[data-ini-value]").setValue("../Content/NewPath");
    await wrapper.find("[data-ini-apply-btn]").trigger("click");
    await flushPromises();

    expect(mockApi.setIniKey).toHaveBeenCalledWith(
      5,
      "C:\\Proj\\Config\\DefaultEngine.ini",
      "Core.System",
      "Paths",
      "../Content/NewPath",
    );
    expect(wrapper.html()).toContain("uecm-bak-1700000000");
  });
});
