import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn(),
    listCredentials: vi.fn(),
    createShare: vi.fn(),
    listShares: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));

import ShareCreateWizard from "@/components/modals/ShareCreateWizard.vue";
import { useMachinesStore } from "@/stores/machines";
import { useCredentialsStore } from "@/stores/credentials";

async function mountReady(open = true) {
  // Pre-populate stores so the modal's options are rendered before the
  // tests interact with them — flushPromises after mount can't reliably
  // satisfy nested await chains in Pinia's setup-style stores under
  // vue-test-utils, so we seed the data directly.
  const machines = useMachinesStore();
  machines.machines = [
    { id: 5, hostname: "lanPC", ip: "192.168.10.20", role: "render", status: "online", last_seen_at: null },
  ];
  const credentials = useCredentialsStore();
  credentials.credentials = [
    { id: 1, alias: "UECM:winrm:LANPC", kind: "winrm", username: "lanpc" },
  ];
  const wrapper = mount(ShareCreateWizard, { props: { open } });
  await flushPromises();
  return wrapper;
}

describe("ShareCreateWizard", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m) => m.mockReset());
    mockApi.listMachines.mockResolvedValue([
      { id: 5, hostname: "lanPC", ip: "192.168.10.20", role: "render", status: "online", last_seen_at: null },
    ]);
    mockApi.listCredentials.mockResolvedValue([
      { id: 1, alias: "UECM:winrm:LANPC", kind: "winrm", username: "lanpc" },
    ]);
    mockApi.listShares.mockResolvedValue([]);
  });

  it("renders nothing when open=false", () => {
    mount(ShareCreateWizard, { props: { open: false } });
    expect(document.body.querySelector("[data-share-wizard]")).toBeNull();
  });

  it("Next on step 1 advances to step 2 once a mode is picked", async () => {
    const wrapper = await mountReady();
    await wrapper.find("[data-mode-open]").setValue();
    await wrapper.find("[data-next-btn]").trigger("click");
    expect(wrapper.find("[data-host-select]").exists()).toBe(true);
  });

  it("Mode A skips svc username field", async () => {
    const wrapper = await mountReady();
    await wrapper.find("[data-next-btn]").trigger("click");
    await wrapper.find("[data-host-select]").setValue("5");
    await wrapper.find("[data-next-btn]").trigger("click");
    expect(wrapper.find("[data-svc-user-input]").exists()).toBe(false);
  });

  it("Mode B requires svc username field", async () => {
    const wrapper = await mountReady();
    await wrapper.find("[data-mode-managed]").setValue();
    await wrapper.find("[data-next-btn]").trigger("click");
    await wrapper.find("[data-host-select]").setValue("5");
    await wrapper.find("[data-next-btn]").trigger("click");
    expect(wrapper.find("[data-svc-user-input]").exists()).toBe(true);
  });

  it("Create button calls createShare with correct args", async () => {
    mockApi.createShare.mockResolvedValue({
      share_config_id: 1,
      unc_path: "\\\\lanPC\\DDC",
      mode: "managed",
      credential_alias: "UECM:share:lanPC:ddc-svc",
    });
    const wrapper = await mountReady();
    await wrapper.find("[data-mode-managed]").setValue();
    await wrapper.find("[data-next-btn]").trigger("click");
    await wrapper.find("[data-host-select]").setValue("5");
    await wrapper.find("[data-next-btn]").trigger("click");
    // step 3 defaults: shareName=DDC, localPath=D:\DDC, svcUsername=ddc-svc
    await wrapper.find("[data-next-btn]").trigger("click");
    // step 4 -> create
    await wrapper.find("[data-create-btn]").trigger("click");
    await flushPromises();
    expect(mockApi.createShare).toHaveBeenCalledWith(
      5,
      "managed",
      "DDC",
      "D:\\DDC",
      null,
      "ddc-svc",
    );
  });

  it("preview shows host hostname and share UNC", async () => {
    const wrapper = await mountReady();
    await wrapper.find("[data-next-btn]").trigger("click");
    await wrapper.find("[data-host-select]").setValue("5");
    await wrapper.find("[data-next-btn]").trigger("click");
    await wrapper.find("[data-next-btn]").trigger("click");
    const preview = wrapper.find("[data-preview]").text();
    expect(preview).toContain("lanPC");
    expect(preview).toContain("\\\\lanPC\\DDC");
  });
});
