import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi, listenMock, captured } = vi.hoisted(() => {
  const captured: { handler: ((e: { payload: unknown }) => void) | null } = { handler: null };
  return {
    mockApi: {
      listMachines: vi.fn(),
      listCredentials: vi.fn(),
      batchSetIniKey: vi.fn(),
    },
    listenMock: vi.fn(async (_n: string, h: (e: { payload: unknown }) => void) => {
      captured.handler = h;
      return () => { captured.handler = null; };
    }),
    captured,
  };
});

vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import BatchIniEditModal from "@/components/modals/BatchIniEditModal.vue";
import { useMachinesStore } from "@/stores/machines";
import { useCredentialsStore } from "@/stores/credentials";

async function setup() {
  setActivePinia(createPinia());
  Object.values(mockApi).forEach((m) => m.mockReset());
  listenMock.mockClear();
  captured.handler = null;
  mockApi.listCredentials.mockResolvedValue([
    { id: 1, alias: "UECM:winrm:LANPC", kind: "winrm", username: "lanpc" },
  ]);
  const m = useMachinesStore();
  m.machines = [
    { id: 5, hostname: "lanPC", ip: "192.168.10.20", role: "render", status: "online", last_seen_at: null },
  ];
  const c = useCredentialsStore();
  c.credentials = [
    { id: 1, alias: "UECM:winrm:LANPC", kind: "winrm", username: "lanpc" },
  ];
}

describe("BatchIniEditModal", () => {
  beforeEach(setup);

  it("renders nothing when open=false", () => {
    mount(BatchIniEditModal, { props: { open: false, machineIds: [] } });
    expect(document.body.querySelector("[data-batch-ini-modal]")).toBeNull();
  });

  it("Apply is disabled when key is empty", async () => {
    const wrapper = mount(BatchIniEditModal, { props: { open: true, machineIds: [5] } });
    await flushPromises();
    await wrapper.find("[data-ini-cred]").setValue("UECM:winrm:LANPC");
    expect(wrapper.find("[data-batch-apply-btn]").attributes("disabled")).toBeDefined();
  });

  it("Apply dispatches batchSetIniKey with all fields", async () => {
    mockApi.batchSetIniKey.mockResolvedValue(undefined);
    const wrapper = mount(BatchIniEditModal, { props: { open: true, machineIds: [5] } });
    await flushPromises();
    await wrapper.find("[data-ini-path]").setValue("C:\\proj\\Config\\DefaultEngine.ini");
    await wrapper.find("[data-ini-section]").setValue("Core.System");
    await wrapper.find("[data-ini-name]").setValue("Paths");
    await wrapper.find("[data-ini-value]").setValue("../Content");
    await wrapper.find("[data-ini-cred]").setValue("UECM:winrm:LANPC");
    await wrapper.find("[data-batch-apply-btn]").trigger("click");
    await flushPromises();
    expect(mockApi.batchSetIniKey).toHaveBeenCalledWith(
      [5],
      "C:\\proj\\Config\\DefaultEngine.ini",
      "Core.System",
      "Paths",
      "../Content",
      "UECM:winrm:LANPC",
    );
  });

  it("renders progress table", async () => {
    const wrapper = mount(BatchIniEditModal, { props: { open: true, machineIds: [5] } });
    await flushPromises();
    expect(wrapper.find("[data-batch-progress]").exists()).toBe(true);
  });

  it("shows machine count", async () => {
    const wrapper = mount(BatchIniEditModal, { props: { open: true, machineIds: [5, 6] } });
    await flushPromises();
    expect(wrapper.text()).toContain("Will apply to 2 machines");
  });
});
