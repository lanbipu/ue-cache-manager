export type StatusKind = "healthy" | "warning" | "critical" | "na" | "offline" | "progress" | "unknown";

export interface HealthCheckDefinition {
  id: string;
  shortLabel: string;
  label: string;
  description: string;
  symptom: string;
  remediation: string;
  subtitle?: string;
  emphasized?: boolean;
}

// MUST stay in sync with src-tauri/src/core/probe_keys.rs PROBE_REGISTRY (17 entries).
// Order matches the 3-layer visual stack: L1 ports → L2 bootstrap → L3 business + derived.
// Drift between this list and PROBE_LAYER_MAP / locale files is guarded by
// health-checks-coverage.spec.ts.
export const HEALTH_CHECKS = [
  // L1 — port reachability (Rust TCP probes, no creds)
  { id: "tcp_5985", shortLabel: "WRM", label: "WinRM 5985", description: "Operator console can TCP-connect to WinRM port 5985.", symptom: "Remote PowerShell calls fail before authentication.", remediation: "Run `uecm-cli winrm bootstrap <host>` (Path B), or USB Path A if all three ports are closed." },
  { id: "tcp_445", shortLabel: "SMB", label: "SMB 445", description: "Operator console can TCP-connect to SMB port 445.", symptom: "SMB shares unreachable; PsExec / share-mount fails.", remediation: "Open inbound TCP 445 + start LanmanServer (winrm bootstrap -EnableSmbServer does both)." },
  { id: "tcp_135", shortLabel: "RPC", label: "RPC 135", description: "Operator console can TCP-connect to DCE/RPC Endpoint Mapper port 135.", symptom: "PsExec-based Path B bootstrap fails to register the SCM service.", remediation: "Switch network profile to Private; winrm bootstrap -NetworkCategory Private handles it." },

  // L2 — bootstrap configuration (PowerShell via WinRM)
  { id: "firewall_445", shortLabel: "FW", label: "Firewall 445", description: "FPS-SMB-In-TCP firewall rule is enabled (TCP 445 inbound).", symptom: "Share path times out from peer machines.", remediation: "Enable-NetFirewallRule -Name FPS-SMB-In-TCP, or re-run winrm bootstrap." },
  { id: "local_account_token_filter", shortLabel: "LATFP", label: "LocalAccountTokenFilterPolicy", description: "Registry DWORD LATFP=1 — allows local admin tokens to elevate over remote NTLM.", symptom: "Remote ops appear to succeed but run with stripped tokens (UAC remote filter).", remediation: "Re-run `uecm-cli winrm bootstrap <host>` (default flow sets LATFP=1)." },
  { id: "long_paths_enabled", shortLabel: "LP", label: "LongPathsEnabled", description: "Registry DWORD LongPathsEnabled=1 — required for UE asset paths > 260 chars.", symptom: "Robocopy / UE build fails with path-too-long.", remediation: "Re-run `uecm-cli winrm bootstrap <host>` (default flow sets LongPathsEnabled=1)." },
  { id: "lanman_server", shortLabel: "LMS", label: "LanmanServer service", description: "LanmanServer (SMB server) is Running and Automatic.", symptom: "DDC shares are unreachable; share creation fails.", remediation: "Start-Service LanmanServer + Set-Service Automatic, or re-run winrm bootstrap." },

  // L3 — business workflow (PowerShell via WinRM + derived)
  { id: "share_reachable", shortLabel: "SHR", label: "Share reachable", description: "A non-admin SMB share exists and can be enumerated.", symptom: "Cache root cannot be mounted.", remediation: "Create or repair the DDC share via `uecm-cli share create`." },
  { id: "ntfs_perm", shortLabel: "ACL", label: "NTFS permissions", description: "Share path NTFS ACL grants the svc account (e.g. ddc-svc) write access.", symptom: "Writes fail after network access succeeds.", remediation: "Grant ACL: icacls <sharePath> /grant ddc-svc:(OI)(CI)F" },
  { id: "cred_user", shortLabel: "USR", label: "User credential store", description: "Current user Credential Manager has the svc account entry.", symptom: "User-context SMB access prompts or fails.", remediation: "Run `uecm-cli share inject-system-cred --host <host>` to populate user + SYSTEM stores." },
  { id: "cred_system", shortLabel: "SYS", label: "SYSTEM credential store", description: "SYSTEM-context Credential Manager has the svc account entry.", symptom: "Editor works but service/SYSTEM jobs fail.", remediation: "Run `uecm-cli share inject-system-cred --host <host>` to push the cred into SYSTEM.", emphasized: true },
  { id: "env_vars", shortLabel: "ENV", label: "UE-SharedDataCachePath env", description: "Shared DDC env var is set at machine scope.", symptom: "EnvPathOverride points to an empty value.", remediation: "Run `uecm-cli env set --name UE-SharedDataCachePath --value <UNC>`." },
  { id: "system_write", shortLabel: "WRT", label: "SYSTEM share write", description: "SYSTEM-context write to the share succeeds (PsExec probe).", symptom: "Render services cannot write to shared cache.", remediation: "Verify cred_system AND share NTFS ACL grants the svc account write.", emphasized: true },
  { id: "winmgmt", shortLabel: "WMI", label: "Winmgmt service", description: "Winmgmt (WMI) is Running — required by machine refresh GPU detection.", symptom: "GPU consistency / refresh probe shows 'no GPU data'.", remediation: "Start-Service Winmgmt + Set-Service Automatic, or re-run winrm bootstrap." },

  // L3 — derived (computed in Rust from other tables, not from PS)
  { id: "ini_consistency", shortLabel: "INI", label: "INI consistency", description: "Latest INI scan has no open critical findings.", symptom: "Machines silently use different cache paths.", remediation: "Resolve INI Scanner findings." },
  { id: "pso_precaching", shortLabel: "PSO", label: "PSO Precaching CVar", subtitle: "Derived from INI scanner R008-R010.", description: "Required PSO precaching CVars are enabled in ConsoleVariables.ini.", symptom: "PSO collection is incomplete or inconsistent.", remediation: "Apply R008-R010 recommendations in INI Scanner." },
  { id: "gpu_consistency", shortLabel: "GPU", label: "GPU/driver consistency", subtitle: "Derived from gpu_consistency module.", description: "GPU model and driver versions match across cluster.", symptom: "PSO cache is invalid across machines.", remediation: "Standardize GPU + driver across the cluster, or split into compatible subgroups before PSO distribute." },
] as const satisfies readonly HealthCheckDefinition[];
