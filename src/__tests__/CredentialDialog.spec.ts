import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listCredentials: vi.fn(),
    saveCredential: vi.fn(),
    deleteCredential: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import CredentialDialog from "@/components/modals/CredentialDialog.vue";

describe("CredentialDialog", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m: any) => m.mockReset());
  });

  it("loads credentials when opened", async () => {
    mockApi.listCredentials.mockResolvedValue([
      { id: 1, alias: "UECM:winrm:HOST-A", kind: "winrm", username: "admin" },
    ]);
    const wrapper = mount(CredentialDialog, { props: { open: false } });
    await wrapper.setProps({ open: true });
    await flushPromises();
    expect(mockApi.listCredentials).toHaveBeenCalled();
    const rows = wrapper.findAll("[data-cred-row]");
    expect(rows.length).toBe(1);
  });

  it("save form submits alias/kind/username/password", async () => {
    mockApi.listCredentials.mockResolvedValue([]);
    mockApi.saveCredential.mockResolvedValue(1);
    const wrapper = mount(CredentialDialog, { props: { open: false } });
    await wrapper.setProps({ open: true });
    await flushPromises();

    await wrapper.find("[data-cred-alias]").setValue("UECM:winrm:HOST-A");
    await wrapper.find("[data-cred-username]").setValue("admin");
    await wrapper.find("[data-cred-password]").setValue("p@ss");
    await wrapper.find("[data-cred-save-btn]").trigger("click");
    await flushPromises();

    expect(mockApi.saveCredential).toHaveBeenCalledWith("UECM:winrm:HOST-A", "winrm", "admin", "p@ss");
  });

  it("delete button calls deleteCredential", async () => {
    mockApi.listCredentials.mockResolvedValue([
      { id: 1, alias: "UECM:winrm:HOST-A", kind: "winrm", username: "admin" },
    ]);
    mockApi.deleteCredential.mockResolvedValue(undefined);
    const wrapper = mount(CredentialDialog, { props: { open: false } });
    await wrapper.setProps({ open: true });
    await flushPromises();

    await wrapper.find("[data-cred-delete-btn]").trigger("click");
    await flushPromises();

    expect(mockApi.deleteCredential).toHaveBeenCalledWith("UECM:winrm:HOST-A");
  });
});
