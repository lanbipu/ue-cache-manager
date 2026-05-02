import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi, listenMock, captured } = vi.hoisted(() => {
  const captured: { handler: ((e: { payload: unknown }) => void) | null } = { handler: null };
  return {
    mockApi: {
      listMachines: vi.fn(),
      listCredentials: vi.fn(),
      batchSetEnvVar: vi.fn(),
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

import BatchEnvVarModal from "@/components/modals/BatchEnvVarModal.vue";
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

describe("BatchEnvVarModal", () => {
  beforeEach(setup);

  it("renders nothing when open=false", () => {
    mount(BatchEnvVarModal, { props: { open: false, machineIds: [] } });
    expect(document.body.querySelector("[data-batch-env-modal]")).toBeNull();
  });

  it("Apply is disabled when no credential selected", async () => {
    const wrapper = mount(BatchEnvVarModal, { props: { open: true, machineIds: [5] } });
    await flushPromises();
    const btn = wrapper.find("[data-batch-apply-btn]");
    expect(btn.attributes("disabled")).toBeDefined();
  });

  it("Apply calls batchSetEnvVar with correct args", async () => {
    mockApi.batchSetEnvVar.mockResolvedValue(undefined);
    const wrapper = mount(BatchEnvVarModal, { props: { open: true, machineIds: [5] } });
    await flushPromises();
    await wrapper.find("[data-env-name]").setValue("UE-SharedDataCachePath");
    await wrapper.find("[data-env-value]").setValue("\\\\HOST\\DDC");
    await wrapper.find("[data-env-cred]").setValue("UECM:winrm:LANPC");
    await wrapper.find("[data-batch-apply-btn]").trigger("click");
    await flushPromises();
    expect(mockApi.batchSetEnvVar).toHaveBeenCalledWith(
      [5],
      "UE-SharedDataCachePath",
      "\\\\HOST\\DDC",
      "UECM:winrm:LANPC",
    );
  });

  it("renders progress table with target machines", async () => {
    const wrapper = mount(BatchEnvVarModal, { props: { open: true, machineIds: [5] } });
    await flushPromises();
    expect(wrapper.find("[data-batch-progress]").exists()).toBe(true);
    expect(wrapper.findAll("[data-batch-row]").length).toBe(1);
  });

  it("shows machine count in header", async () => {
    const wrapper = mount(BatchEnvVarModal, { props: { open: true, machineIds: [5] } });
    await flushPromises();
    expect(wrapper.text()).toContain("Will apply to 1 machine");
  });
});
