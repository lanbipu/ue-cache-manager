import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

// vi.hoisted ensures mockApi is initialized before vi.mock factory runs
const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn(),
    addMachine: vi.fn(),
    deleteMachine: vi.fn(),
    renameMachine: vi.fn(),
    getMachineDetail: vi.fn(),
    refreshMachine: vi.fn(),
    bootstrapWinrm: vi.fn(),
    getWinrmBootstrapScript: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import { useMachinesStore } from "@/stores/machines";

describe("machines store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockApi.listMachines.mockReset();
    mockApi.addMachine.mockReset();
    mockApi.deleteMachine.mockReset();
    mockApi.renameMachine.mockReset();
    mockApi.getMachineDetail.mockReset();
    mockApi.refreshMachine.mockReset();
    mockApi.bootstrapWinrm.mockReset();
    mockApi.getWinrmBootstrapScript.mockReset();
  });

  it("starts with empty machines list", () => {
    const store = useMachinesStore();
    expect(store.machines).toEqual([]);
    expect(store.isLoading).toBe(false);
  });

  it("loadMachines populates the list", async () => {
    mockApi.listMachines.mockResolvedValue([
      { id: 1, hostname: "A", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null },
    ]);
    const store = useMachinesStore();
    await store.loadMachines();
    expect(store.machines).toHaveLength(1);
    expect(store.machines[0].hostname).toBe("A");
  });

  it("loadMachines toggles loading state", async () => {
    mockApi.listMachines.mockImplementation(
      () => new Promise((resolve) => setTimeout(() => resolve([]), 10)),
    );
    const store = useMachinesStore();
    const promise = store.loadMachines();
    expect(store.isLoading).toBe(true);
    await promise;
    expect(store.isLoading).toBe(false);
  });

  it("addMachine creates and reloads", async () => {
    mockApi.addMachine.mockResolvedValue(42);
    mockApi.listMachines.mockResolvedValue([
      { id: 42, hostname: "X", ip: "2.2.2.2", role: "unknown", status: "unknown", last_seen_at: null },
    ]);
    const store = useMachinesStore();
    await store.addMachine("X", "2.2.2.2");
    expect(mockApi.addMachine).toHaveBeenCalledWith("X", "2.2.2.2");
    expect(store.machines).toHaveLength(1);
  });

  it("deleteMachine removes and reloads", async () => {
    mockApi.deleteMachine.mockResolvedValue(undefined);
    mockApi.listMachines.mockResolvedValue([]);
    const store = useMachinesStore();
    store.machines = [
      { id: 1, hostname: "A", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null },
    ];
    await store.deleteMachine(1);
    expect(mockApi.deleteMachine).toHaveBeenCalledWith(1);
    expect(store.machines).toEqual([]);
  });

  it("captures errors during load", async () => {
    mockApi.listMachines.mockRejectedValue({ code: "DATABASE", message: "boom" });
    const store = useMachinesStore();
    await store.loadMachines();
    expect(store.error).toEqual({ code: "DATABASE", message: "boom" });
    expect(store.machines).toEqual([]);
  });

  it("selectMachine populates selectedDetail", async () => {
    mockApi.getMachineDetail.mockResolvedValue({
      machine: { id: 5, hostname: "X", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null },
      ue_installs: [],
      gpus: [],
    });
    const store = useMachinesStore();
    await store.selectMachine(5);
    expect(store.selectedDetail?.machine.hostname).toBe("X");
  });

  it("renameMachine calls api then reloads list", async () => {
    mockApi.renameMachine.mockResolvedValue(undefined);
    mockApi.listMachines.mockResolvedValue([
      { id: 1, hostname: "NEW", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null },
    ]);
    const store = useMachinesStore();
    await store.renameMachine(1, "NEW");
    expect(mockApi.renameMachine).toHaveBeenCalledWith(1, "NEW");
    expect(mockApi.listMachines).toHaveBeenCalled();
    expect(store.machines[0].hostname).toBe("NEW");
  });

  it("refreshSelected re-reads detail after refresh", async () => {
    mockApi.getMachineDetail.mockResolvedValueOnce({
      machine: { id: 5, hostname: "X", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null },
      ue_installs: [],
      gpus: [],
    });
    mockApi.refreshMachine.mockResolvedValue({
      machine_id: 5,
      winrm_ok: true,
      ue_installs: [],
      gpus: [],
      error: null,
    });
    mockApi.getMachineDetail.mockResolvedValueOnce({
      machine: { id: 5, hostname: "X", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null },
      ue_installs: [{ id: 1, machine_id: 5, version: "5.4", install_path: "C:\\UE_5.4", is_primary: false }],
      gpus: [],
    });
    const store = useMachinesStore();
    await store.selectMachine(5);
    await store.refreshSelected();
    expect(mockApi.refreshMachine).toHaveBeenCalledWith(5);
    expect(store.selectedDetail?.ue_installs).toHaveLength(1);
  });

  it("bootstrapSelected re-reads detail after successful bootstrap", async () => {
    mockApi.getMachineDetail.mockResolvedValueOnce({
      machine: { id: 5, hostname: "X", ip: "1.1.1.1", role: "render", status: "offline", last_seen_at: null },
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
      machine: { id: 5, hostname: "X", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: "2026-05-09 12:00:00" },
      ue_installs: [],
      gpus: [],
    });
    const store = useMachinesStore();
    await store.selectMachine(5);
    await store.bootstrapSelected("UECM:winrm:X");
    expect(mockApi.bootstrapWinrm).toHaveBeenCalledWith(5, "UECM:winrm:X", false);
    expect(store.bootstrapResult?.method).toBe("psexec");
    expect(store.selectedDetail?.machine.status).toBe("online");
  });

  it("loadBootstrapScript stores the manual fallback script", async () => {
    mockApi.getWinrmBootstrapScript.mockResolvedValue("Enable-PSRemoting -Force");
    const store = useMachinesStore();
    await store.loadBootstrapScript();
    expect(store.bootstrapScript).toContain("Enable-PSRemoting");
  });

  it("clears bootstrap state when switching machines", async () => {
    mockApi.getMachineDetail.mockResolvedValueOnce({
      machine: { id: 5, hostname: "X", ip: "1.1.1.1", role: "render", status: "offline", last_seen_at: null },
      ue_installs: [],
      gpus: [],
    });
    mockApi.bootstrapWinrm.mockResolvedValue({
      ok: false,
      method: "psexec",
      message: "ADMIN$ unavailable",
      winrm_ok: false,
      manual_script: "Enable-PSRemoting -Force",
    });
    mockApi.getMachineDetail.mockResolvedValueOnce({
      machine: { id: 5, hostname: "X", ip: "1.1.1.1", role: "render", status: "offline", last_seen_at: null },
      ue_installs: [],
      gpus: [],
    });
    mockApi.getMachineDetail.mockResolvedValueOnce({
      machine: { id: 6, hostname: "Y", ip: "1.1.1.2", role: "render", status: "offline", last_seen_at: null },
      ue_installs: [],
      gpus: [],
    });

    const store = useMachinesStore();
    await store.selectMachine(5);
    await store.bootstrapSelected("UECM:winrm:X");
    expect(store.bootstrapError).toBe("ADMIN$ unavailable");
    expect(store.bootstrapScript).toContain("Enable-PSRemoting");

    await store.selectMachine(6);
    expect(store.bootstrapError).toBeNull();
    expect(store.bootstrapResult).toBeNull();
    expect(store.bootstrapScript).toBeNull();
  });
});
