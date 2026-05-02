import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

const { mockApi, listenMock, captured } = vi.hoisted(() => {
  const captured: { handler: ((e: { payload: unknown }) => void) | null } = {
    handler: null,
  };
  return {
    mockApi: {
      batchSetEnvVar: vi.fn(),
      batchSetIniKey: vi.fn(),
    },
    listenMock: vi.fn(async (_name: string, handler: (e: { payload: unknown }) => void) => {
      captured.handler = handler;
      return () => {
        captured.handler = null;
      };
    }),
    captured,
  };
});

vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import { useBatchStore } from "@/stores/batch";

describe("batch store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockApi.batchSetEnvVar.mockReset();
    mockApi.batchSetIniKey.mockReset();
    listenMock.mockClear();
    captured.handler = null;
  });

  it("starts with no events", () => {
    expect(useBatchStore().events).toEqual([]);
  });

  it("runEnvVar registers listener and dispatches command", async () => {
    mockApi.batchSetEnvVar.mockResolvedValue(undefined);
    const store = useBatchStore();
    await store.runEnvVar([1, 2], "X", "Y", "alias");
    expect(listenMock).toHaveBeenCalledWith("batch-progress", expect.any(Function));
    expect(mockApi.batchSetEnvVar).toHaveBeenCalledWith([1, 2], "X", "Y", "alias");
  });

  it("captures emitted events into events array", async () => {
    mockApi.batchSetEnvVar.mockResolvedValue(undefined);
    const store = useBatchStore();
    await store.runEnvVar([1], "X", "Y", "alias");
    captured.handler!({
      payload: { machine_id: 1, status: "running", message: null },
    });
    captured.handler!({
      payload: { machine_id: 1, status: "ok", message: null },
    });
    expect(store.events).toHaveLength(2);
  });

  it("byMachine returns latest event per machine", async () => {
    mockApi.batchSetEnvVar.mockResolvedValue(undefined);
    const store = useBatchStore();
    await store.runEnvVar([1, 2], "X", "Y", "alias");
    captured.handler!({
      payload: { machine_id: 1, status: "running", message: null },
    });
    captured.handler!({
      payload: { machine_id: 2, status: "running", message: null },
    });
    captured.handler!({
      payload: { machine_id: 1, status: "ok", message: null },
    });
    expect(store.byMachine.get(1)?.status).toBe("ok");
    expect(store.byMachine.get(2)?.status).toBe("running");
  });

  it("reset clears events", async () => {
    mockApi.batchSetEnvVar.mockResolvedValue(undefined);
    const store = useBatchStore();
    await store.runEnvVar([1], "X", "Y", "alias");
    captured.handler!({
      payload: { machine_id: 1, status: "running", message: null },
    });
    expect(store.events).toHaveLength(1);
    store.reset();
    expect(store.events).toEqual([]);
  });
});
