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
  /** TCP 135 (DCE/RPC EPM) — required by PsExec-based Path B remote bootstrap. Optional for backward-compat with older fixtures. */
  rpc_open?: boolean;
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

export interface WinrmBootstrapResult {
  ok: boolean;
  method: string;
  message: string;
  winrm_ok: boolean;
  changed: string[];
  manual_script: string | null;
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

export type DiscoveryStatus = "auto" | "manual_alias" | "manual_path";

export interface ProjectSummary {
  id: number;
  uproject_name: string;
  display_name: string | null;
  uproject_guid: string | null;
  location_count: number;
}

export interface ProjectLocation {
  id: number | null;
  project_id: number;
  machine_id: number;
  abs_path: string;
  uproject_path: string;
  discovery_status: DiscoveryStatus;
  discovered_at: string | null;
}

export interface DiscoveryResult {
  project_id: number;
  location_id: number;
  uproject_filename: string;
  abs_path: string;
}

export type UeRunnerEventKind =
  | "spawned"
  | "log_line"
  | "progress"
  | "completed"
  | "cancelled"
  | "error";

export interface UeRunnerEvent {
  kind: UeRunnerEventKind;
  pid?: number;
  log_path?: string;
  text?: string;
  parsed_kind?: string | null;
  pct?: number | null;
  label?: string;
  exit_code?: number;
  log_tail?: string[];
  message?: string;
}

export interface UeRunnerProgressPayload {
  job_id: string;
  source_machine_id: number;
  project_id: number;
  event: UeRunnerEvent;
}

export type BackendChoice = "remote" | "local";

export interface GenerateJobResponse {
  job_id: string;
  source_machine_id: number;
  project_id: number;
  backend: BackendChoice;
}

export interface PakOutput {
  path: string;
  size_bytes: number;
}

export interface PakVerifiedPayload {
  job_id: string;
  project_id: number;
  verified: boolean;
  output: PakOutput | null;
}

export interface DistributePlanItem {
  target_machine_id: number;
  target_host: string;
  source_unc: string;
  target_local: string;
  credential_user: string | null;
  source_smb_user: string | null;
}

export interface DistributeJobResponse {
  job_id: string;
  project_id: number;
  source_machine_id: number;
  plan: DistributePlanItem[];
}

export interface PakDistributeProgressPayload {
  job_id: string;
  project_id: number;
  source_machine_id: number;
  event: BatchEvent;
}

export interface PsoCacheFile {
  id: number | null;
  project_id: number;
  source_machine_id: number;
  file_path: string;
  file_name: string;
  size_bytes: number;
  gpu_signature: string;
  ue_version: string | null;
  collected_at: string | null;
}

export interface PsoCollectJobResponse {
  job_id: string;
  source_machine_id: number;
  project_id: number;
}

export interface PsoDistributePlanItem {
  target_machine_id: number;
  target_host: string;
  source_unc: string;
  target_local: string;
  file_name: string;
  credential_user: string | null;
  source_smb_user: string | null;
}

export interface PsoDistributeJobResponse {
  job_id: string;
  plan: PsoDistributePlanItem[];
}

export interface PsoCollectFinalizedPayload {
  job_id: string;
  source_machine_id: number;
  project_id: number;
  files_collected: number | null;
  error_message?: string | null;
}

export interface GpuSignature {
  vendor: string;
  model: string;
  driver: string;
}

export type CellStatus = "match" | "deviation" | "unknown";

export interface GpuSignatureCount {
  signature: GpuSignature;
  count: number;
}

export interface MachineGpuCell {
  machine_id: number;
  hostname: string;
  signature: GpuSignature | null;
  status: CellStatus;
}

export interface GpuMatrix {
  signatures: GpuSignatureCount[];
  baseline: GpuSignature | null;
  cells: MachineGpuCell[];
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
export type FindingCategory = Category | "PSO";
export type RecommendedAction = "set" | "remove" | "manual";

export interface IniFinding {
  id: number | null;
  scan_run_id: number;
  machine_id: number;
  rule_id: string;
  severity: Severity;
  category: FindingCategory;
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

export interface IniScanSummary {
  scan_run_id: number;
  critical: number;
  warning: number;
  healthy: number;
  info: number;
  total_files: number;
}

export interface ScanInisRequest {
  machine_ids: number[];
  credential_alias: string;
  project_paths: string[];
  user_profile_path: string | null;
}

export interface ScanInisResponse {
  scan_run_id: number;
  summary: IniScanSummary;
  findings: IniFinding[];
}

export interface ApplyFindingResult {
  backup_path: string | null;
  message: string;
}

export type HealthStatus = "healthy" | "warning" | "critical" | "na" | "offline" | "unknown";

export interface CheckOutcome {
  status: HealthStatus;
  message: string;
  sample: string;
  remediation?: string;
}

export interface HealthCheckRow {
  id?: number | null;
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

export type HealthCheckRun = HealthCheckRow;

export interface RunHealthCheckRequest {
  machine_ids: number[];
  credential_alias: string;
  project_paths: string[];
}

export interface RunHealthCheckResponse {
  scan_run_id: number;
  results: HealthCheckRun[];
}

export interface HealthProgressEvent {
  scan_run_id: number;
  machine_id: number;
  done: boolean;
  error: string | null;
}

const demoMachines: Machine[] = [
  {
    id: 101,
    hostname: "STAGE-CONTROL",
    ip: "192.168.10.10",
    role: "operator",
    status: "online",
    last_seen_at: "2026-05-16T19:52:14+08:00",
  },
  {
    id: 102,
    hostname: "RENDER-01",
    ip: "192.168.10.21",
    role: "render",
    status: "online",
    last_seen_at: "2026-05-16T19:51:48+08:00",
  },
  {
    id: 103,
    hostname: "RENDER-02",
    ip: "192.168.10.22",
    role: "render",
    status: "warning",
    last_seen_at: "2026-05-16T19:47:02+08:00",
  },
  {
    id: 104,
    hostname: "RENDER-03",
    ip: "192.168.10.23",
    role: "render",
    status: "critical",
    last_seen_at: "2026-05-16T18:33:41+08:00",
  },
  {
    id: 105,
    hostname: "VP-WORKSTATION",
    ip: "192.168.10.31",
    role: "artist",
    status: "online",
    last_seen_at: "2026-05-16T19:49:09+08:00",
  },
  {
    id: 106,
    hostname: "NAS-DDC",
    ip: "192.168.10.40",
    role: "cache-host",
    status: "degraded",
    last_seen_at: "2026-05-16T19:10:25+08:00",
  },
  {
    id: 107,
    hostname: "COLOR-GRADE",
    ip: "192.168.10.55",
    role: "review",
    status: "offline",
    last_seen_at: "2026-05-15T23:18:06+08:00",
  },
  {
    id: 108,
    hostname: "LED-PROCESSOR",
    ip: "192.168.10.60",
    role: "stage",
    status: "unknown",
    last_seen_at: null,
  },
];

const demoDetails: Record<number, MachineDetail> = Object.fromEntries(
  demoMachines.map((machine, index) => [
    machine.id!,
    {
      machine,
      ue_installs:
        machine.status === "offline" || machine.status === "unknown"
          ? []
          : [
              {
                id: machine.id! * 10 + 1,
                machine_id: machine.id!,
                version: index % 2 === 0 ? "5.4" : "5.3",
                install_path: index % 2 === 0 ? "C:\\Program Files\\Epic Games\\UE_5.4" : "D:\\Epic\\UE_5.3",
                is_primary: true,
              },
              {
                id: machine.id! * 10 + 2,
                machine_id: machine.id!,
                version: "5.2",
                install_path: "D:\\Epic\\UE_5.2",
                is_primary: false,
              },
            ],
      gpus:
        machine.status === "offline" || machine.status === "unknown"
          ? []
          : [
              {
                id: machine.id! * 10 + 3,
                machine_id: machine.id!,
                gpu_model: index === 0 ? "NVIDIA RTX A6000" : "NVIDIA RTX 4090",
                driver_version: index === 3 ? "551.86" : "552.44",
                vendor: "nvidia",
                vram_mb: index === 0 ? 49152 : 24576,
              },
            ],
    },
  ]),
) as Record<number, MachineDetail>;

function isPlainBrowserDev() {
  if (typeof window === "undefined") return false;
  const tauriWindow = window as Window & { __TAURI_INTERNALS__?: unknown };
  return import.meta.env.DEV && import.meta.env.MODE !== "test" && !tauriWindow.__TAURI_INTERNALS__;
}

export const tauriApi = {
  // Machines
  async listMachines(): Promise<Machine[]> {
    if (isPlainBrowserDev()) return demoMachines;
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
    if (isPlainBrowserDev()) {
      const detail = demoDetails[id];
      if (detail) return detail;
    }
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
    if (isPlainBrowserDev()) {
      const detail = demoDetails[machineId];
      return {
        machine_id: machineId,
        winrm_ok: detail?.machine.status === "online",
        ue_installs: detail?.ue_installs ?? [],
        gpus: detail?.gpus ?? [],
        error: detail?.machine.status === "online" ? null : "Demo host is not reachable.",
      };
    }
    return invoke<RefreshResult>("refresh_machine", { machineId });
  },
  async bootstrapWinrm(
    machineId: number,
    credentialAlias: string,
    enableLocalAccountRemoteAdmin = false,
  ): Promise<WinrmBootstrapResult> {
    return invoke<WinrmBootstrapResult>("bootstrap_winrm", {
      machineId,
      credentialAlias,
      enableLocalAccountRemoteAdmin,
    });
  },
  async getWinrmBootstrapScript(): Promise<string> {
    return invoke<string>("get_winrm_bootstrap_script");
  },

  // Credentials
  async listCredentials(): Promise<CredentialRecord[]> {
    if (isPlainBrowserDev()) {
      return [
        {
          id: 1,
          alias: "UECM:winrm:stage-admin",
          kind: "winrm",
          username: "stage-admin",
        },
      ];
    }
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

  // Projects
  async listProjects(): Promise<ProjectSummary[]> {
    return invoke<ProjectSummary[]>("list_projects");
  },
  async listProjectLocations(projectId: number): Promise<ProjectLocation[]> {
    return invoke<ProjectLocation[]>("list_project_locations", { projectId });
  },
  async discoverProjects(
    machineId: number,
    searchRoots: string[],
    operatorCredentialAlias: string | null,
  ): Promise<DiscoveryResult[]> {
    return invoke<DiscoveryResult[]>("discover_projects", {
      machineId,
      searchRoots,
      operatorCredentialAlias,
    });
  },
  async setProjectLocation(
    projectId: number,
    machineId: number,
    absPath: string,
    uprojectPath: string,
    manual: boolean,
  ): Promise<number> {
    return invoke<number>("set_project_location", {
      projectId,
      machineId,
      absPath,
      uprojectPath,
      manual,
    });
  },
  async deleteProject(projectId: number): Promise<void> {
    return invoke<void>("delete_project", { projectId });
  },
  async deleteProjectLocation(locationId: number): Promise<void> {
    return invoke<void>("delete_project_location", { locationId });
  },
  async createProjectManual(
    uprojectName: string,
    displayName: string | null,
  ): Promise<number> {
    return invoke<number>("create_project_manual", { uprojectName, displayName });
  },

  // DDC Pak
  async generateDdcPak(args: {
    backend: BackendChoice;
    sourceMachineId: number | null;
    projectId: number;
    localUprojectPath: string | null;
    localEnginePath: string | null;
    ueVersion: string | null;
    operatorCredentialAlias: string | null;
  }): Promise<GenerateJobResponse> {
    return invoke<GenerateJobResponse>("generate_ddc_pak", {
      backend: args.backend,
      sourceMachineId: args.sourceMachineId,
      projectId: args.projectId,
      localUprojectPath: args.localUprojectPath,
      localEnginePath: args.localEnginePath,
      ueVersion: args.ueVersion,
      operatorCredentialAlias: args.operatorCredentialAlias,
    });
  },
  async cancelUeJob(jobId: string): Promise<boolean> {
    return invoke<boolean>("cancel_ue_job", { jobId });
  },
  async verifyPakOutput(
    machineId: number,
    projectId: number,
    operatorCredentialAlias: string | null,
  ): Promise<PakOutput> {
    return invoke<PakOutput>("verify_pak_output", {
      machineId,
      projectId,
      operatorCredentialAlias,
    });
  },
  async distributeDdcPak(args: {
    sourceMachineId: number;
    projectId: number;
    targetMachineIds: number[];
    namedShareUnc: string | null;
    operatorCredentialAlias: string | null;
    sourceSmbCredentialAlias: string | null;
  }): Promise<DistributeJobResponse> {
    return invoke<DistributeJobResponse>("distribute_ddc_pak", {
      sourceMachineId: args.sourceMachineId,
      projectId: args.projectId,
      targetMachineIds: args.targetMachineIds,
      namedShareUnc: args.namedShareUnc,
      operatorCredentialAlias: args.operatorCredentialAlias,
      sourceSmbCredentialAlias: args.sourceSmbCredentialAlias,
    });
  },

  // System
  async testPowerShellBridge(message: string): Promise<EchoResult> {
    return invoke<EchoResult>("test_powershell_bridge", { message });
  },

  // Diagnostics — INI scanner
  async scanInis(request: ScanInisRequest): Promise<ScanInisResponse> {
    return invoke<ScanInisResponse>("scan_inis", { request });
  },
  async listFindings(scanRunId: number): Promise<IniFinding[]> {
    return invoke<IniFinding[]>("list_findings_for_run", { scanRunId });
  },
  async listFindingsForRun(scanRunId: number): Promise<IniFinding[]> {
    return this.listFindings(scanRunId);
  },
  async listScanRuns(scanType: string, limit = 20): Promise<ScanRun[]> {
    if (scanType === "health") {
      return invoke<ScanRun[]>("list_recent_health_runs", { limit });
    }
    return invoke<ScanRun[]>("list_recent_ini_runs", { limit });
  },
  async listRecentIniRuns(limit: number): Promise<ScanRun[]> {
    return this.listScanRuns("ini", limit);
  },
  async applyFinding(findingId: number, credentialAlias: string): Promise<ApplyFindingResult> {
    const backupPath = await invoke<string>("apply_finding", { findingId, credentialAlias });
    return { backup_path: backupPath, message: "applied" };
  },
  async skipFinding(findingId: number): Promise<void> {
    return invoke<void>("skip_finding", { findingId });
  },

  // Diagnostics — Health check
  async runHealthCheck(request: RunHealthCheckRequest): Promise<RunHealthCheckResponse> {
    const response = await invoke<RunHealthCheckResponse | HealthRunSummary>("run_health_check", {
      request,
    });
    if ("results" in response) {
      return response;
    }
    return {
      scan_run_id: response.scan_run_id,
      results: await this.listHealthResultsForRun(response.scan_run_id),
    };
  },
  async listRecentHealthRuns(limit: number): Promise<ScanRun[]> {
    return this.listScanRuns("health", limit);
  },
  async listHealthResultsForRun(scanRunId: number): Promise<HealthCheckRow[]> {
    return invoke<HealthCheckRow[]>("list_health_results_for_run", { scanRunId });
  },
  async startPsoCollection(args: {
    sourceMachineId: number;
    projectId: number;
    ueVersion: string | null;
    resolutionW: number;
    resolutionH: number;
    windowed: boolean;
    maxMinutes: number;
    operatorCredentialAlias: string | null;
  }): Promise<PsoCollectJobResponse> {
    return invoke<PsoCollectJobResponse>("start_pso_collection", {
      sourceMachineId: args.sourceMachineId,
      projectId: args.projectId,
      ueVersion: args.ueVersion,
      resolutionW: args.resolutionW,
      resolutionH: args.resolutionH,
      windowed: args.windowed,
      maxMinutes: args.maxMinutes,
      operatorCredentialAlias: args.operatorCredentialAlias,
    });
  },
  async listPsoCacheFiles(
    projectId: number,
    filters?: { sourceMachineId?: number | null; gpuSignature?: string | null },
  ): Promise<PsoCacheFile[]> {
    return invoke<PsoCacheFile[]>("list_pso_cache_files", {
      projectId,
      sourceMachineId: filters?.sourceMachineId ?? null,
      gpuSignature: filters?.gpuSignature ?? null,
    });
  },
  async distributePsoCache(args: {
    fileId: number;
    targetMachineIds: number[];
    namedShareUnc: string | null;
    operatorCredentialAlias: string | null;
    sourceSmbCredentialAlias: string | null;
    forceGpuMismatch: boolean;
  }): Promise<PsoDistributeJobResponse> {
    return invoke<PsoDistributeJobResponse>("distribute_pso_cache", {
      request: {
        file_id: args.fileId,
        target_machine_ids: args.targetMachineIds,
        named_share_unc: args.namedShareUnc,
        operator_credential_alias: args.operatorCredentialAlias,
        source_smb_credential_alias: args.sourceSmbCredentialAlias,
        force_gpu_mismatch: args.forceGpuMismatch,
      },
    });
  },
  async verifyPsoPrecaching(request: ScanInisRequest): Promise<ScanInisResponse> {
    return invoke<ScanInisResponse>("verify_pso_precaching", { request });
  },
  async getGpuConsistencyMatrix(): Promise<GpuMatrix> {
    return invoke<GpuMatrix>("get_gpu_consistency_matrix");
  },
};
