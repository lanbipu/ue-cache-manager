import { describe, it, expect, vi, beforeEach } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const { mockApi, listenMock, handlers } = vi.hoisted(() => {
  const handlers: Record<string, ((e: { payload: unknown }) => void) | undefined> = {};
  return {
    mockApi: {
      generateDdcPak: vi.fn(),
      distributeDdcPak: vi.fn(),
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

import { useDdcPakStore } from "@/stores/ddcPak";

describe("ddc pak store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((fn) => fn.mockReset());
    listenMock.mockClear();
    for (const key of Object.keys(handlers)) delete handlers[key];
  });

  it("starts generate job and attaches listeners", async () => {
    mockApi.generateDdcPak.mockResolvedValue({
      job_id: "gen-1",
      source_machine_id: 1,
      project_id: 2,
      backend: "remote",
    });
    const store = useDdcPakStore();
    await store.startGenerate({
      backend: "remote",
      sourceMachineId: 1,
      projectId: 2,
      localUprojectPath: null,
      localEnginePath: null,
      ueVersion: "5.4",
      operatorCredentialAlias: "cred",
    });
    expect(listenMock).toHaveBeenCalledTimes(3);
    expect(store.generateJobs[0].status).toBe("spawning");
  });

  it("runner events update job state", async () => {
    mockApi.generateDdcPak.mockResolvedValue({
      job_id: "gen-1",
      source_machine_id: 1,
      project_id: 2,
      backend: "remote",
    });
    const store = useDdcPakStore();
    await store.startGenerate({
      backend: "remote",
      sourceMachineId: 1,
      projectId: 2,
      localUprojectPath: null,
      localEnginePath: null,
      ueVersion: null,
      operatorCredentialAlias: null,
    });
    handlers["ue-runner-progress"]!({
      payload: { job_id: "gen-1", source_machine_id: 1, project_id: 2, event: { kind: "spawned", pid: 9, log_path: "x" } },
    });
    handlers["ue-runner-progress"]!({
      payload: { job_id: "gen-1", source_machine_id: 1, project_id: 2, event: { kind: "log_line", text: "hello" } },
    });
    expect(store.generateJobs[0].status).toBe("running");
    expect(store.generateJobs[0].log_lines).toEqual(["hello"]);
  });

  it("pak verified auto-starts pending distribute", async () => {
    mockApi.generateDdcPak.mockResolvedValue({
      job_id: "gen-1",
      source_machine_id: 1,
      project_id: 2,
      backend: "remote",
    });
    mockApi.distributeDdcPak.mockResolvedValue({
      job_id: "dist-1",
      project_id: 2,
      source_machine_id: 1,
      plan: [{ target_machine_id: 3, target_host: "3.3.3.3", source_unc: "\\\\s\\D$", target_local: "D:\\X", credential_user: null, source_smb_user: null }],
    });
    const store = useDdcPakStore();
    await store.startGenerate(
      {
        backend: "remote",
        sourceMachineId: 1,
        projectId: 2,
        localUprojectPath: null,
        localEnginePath: null,
        ueVersion: null,
        operatorCredentialAlias: "cred",
      },
      {
        source_machine_id: 1,
        target_machine_ids: [3],
        named_share_unc: null,
        operator_credential_alias: "cred",
        source_smb_credential_alias: null,
      },
    );
    await store.onPakVerified({
      job_id: "gen-1",
      project_id: 2,
      verified: true,
      output: { path: "D:\\X\\DerivedDataCache\\DDC.ddp", size_bytes: 10 },
    });
    expect(mockApi.distributeDdcPak).toHaveBeenCalled();
    expect(store.distributeJobs[0].job_id).toBe("dist-1");
  });
});
