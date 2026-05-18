import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type DeployStep =
  | "provision_local_dir"
  | "set_local_env"
  | "create_smb_share"
  | "set_shared_env"
  | "write_backend_graph"
  | "generate_ddc_pak"
  | "distribute_ddc_pak"
  | "set_pso_cvars"
  | "collect_pso"
  | "distribute_pso"
  | "verify_startup_logs";

export interface DeployPlan {
  project_id: number;
  source_machine_id: number;
  target_machine_ids: number[];
  local_cache: { path: string; service_account: string | null };
  shared_cache: {
    server_machine_id: number;
    share_name: string;
    server_path: string;
    mode: string;
    unc_path: string | null;
  };
  ddc_pak: { enabled: boolean };
  pso: { enabled: boolean; resolution: string; max_minutes: number };
  verify: { run_log_verify: boolean; editor_exe: string; timeout_seconds: number };
}

export type DeployEvent =
  | { kind: "step_started"; step: DeployStep; hosts: string[] }
  | { kind: "step_host_ok"; step: DeployStep; host: string; message: string | null }
  | { kind: "step_host_error"; step: DeployStep; host: string; error: string }
  | { kind: "step_completed"; step: DeployStep; ok_count: number; fail_count: number }
  | { kind: "plan_completed"; ok: boolean; summary: string };

export function previewPlan(plan: DeployPlan): Promise<DeployStep[]> {
  return invoke("deploy_ddc_plan_preview", { plan });
}

export function runPlan(
  plan: DeployPlan,
  credentialAlias: string | null,
  stopOnFailure: boolean,
): Promise<void> {
  return invoke("deploy_ddc_run", { plan, credentialAlias, stopOnFailure });
}

export function subscribe(onEvent: (e: DeployEvent) => void): Promise<UnlistenFn> {
  return listen<DeployEvent>("deploy-event", (e) => onEvent(e.payload));
}
