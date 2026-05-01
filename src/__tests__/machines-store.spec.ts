import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

// vi.hoisted ensures mockApi is initialized before vi.mock factory runs
const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn(),
    addMachine: vi.fn(),
    deleteMachine: vi.fn(),
    getMachineDetail: vi.fn(),
    refreshMachine: vi.fn(),
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
    mockApi.getMachineDetail.mockReset();
    mockApi.refreshMachine.mockReset();
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
});
