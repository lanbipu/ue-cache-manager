import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    getMachineDetail: vi.fn(),
    refreshMachine: vi.fn(),
    listCredentials: vi.fn(),
    bootstrapWinrm: vi.fn(),
    getWinrmBootstrapScript: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import MachineDetail from "@/components/machines/MachineDetail.vue";
import { useMachinesStore } from "@/stores/machines";

describe("MachineDetail", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockApi.getMachineDetail.mockReset();
    mockApi.refreshMachine.mockReset();
    mockApi.listCredentials.mockReset();
    mockApi.bootstrapWinrm.mockReset();
    mockApi.getWinrmBootstrapScript.mockReset();
    mockApi.listCredentials.mockResolvedValue([]);
  });

  it("shows empty state when no detail selected", () => {
    const wrapper = mount(MachineDetail);
    expect(wrapper.text()).toContain("Select a machine");
  });

  it("renders hostname/ip from selectedDetail", async () => {
    mockApi.getMachineDetail.mockResolvedValue({
      machine: {
        id: 1,
        hostname: "RENDER-01",
        ip: "192.168.10.21",
        role: "render",
        status: "online",
        last_seen_at: null,
      },
      ue_installs: [],
      gpus: [],
    });
    const store = useMachinesStore();
    await store.selectMachine(1);
    const wrapper = mount(MachineDetail);
    await flushPromises();
    expect(wrapper.text()).toContain("RENDER-01");
    expect(wrapper.text()).toContain("192.168.10.21");
  });

  it("renders UE installs and GPUs when present", async () => {
    mockApi.getMachineDetail.mockResolvedValue({
      machine: {
        id: 1,
        hostname: "RENDER-01",
        ip: "192.168.10.21",
        role: "render",
        status: "online",
        last_seen_at: null,
      },
      ue_installs: [
        { id: 1, machine_id: 1, version: "5.4", install_path: "C:\\UE_5.4", is_primary: true },
      ],
      gpus: [
        { id: 1, machine_id: 1, gpu_model: "RTX 4090", driver_version: "551.86", vendor: "nvidia", vram_mb: 24576 },
      ],
    });
    const store = useMachinesStore();
    await store.selectMachine(1);
    const wrapper = mount(MachineDetail);
    await flushPromises();
    expect(wrapper.text()).toContain("5.4");
    expect(wrapper.text()).toContain("C:\\UE_5.4");
    expect(wrapper.text()).toContain("RTX 4090");
    expect(wrapper.text()).toContain("551.86");
  });

  it("renders online status badge with green class", async () => {
    mockApi.getMachineDetail.mockResolvedValue({
      machine: {
        id: 1,
        hostname: "RENDER-01",
        ip: "192.168.10.21",
        role: "render",
        status: "online",
        last_seen_at: "2026-05-02 10:00:00",
      },
      ue_installs: [],
      gpus: [],
    });
    const store = useMachinesStore();
    await store.selectMachine(1);
    const wrapper = mount(MachineDetail);
    await flushPromises();
    const badge = wrapper.find("[data-status-badge]");
    expect(badge.exists()).toBe(true);
    expect(badge.text()).toBe("online");
    expect(badge.classes()).toContain("bg-emerald-500");
    expect(badge.classes()).toContain("text-white");
  });

  it("renders offline status badge with red class", async () => {
    mockApi.getMachineDetail.mockResolvedValue({
      machine: {
        id: 1,
        hostname: "RENDER-01",
        ip: "192.168.10.21",
        role: "render",
        status: "offline",
        last_seen_at: "2026-05-02 10:00:00",
      },
      ue_installs: [],
      gpus: [],
    });
    const store = useMachinesStore();
    await store.selectMachine(1);
    const wrapper = mount(MachineDetail);
    await flushPromises();
    const badge = wrapper.find("[data-status-badge]");
    expect(badge.exists()).toBe(true);
    expect(badge.text()).toBe("offline");
    expect(badge.classes()).toContain("bg-rose-500");
    expect(badge.classes()).toContain("text-white");
  });

  it("renders unknown status badge with gray class", async () => {
    mockApi.getMachineDetail.mockResolvedValue({
      machine: {
        id: 1,
        hostname: "RENDER-01",
        ip: "192.168.10.21",
        role: "render",
        status: "unknown",
        last_seen_at: null,
      },
      ue_installs: [],
      gpus: [],
    });
    const store = useMachinesStore();
    await store.selectMachine(1);
    const wrapper = mount(MachineDetail);
    await flushPromises();
    const badge = wrapper.find("[data-status-badge]");
    expect(badge.exists()).toBe(true);
    expect(badge.text()).toBe("unknown");
    expect(badge.classes()).toContain("bg-muted");
    expect(badge.classes()).toContain("text-muted-foreground");
  });

  it("clicking refresh button calls store.refreshSelected", async () => {
    mockApi.getMachineDetail.mockResolvedValue({
      machine: {
        id: 1,
        hostname: "RENDER-01",
        ip: "192.168.10.21",
        role: "render",
        status: "online",
        last_seen_at: null,
      },
      ue_installs: [],
      gpus: [],
    });
    mockApi.refreshMachine.mockResolvedValue({
      machine_id: 1,
      winrm_ok: true,
      ue_installs: [],
      gpus: [],
      error: null,
    });
    const store = useMachinesStore();
    await store.selectMachine(1);
    const wrapper = mount(MachineDetail);
    await flushPromises();
    await wrapper.find("[data-refresh-btn]").trigger("click");
    await flushPromises();
    expect(mockApi.refreshMachine).toHaveBeenCalledWith(1);
  });

  it("shows bootstrap controls for offline machines", async () => {
    mockApi.listCredentials.mockResolvedValue([
      { id: 3, alias: "UECM:winrm:RAZER", kind: "winrm", username: "RAZER\\admin" },
    ]);
    mockApi.getMachineDetail.mockResolvedValue({
      machine: {
        id: 1,
        hostname: "RAZER",
        ip: "192.168.10.173",
        role: "unknown",
        status: "offline",
        last_seen_at: null,
      },
      ue_installs: [],
      gpus: [],
    });
    const store = useMachinesStore();
    await store.selectMachine(1);
    const wrapper = mount(MachineDetail);
    await flushPromises();
    expect(wrapper.find("[data-bootstrap-panel]").exists()).toBe(true);
    expect(wrapper.find("[data-bootstrap-cred]").exists()).toBe(true);
    expect(wrapper.find("[data-bootstrap-btn]").exists()).toBe(true);
    expect(wrapper.text()).toContain("first-contact bootstrap");
  });

  it("clicking bootstrap calls bootstrapSelected with the selected credential", async () => {
    mockApi.listCredentials.mockResolvedValue([
      { id: 3, alias: "UECM:winrm:RAZER", kind: "winrm", username: "RAZER\\admin" },
    ]);
    mockApi.getMachineDetail.mockResolvedValueOnce({
      machine: {
        id: 1,
        hostname: "RAZER",
        ip: "192.168.10.173",
        role: "unknown",
        status: "offline",
        last_seen_at: null,
      },
      ue_installs: [],
      gpus: [],
    });
    mockApi.bootstrapWinrm.mockResolvedValue({
      ok: true,
      method: "psexec",
      message: "WinRM enabled",
      winrm_ok: true,
      manual_script: null,
    });
    mockApi.getMachineDetail.mockResolvedValueOnce({
      machine: {
        id: 1,
        hostname: "RAZER",
        ip: "192.168.10.173",
        role: "unknown",
        status: "online",
        last_seen_at: "2026-05-09 12:00:00",
      },
      ue_installs: [],
      gpus: [],
    });
    const store = useMachinesStore();
    await store.selectMachine(1);
    const wrapper = mount(MachineDetail);
    await flushPromises();
    await wrapper.find("[data-bootstrap-cred]").setValue("UECM:winrm:RAZER");
    await wrapper.find("[data-bootstrap-btn]").trigger("click");
    await flushPromises();
    expect(mockApi.bootstrapWinrm).toHaveBeenCalledWith(1, "UECM:winrm:RAZER", false);
  });

  it("passes the workgroup local-admin option when checked", async () => {
    mockApi.listCredentials.mockResolvedValue([
      { id: 3, alias: "UECM:winrm:RAZER", kind: "winrm", username: "RAZER\\admin" },
    ]);
    mockApi.getMachineDetail.mockResolvedValueOnce({
      machine: {
        id: 1,
        hostname: "RAZER",
        ip: "192.168.10.173",
        role: "unknown",
        status: "offline",
        last_seen_at: null,
      },
      ue_installs: [],
      gpus: [],
    });
    mockApi.bootstrapWinrm.mockResolvedValue({
      ok: true,
      method: "psexec",
      message: "WinRM enabled",
      winrm_ok: true,
      manual_script: null,
    });
    mockApi.getMachineDetail.mockResolvedValueOnce({
      machine: {
        id: 1,
        hostname: "RAZER",
        ip: "192.168.10.173",
        role: "unknown",
        status: "online",
        last_seen_at: "2026-05-09 12:00:00",
      },
      ue_installs: [],
      gpus: [],
    });
    const store = useMachinesStore();
    await store.selectMachine(1);
    const wrapper = mount(MachineDetail);
    await flushPromises();
    await wrapper.find("[data-bootstrap-local-admin]").setValue(true);
    await wrapper.find("[data-bootstrap-btn]").trigger("click");
    await flushPromises();
    expect(mockApi.bootstrapWinrm).toHaveBeenCalledWith(1, "UECM:winrm:RAZER", true);
  });

  it("loads the manual bootstrap script on demand", async () => {
    mockApi.getMachineDetail.mockResolvedValue({
      machine: {
        id: 1,
        hostname: "RAZER",
        ip: "192.168.10.173",
        role: "unknown",
        status: "offline",
        last_seen_at: null,
      },
      ue_installs: [],
      gpus: [],
    });
    mockApi.getWinrmBootstrapScript.mockResolvedValue("Enable-PSRemoting -Force");
    const store = useMachinesStore();
    await store.selectMachine(1);
    const wrapper = mount(MachineDetail);
    await flushPromises();
    await wrapper.find("[data-load-bootstrap-script]").trigger("click");
    await flushPromises();
    expect(mockApi.getWinrmBootstrapScript).toHaveBeenCalled();
    expect(wrapper.find("[data-bootstrap-script]").text()).toContain("Enable-PSRemoting");
  });
});
