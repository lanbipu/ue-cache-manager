import { describe, it, expect, vi, beforeEach } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const { mockApi, listenMock, handlers } = vi.hoisted(() => {
  const handlers: Record<string, ((e: { payload: unknown }) => void) | undefined> = {};
  return {
    mockApi: {
      startPsoCollection: vi.fn(),
      listPsoCacheFiles: vi.fn(),
      distributePsoCache: vi.fn(),
      cancelUeJob: vi.fn(),
    },
    listenMock: vi.fn(async (name: string, handler: (e: { payload: unknown }) => void) => {
      handlers[name] = handler;
      return () => {
        delete handlers[name];
      };
    }),
    handlers,
  };
});

vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import { usePsoStore } from "@/stores/pso";

describe("pso store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((fn) => fn.mockReset());
    listenMock.mockClear();
    for (const key of Object.keys(handlers)) delete handlers[key];
  });

  it("starts a collection job and attaches listeners", async () => {
    mockApi.startPsoCollection.mockResolvedValue({
      job_id: "pso-1",
      source_machine_id: 1,
      project_id: 10,
    });
    const store = usePsoStore();
    await store.startCollection({
      sourceMachineId: 1,
      projectId: 10,
      ueVersion: "5.4",
      resolutionW: 1920,
      resolutionH: 1080,
      windowed: true,
      maxMinutes: 10,
      operatorCredentialAlias: "cred",
    });
    expect(listenMock).toHaveBeenCalledTimes(3);
    expect(mockApi.startPsoCollection).toHaveBeenCalledWith({
      sourceMachineId: 1,
      projectId: 10,
      ueVersion: "5.4",
      resolutionW: 1920,
      resolutionH: 1080,
      windowed: true,
      maxMinutes: 10,
      operatorCredentialAlias: "cred",
    });
    expect(store.collectJobs[0].status).toBe("spawning");
  });

  it("runner events update collection state", async () => {
    mockApi.startPsoCollection.mockResolvedValue({
      job_id: "pso-1",
      source_machine_id: 1,
      project_id: 10,
    });
    const store = usePsoStore();
    await store.startCollection({
      sourceMachineId: 1,
      projectId: 10,
      ueVersion: null,
      resolutionW: 640,
      resolutionH: 360,
      windowed: true,
      maxMinutes: 5,
      operatorCredentialAlias: null,
    });
    handlers["ue-runner-progress"]!({
      payload: {
        job_id: "pso-1",
        source_machine_id: 1,
        project_id: 10,
        event: { kind: "spawned", pid: 9, log_path: "x" },
      },
    });
    handlers["ue-runner-progress"]!({
      payload: {
        job_id: "pso-1",
        source_machine_id: 1,
        project_id: 10,
        event: { kind: "log_line", text: "LogShaderPipelineCache: collecting" },
      },
    });
    expect(store.collectJobs[0].status).toBe("collecting");
    expect(store.collectJobs[0].log_lines).toEqual(["LogShaderPipelineCache: collecting"]);
  });

  it("finalized event reloads cache files", async () => {
    mockApi.startPsoCollection.mockResolvedValue({
      job_id: "pso-1",
      source_machine_id: 1,
      project_id: 10,
    });
    mockApi.listPsoCacheFiles.mockResolvedValue([
      {
        id: 1,
        project_id: 10,
        source_machine_id: 1,
        file_path: "D:\\Proj\\Saved\\CollectedPSOs\\x.upipelinecache",
        file_name: "x.upipelinecache",
        size_bytes: 100,
        gpu_signature: "nvidia:RTX 4090:551.86",
        ue_version: "5.4",
        collected_at: null,
      },
    ]);
    const store = usePsoStore();
    await store.startCollection({
      sourceMachineId: 1,
      projectId: 10,
      ueVersion: null,
      resolutionW: 640,
      resolutionH: 360,
      windowed: true,
      maxMinutes: 5,
      operatorCredentialAlias: null,
    });
    await store.onFinalized({
      job_id: "pso-1",
      source_machine_id: 1,
      project_id: 10,
      files_collected: 1,
    });
    expect(store.collectJobs[0].status).toBe("completed");
    expect(store.cacheFilesByProject[10]).toHaveLength(1);
  });

  it("starts distribution and ignores unrelated progress events", async () => {
    mockApi.distributePsoCache.mockResolvedValue({
      job_id: "dist-1",
      plan: [
        {
          target_machine_id: 2,
          target_host: "192.168.10.22",
          source_unc: "\\\\SOURCE\\PSO",
          target_local: "D:\\Proj\\Saved\\CollectedPSOs",
          file_name: "x.upipelinecache",
          credential_user: null,
          source_smb_user: null,
        },
      ],
    });
    const store = usePsoStore();
    await store.startDistribute({
      fileId: 1,
      targetMachineIds: [2],
      namedShareUnc: null,
      operatorCredentialAlias: null,
      sourceSmbCredentialAlias: null,
      forceGpuMismatch: false,
    });
    handlers["pak-distribute-progress"]!({
      payload: {
        job_id: "ddc-job",
        project_id: 10,
        source_machine_id: 1,
        event: { machine_id: 2, status: "ok", message: null },
      },
    });
    expect(store.distributeJobs[0].targets[0].status).toBe("pending");
    handlers["pak-distribute-progress"]!({
      payload: {
        job_id: "dist-1",
        project_id: 10,
        source_machine_id: 1,
        event: { machine_id: 2, status: "ok", message: "copied" },
      },
    });
    expect(store.distributeJobs[0].status).toBe("completed");
    expect(store.distributeJobs[0].targets[0].message).toBe("copied");
  });
});
