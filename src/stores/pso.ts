import { defineStore } from "pinia";
import { ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  tauriApi,
  type PakDistributeProgressPayload,
  type PsoCacheFile,
  type PsoCollectFinalizedPayload,
  type PsoCollectJobResponse,
  type PsoDistributeJobResponse,
  type UecmError,
  type UeRunnerProgressPayload,
} from "@/services/tauri";

export interface CollectJobState {
  job_id: string;
  source_machine_id: number;
  project_id: number;
  status:
    | "queued"
    | "spawning"
    | "collecting"
    | "completing"
    | "completed"
    | "cancelled"
    | "error";
  log_lines: string[];
  files_collected: number | null;
  error_message: string | null;
  started_at: string;
  finished_at: string | null;
}

export interface PsoDistributeTargetState {
  target_machine_id: number;
  target_host: string;
  status: "pending" | "running" | "ok" | "err";
  message: string | null;
}

export interface PsoDistributeJobState {
  job_id: string;
  file_id: number;
  status: "queued" | "running" | "completed";
  targets: PsoDistributeTargetState[];
  started_at: string;
  finished_at: string | null;
}

export const usePsoStore = defineStore("pso", () => {
  const cacheFilesByProject = ref<Record<number, PsoCacheFile[]>>({});
  const collectJobs = ref<CollectJobState[]>([]);
  const distributeJobs = ref<PsoDistributeJobState[]>([]);
  const error = ref<UecmError | null>(null);

  let unlistenRunner: UnlistenFn | null = null;
  let unlistenFinalized: UnlistenFn | null = null;
  let unlistenDistribute: UnlistenFn | null = null;

  async function attach() {
    if (unlistenRunner) return;
    unlistenRunner = await listen<UeRunnerProgressPayload>("ue-runner-progress", (event) => {
      onRunnerEvent(event.payload);
    });
    unlistenFinalized = await listen<PsoCollectFinalizedPayload>(
      "pso-collect-finalized",
      (event) => {
        void onFinalized(event.payload);
      },
    );
    unlistenDistribute = await listen<PakDistributeProgressPayload>(
      "pak-distribute-progress",
      (event) => {
        onDistributeEvent(event.payload);
      },
    );
  }

  async function detach() {
    unlistenRunner?.();
    unlistenFinalized?.();
    unlistenDistribute?.();
    unlistenRunner = null;
    unlistenFinalized = null;
    unlistenDistribute = null;
  }

  function onRunnerEvent(payload: UeRunnerProgressPayload) {
    const job = collectJobs.value.find((item) => item.job_id === payload.job_id);
    if (!job) return;
    const event = payload.event;
    switch (event.kind) {
      case "spawned":
        job.status = "collecting";
        break;
      case "log_line":
        if (event.text) {
          job.log_lines.push(event.text);
          if (job.log_lines.length > 200) {
            job.log_lines.splice(0, job.log_lines.length - 200);
          }
        }
        break;
      case "completed":
      case "cancelled":
        job.status = "completing";
        break;
      case "error":
        job.status = "error";
        job.error_message = event.message ?? "unknown error";
        job.finished_at = new Date().toISOString();
        break;
      default:
        break;
    }
  }

  async function onFinalized(payload: PsoCollectFinalizedPayload) {
    const job = collectJobs.value.find((item) => item.job_id === payload.job_id);
    if (!job) return;
    if (payload.error_message) {
      job.status = "error";
      job.error_message = payload.error_message;
    } else {
      job.status = "completed";
      job.files_collected = payload.files_collected ?? 0;
    }
    job.finished_at = new Date().toISOString();
    if (!payload.error_message) {
      await loadFiles(payload.project_id);
    }
  }

  function onDistributeEvent(payload: PakDistributeProgressPayload) {
    const job = distributeJobs.value.find((item) => item.job_id === payload.job_id);
    if (!job) return;
    const target = job.targets.find(
      (item) => item.target_machine_id === payload.event.machine_id,
    );
    if (!target) return;
    target.status = payload.event.status;
    target.message = payload.event.message;
    if (job.targets.every((item) => item.status === "ok" || item.status === "err")) {
      job.status = "completed";
      job.finished_at = new Date().toISOString();
    }
  }

  async function loadFiles(projectId: number) {
    error.value = null;
    try {
      const files = await tauriApi.listPsoCacheFiles(projectId);
      cacheFilesByProject.value = { ...cacheFilesByProject.value, [projectId]: files };
    } catch (e) {
      error.value = e as UecmError;
    }
  }

  async function startCollection(args: {
    sourceMachineId: number;
    projectId: number;
    ueVersion: string | null;
    resolutionW: number;
    resolutionH: number;
    windowed: boolean;
    maxMinutes: number;
    operatorCredentialAlias: string | null;
  }): Promise<PsoCollectJobResponse> {
    await attach();
    error.value = null;
    const response = await tauriApi.startPsoCollection(args);
    collectJobs.value.unshift({
      job_id: response.job_id,
      source_machine_id: response.source_machine_id,
      project_id: response.project_id,
      status: "spawning",
      log_lines: [],
      files_collected: null,
      error_message: null,
      started_at: new Date().toISOString(),
      finished_at: null,
    });
    return response;
  }

  async function cancelCollection(jobId: string): Promise<boolean> {
    const cancelled = await tauriApi.cancelUeJob(jobId);
    if (cancelled) {
      const job = collectJobs.value.find((item) => item.job_id === jobId);
      if (job) job.status = "completing";
    }
    return cancelled;
  }

  async function startDistribute(args: {
    fileId: number;
    targetMachineIds: number[];
    namedShareUnc: string | null;
    operatorCredentialAlias: string | null;
    sourceSmbCredentialAlias: string | null;
    forceGpuMismatch: boolean;
  }): Promise<PsoDistributeJobResponse> {
    await attach();
    error.value = null;
    const response = await tauriApi.distributePsoCache(args);
    distributeJobs.value.unshift({
      job_id: response.job_id,
      file_id: args.fileId,
      status: "running",
      targets: response.plan.map((item) => ({
        target_machine_id: item.target_machine_id,
        target_host: item.target_host,
        status: "pending",
        message: null,
      })),
      started_at: new Date().toISOString(),
      finished_at: null,
    });
    return response;
  }

  return {
    cacheFilesByProject,
    collectJobs,
    distributeJobs,
    error,
    attach,
    detach,
    loadFiles,
    startCollection,
    cancelCollection,
    startDistribute,
    onRunnerEvent,
    onFinalized,
    onDistributeEvent,
  };
});
