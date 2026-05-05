import { invoke } from "@tauri-apps/api/core";

export interface Machine {
  id: number | null;
  hostname: string;
  ip: string;
  role: string;
  status: string;
  last_seen_at: string | null;
}

export interface UeInstall {
  id: number | null;
  machine_id: number;
  version: string;
  install_path: string;
  is_primary: boolean;
}

export type GpuVendor = "nvidia" | "amd" | "intel" | "unknown";

export interface GpuInfo {
  id: number | null;
  machine_id: number;
  gpu_model: string;
  driver_version: string;
  vendor: GpuVendor;
  vram_mb: number | null;
}

export interface MachineDetail {
  machine: Machine;
  ue_installs: UeInstall[];
  gpus: GpuInfo[];
}

export interface ProbedHost {
  ip: string;
  winrm_open: boolean;
  smb_open: boolean;
}

export interface ScanResult {
  probed: ProbedHost[];
}

export interface RefreshResult {
  machine_id: number;
  winrm_ok: boolean;
  ue_installs: UeInstall[];
  gpus: GpuInfo[];
  error: string | null;
}

export type CredentialKind = "winrm" | "share";

export interface CredentialRecord {
  id: number | null;
  alias: string;
  kind: CredentialKind;
  username: string;
}

export interface IniKey {
  name: string;
  value: string;
}

export interface WriteIniResponse {
  backup_path: string;
}

export type ShareMode = "open" | "managed";

export interface ShareConfig {
  id: number | null;
  host_machine_id: number;
  share_name: string;
  unc_path: string;
  local_path: string;
  mode: ShareMode;
  credential_alias: string | null;
}

export interface ShareCreateResult {
  share_config_id: number;
  unc_path: string;
  mode: ShareMode;
  credential_alias: string | null;
}

export interface InjectionResult {
  client_machine_id: number;
  ok: boolean;
  message: string;
}

export type BatchStatus = "running" | "ok" | "err";

export interface BatchEvent {
  machine_id: number;
  status: BatchStatus;
  message: string | null;
}

export interface EchoResult {
  received: string;
  timestamp: string;
  machine: string;
}

export interface UecmError {
  code: string;
  message: string;
}

export function formatUecmError(e: unknown): string {
  return typeof e === "string"
    ? e
    : (e as { message?: string })?.message ?? JSON.stringify(e);
}

export type Severity = "critical" | "warning" | "healthy" | "info";
export type Category = "project" | "user" | "engine";
export type RecommendedAction = "set" | "remove" | "manual";

export interface IniFinding {
  id: number | null;
  scan_run_id: number;
  machine_id: number;
  rule_id: string;
  severity: Severity;
  category: Category;
  file_path: string;
  section: string | null;
  key_name: string | null;
  line_number: number | null;
  snippet_before: string;
  snippet_after: string | null;
  recommended_action: RecommendedAction;
  recommended_value: string | null;
  symptom: string;
  rationale: string;
  fixed_at: string | null;
  skipped_at: string | null;
}

export interface ScanRun {
  id: number | null;
  scan_type: "ini" | "health";
  started_at: string | null;
  finished_at: string | null;
  machine_ids: number[];
  summary: Record<string, number> | null;
}

export interface ScanRunSummary {
  scan_run_id: number;
  critical: number;
  warning: number;
  healthy: number;
}

export type HealthStatus = "healthy" | "warning" | "critical" | "na" | "offline" | "unknown";

export interface CheckOutcome {
  status: HealthStatus;
  message: string;
  sample: string;
}

export interface HealthCheckRow {
  scan_run_id: number;
  machine_id: number;
  machine_results: Record<string, CheckOutcome>;
}

export interface HealthRunSummary {
  scan_run_id: number;
  healthy: number;
  warning: number;
  critical: number;
  offline: number;
  total: number;
}

export interface HealthProgressEvent {
  scan_run_id: number;
  machine_id: number;
  done: boolean;
  error: string | null;
}

export const tauriApi = {
  // Machines
  async listMachines(): Promise<Machine[]> {
    return invoke<Machine[]>("list_machines");
  },
  async addMachine(hostname: string, ip: string): Promise<number> {
    return invoke<number>("add_machine", { hostname, ip });
  },
  async deleteMachine(id: number): Promise<void> {
    return invoke<void>("delete_machine", { id });
  },
  async renameMachine(id: number, hostname: string): Promise<void> {
    return invoke<void>("rename_machine", { id, hostname });
  },
  async getMachineDetail(id: number): Promise<MachineDetail> {
    return invoke<MachineDetail>("get_machine_detail", { id });
  },

  // Discovery
  async scanNetwork(cidr: string): Promise<ScanResult> {
    return invoke<ScanResult>("scan_network", { cidr });
  },
  async addDiscoveredMachine(ip: string, hostname: string | null): Promise<number> {
    return invoke<number>("add_discovered_machine", { ip, hostname });
  },
  async refreshMachine(machineId: number): Promise<RefreshResult> {
    return invoke<RefreshResult>("refresh_machine", { machineId });
  },

  // Credentials
  async listCredentials(): Promise<CredentialRecord[]> {
    return invoke<CredentialRecord[]>("list_credentials");
  },
  async saveCredential(
    alias: string,
    kind: CredentialKind,
    username: string,
    password: string,
  ): Promise<number> {
    return invoke<number>("save_credential", { alias, kind, username, password });
  },
  async deleteCredential(alias: string): Promise<void> {
    return invoke<void>("delete_credential", { alias });
  },

  // Env vars
  async setMachineEnvVar(machineId: number, name: string, value: string): Promise<void> {
    return invoke<void>("set_machine_env_var", { machineId, name, value });
  },
  async getMachineEnvVar(machineId: number, name: string): Promise<string | null> {
    return invoke<string | null>("get_machine_env_var", { machineId, name });
  },
  async setMachineEnvVarWithCredential(
    machineId: number,
    name: string,
    value: string,
    credentialAlias: string,
  ): Promise<void> {
    return invoke<void>("set_machine_env_var_with_credential", {
      machineId,
      name,
      value,
      credentialAlias,
    });
  },
  async getMachineEnvVarWithCredential(
    machineId: number,
    name: string,
    credentialAlias: string,
  ): Promise<string | null> {
    return invoke<string | null>("get_machine_env_var_with_credential", {
      machineId,
      name,
      credentialAlias,
    });
  },

  // INI editor
  async readIniSection(
    machineId: number,
    filePath: string,
    section: string,
  ): Promise<IniKey[]> {
    return invoke<IniKey[]>("read_ini_section", { machineId, filePath, section });
  },
  async setIniKey(
    machineId: number,
    filePath: string,
    section: string,
    name: string,
    value: string,
  ): Promise<WriteIniResponse> {
    return invoke<WriteIniResponse>("set_ini_key", {
      machineId,
      filePath,
      section,
      name,
      value,
    });
  },
  async readIniSectionWithCredential(
    machineId: number,
    filePath: string,
    section: string,
    credentialAlias: string,
  ): Promise<IniKey[]> {
    return invoke<IniKey[]>("read_ini_section_with_credential", {
      machineId,
      filePath,
      section,
      credentialAlias,
    });
  },
  async setIniKeyWithCredential(
    machineId: number,
    filePath: string,
    section: string,
    name: string,
    value: string,
    credentialAlias: string,
  ): Promise<WriteIniResponse> {
    return invoke<WriteIniResponse>("set_ini_key_with_credential", {
      machineId,
      filePath,
      section,
      name,
      value,
      credentialAlias,
    });
  },

  // Shares
  async createShare(
    hostMachineId: number,
    mode: ShareMode,
    shareName: string,
    localPath: string,
    operatorCredentialAlias: string | null,
    svcUsername: string | null,
  ): Promise<ShareCreateResult> {
    return invoke<ShareCreateResult>("create_share", {
      hostMachineId,
      mode,
      shareName,
      localPath,
      operatorCredentialAlias,
      svcUsername,
    });
  },
  async injectShareCredentialToClients(
    shareConfigId: number,
    clientMachineIds: number[],
    operatorCredentialAlias: string | null,
  ): Promise<InjectionResult[]> {
    return invoke<InjectionResult[]>("inject_share_credential_to_clients", {
      shareConfigId,
      clientMachineIds,
      operatorCredentialAlias,
    });
  },
  async listShares(): Promise<ShareConfig[]> {
    return invoke<ShareConfig[]>("list_shares");
  },
  async deleteShare(shareConfigId: number, alsoRemoveRemote: boolean): Promise<void> {
    return invoke<void>("delete_share", { shareConfigId, alsoRemoveRemote });
  },

  // Batch
  async batchSetEnvVar(
    machineIds: number[],
    name: string,
    value: string,
    credentialAlias: string,
  ): Promise<void> {
    return invoke<void>("batch_set_env_var", {
      machineIds,
      name,
      value,
      credentialAlias,
    });
  },
  async batchSetIniKey(
    machineIds: number[],
    filePath: string,
    section: string,
    name: string,
    value: string,
    credentialAlias: string,
  ): Promise<void> {
    return invoke<void>("batch_set_ini_key", {
      machineIds,
      filePath,
      section,
      name,
      value,
      credentialAlias,
    });
  },

  // System
  async testPowerShellBridge(message: string): Promise<EchoResult> {
    return invoke<EchoResult>("test_powershell_bridge", { message });
  },

  // Diagnostics — INI scanner
  async scanInis(
    machineIds: number[],
    projectPathsPerMachine: Record<number, string[]>,
    userProfile: string,
    credentialAlias: string,
  ): Promise<ScanRunSummary> {
    return invoke<ScanRunSummary>("scan_inis", {
      machineIds, projectPathsPerMachine, userProfile, credentialAlias,
    });
  },
  async listFindingsForRun(scanRunId: number): Promise<IniFinding[]> {
    return invoke<IniFinding[]>("list_findings_for_run", { scanRunId });
  },
  async listRecentIniRuns(limit: number): Promise<ScanRun[]> {
    return invoke<ScanRun[]>("list_recent_ini_runs", { limit });
  },
  async applyFinding(findingId: number, credentialAlias: string): Promise<string> {
    return invoke<string>("apply_finding", { findingId, credentialAlias });
  },
  async skipFinding(findingId: number): Promise<void> {
    return invoke<void>("skip_finding", { findingId });
  },

  // Diagnostics — Health check
  async runHealthCheck(
    machineIds: number[],
    projectPathsPerMachine: Record<number, string[]>,
    credentialAlias: string,
  ): Promise<HealthRunSummary> {
    return invoke<HealthRunSummary>("run_health_check", {
      machineIds, projectPathsPerMachine, credentialAlias,
    });
  },
  async listRecentHealthRuns(limit: number): Promise<ScanRun[]> {
    return invoke<ScanRun[]>("list_recent_health_runs", { limit });
  },
  async listHealthResultsForRun(scanRunId: number): Promise<HealthCheckRow[]> {
    return invoke<HealthCheckRow[]>("list_health_results_for_run", { scanRunId });
  },
};
