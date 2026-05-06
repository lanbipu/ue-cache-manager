import { defineStore } from "pinia";
import { ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  tauriApi,
  type BackendChoice,
  type DistributeJobResponse,
  type PakDistributeProgressPayload,
  type PakOutput,
  type PakVerifiedPayload,
  type UecmError,
  type UeRunnerProgressPayload,
} from "@/services/tauri";

export interface PendingDistribute {
  source_machine_id: number;
  target_machine_ids: number[];
  named_share_unc: string | null;
  operator_credential_alias: string | null;
  source_smb_credential_alias: string | null;
}

export interface GenerateJobState {
  job_id: string;
  source_machine_id: number;
  project_id: number;
  backend: BackendChoice;
  status:
    | "queued"
    | "spawning"
    | "running"
    | "verifying"
    | "completed"
    | "verify_failed"
    | "cancelled"
    | "error";
  log_lines: string[];
  progress_pct: number | null;
  progress_label: string | null;
  exit_code: number | null;
  error_message: string | null;
  output: PakOutput | null;
  pending_distribute: PendingDistribute | null;
  started_at: string;
  finished_at: string | null;
}

export interface DistributeTargetState {
  target_machine_id: number;
  target_host: string;
  status: "pending" | "running" | "ok" | "err";
  message: string | null;
}

export interface DistributeJobState {
  job_id: string;
  project_id: number;
  source_machine_id: number;
  status: "queued" | "running" | "completed";
  targets: DistributeTargetState[];
  started_at: string;
  finished_at: string | null;
}

export const useDdcPakStore = defineStore("ddcPak", () => {
  const generateJobs = ref<GenerateJobState[]>([]);
  const distributeJobs = ref<DistributeJobState[]>([]);
  const error = ref<UecmError | null>(null);

  let unlistenRunner: UnlistenFn | null = null;
  let unlistenVerified: UnlistenFn | null = null;
  let unlistenDistribute: UnlistenFn | null = null;

  async function attach() {
    if (unlistenRunner) return;
    unlistenRunner = await listen<UeRunnerProgressPayload>("ue-runner-progress", (event) => {
      onUeRunnerEvent(event.payload);
    });
    unlistenVerified = await listen<PakVerifiedPayload>("pak-verified", (event) => {
      void onPakVerified(event.payload);
    });
    unlistenDistribute = await listen<PakDistributeProgressPayload>(
      "pak-distribute-progress",
      (event) => {
        onDistributeEvent(event.payload);
      },
    );
  }

  async function detach() {
    unlistenRunner?.();
    unlistenVerified?.();
    unlistenDistribute?.();
    unlistenRunner = null;
    unlistenVerified = null;
    unlistenDistribute = null;
  }

  function onUeRunnerEvent(payload: UeRunnerProgressPayload) {
    const job = generateJobs.value.find((item) => item.job_id === payload.job_id);
    if (!job) return;
    const event = payload.event;
    switch (event.kind) {
      case "spawned":
        job.status = "running";
        break;
      case "log_line":
        if (event.text) {
          job.log_lines.push(event.text);
          if (job.log_lines.length > 200) {
            job.log_lines.splice(0, job.log_lines.length - 200);
          }
        }
        break;
      case "progress":
        job.progress_pct = event.pct ?? job.progress_pct;
        job.progress_label = event.label ?? job.progress_label;
        break;
      case "completed":
        job.status = "verifying";
        job.exit_code = event.exit_code ?? null;
        break;
      case "cancelled":
        job.status = "cancelled";
        job.pending_distribute = null;
        job.finished_at = new Date().toISOString();
        break;
      case "error":
        job.status = "error";
        job.error_message = event.message ?? "unknown error";
        job.pending_distribute = null;
        job.finished_at = new Date().toISOString();
        break;
    }
  }

  async function onPakVerified(payload: PakVerifiedPayload) {
    const job = generateJobs.value.find((item) => item.job_id === payload.job_id);
    if (!job) return;
    if (payload.verified) {
      job.status = "completed";
      job.output = payload.output;
    } else {
      job.status = "verify_failed";
      job.error_message = "pak verification failed";
      job.pending_distribute = null;
    }
    job.finished_at = new Date().toISOString();

    if (payload.verified && job.pending_distribute) {
      const pending = job.pending_distribute;
      job.pending_distribute = null;
      await startDistribute({
        sourceMachineId: pending.source_machine_id,
        projectId: job.project_id,
        targetMachineIds: pending.target_machine_ids,
        namedShareUnc: pending.named_share_unc,
        operatorCredentialAlias: pending.operator_credential_alias,
        sourceSmbCredentialAlias: pending.source_smb_credential_alias,
      });
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

  async function startGenerate(
    args: {
      backend: BackendChoice;
      sourceMachineId: number | null;
      projectId: number;
      localUprojectPath: string | null;
      localEnginePath: string | null;
      ueVersion: string | null;
      operatorCredentialAlias: string | null;
    },
    pendingDistribute: PendingDistribute | null = null,
  ) {
    await attach();
    error.value = null;
    const response = await tauriApi.generateDdcPak(args);
    generateJobs.value.unshift({
      job_id: response.job_id,
      source_machine_id: response.source_machine_id,
      project_id: response.project_id,
      backend: response.backend,
      status: "spawning",
      log_lines: [],
      progress_pct: null,
      progress_label: null,
      exit_code: null,
      error_message: null,
      output: null,
      pending_distribute: pendingDistribute,
      started_at: new Date().toISOString(),
      finished_at: null,
    });
    return response;
  }

  async function startDistribute(args: {
    sourceMachineId: number;
    projectId: number;
    targetMachineIds: number[];
    namedShareUnc: string | null;
    operatorCredentialAlias: string | null;
    sourceSmbCredentialAlias: string | null;
  }): Promise<DistributeJobResponse> {
    await attach();
    error.value = null;
    const response = await tauriApi.distributeDdcPak(args);
    distributeJobs.value.unshift({
      job_id: response.job_id,
      project_id: response.project_id,
      source_machine_id: response.source_machine_id,
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

  async function cancel(jobId: string) {
    const cancelled = await tauriApi.cancelUeJob(jobId);
    if (cancelled) {
      const job = generateJobs.value.find((item) => item.job_id === jobId);
      if (job) {
        job.status = "cancelled";
        job.pending_distribute = null;
        job.finished_at = new Date().toISOString();
      }
    }
    return cancelled;
  }

  return {
    generateJobs,
    distributeJobs,
    error,
    attach,
    detach,
    startGenerate,
    startDistribute,
    cancel,
    onUeRunnerEvent,
    onPakVerified,
    onDistributeEvent,
  };
});
