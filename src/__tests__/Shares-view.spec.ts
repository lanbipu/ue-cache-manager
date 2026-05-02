import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listShares: vi.fn(),
    listMachines: vi.fn(),
    deleteShare: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));

import Shares from "@/views/Shares.vue";

describe("Shares view", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m) => m.mockReset());
    mockApi.listMachines.mockResolvedValue([
      { id: 5, hostname: "lanPC", ip: "192.168.10.20", role: "render", status: "online", last_seen_at: null },
    ]);
  });

  it("shows empty message when no shares exist", async () => {
    mockApi.listShares.mockResolvedValue([]);
    const wrapper = mount(Shares);
    await flushPromises();
    expect(wrapper.find("[data-shares-empty]").exists()).toBe(true);
  });

  it("renders one row per share with hostname resolved", async () => {
    mockApi.listShares.mockResolvedValue([
      {
        id: 1,
        host_machine_id: 5,
        share_name: "DDC",
        unc_path: "\\\\lanPC\\DDC",
        local_path: "D:\\DDC",
        mode: "managed",
        credential_alias: "UECM:share:lanPC:ddc-svc",
      },
    ]);
    const wrapper = mount(Shares);
    await flushPromises();
    expect(wrapper.findAll("[data-share-row]").length).toBe(1);
    expect(wrapper.text()).toContain("lanPC");
    expect(wrapper.text()).toContain("\\\\lanPC\\DDC");
  });

  it("Delete button reveals confirm panel with also-remove-remote checkbox", async () => {
    mockApi.listShares.mockResolvedValue([
      {
        id: 1,
        host_machine_id: 5,
        share_name: "DDC",
        unc_path: "\\\\lanPC\\DDC",
        local_path: "D:\\DDC",
        mode: "open",
        credential_alias: null,
      },
    ]);
    const wrapper = mount(Shares);
    await flushPromises();
    await wrapper.find("[data-share-delete-btn]").trigger("click");
    expect(wrapper.find("[data-delete-confirm]").exists()).toBe(true);
    expect(wrapper.find("[data-also-remove-remote]").exists()).toBe(true);
  });

  it("Confirm Delete calls deleteShare with also_remove_remote flag", async () => {
    mockApi.listShares
      .mockResolvedValueOnce([
        {
          id: 1,
          host_machine_id: 5,
          share_name: "DDC",
          unc_path: "\\\\lanPC\\DDC",
          local_path: "D:\\DDC",
          mode: "open",
          credential_alias: null,
        },
      ])
      .mockResolvedValueOnce([]);
    mockApi.deleteShare.mockResolvedValue(undefined);
    const wrapper = mount(Shares);
    await flushPromises();
    await wrapper.find("[data-share-delete-btn]").trigger("click");
    await wrapper.find("[data-also-remove-remote]").setValue(true);
    await wrapper.find("[data-confirm-delete]").trigger("click");
    await flushPromises();
    expect(mockApi.deleteShare).toHaveBeenCalledWith(1, true);
  });
});
