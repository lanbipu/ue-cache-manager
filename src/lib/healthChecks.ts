export type StatusKind =
  | "healthy" | "warning" | "critical"
  | "na" | "offline" | "progress" | "unknown" | "info";

export interface HealthCheckDef {
  id: string;
  label: string;
  shortLabel: string;
  emphasized?: boolean;
  description: string;
  remediation: string;
  symptom: string;
}

export const HEALTH_CHECKS: HealthCheckDef[] = [
  { id: "smb", label: "SMB service running", shortLabel: "SMB",
    description: "LanmanServer service running on the host.",
    remediation: "Start-Service LanmanServer; Set-Service -StartupType Automatic.",
    symptom: "Clients cannot mount any share." },
  { id: "firewall_445", label: "Firewall 445 inbound allowed", shortLabel: "FW 445",
    description: "Inbound TCP/445 SMB rule enabled.",
    remediation: "Enable-NetFirewallRule -DisplayGroup 'File and Printer Sharing'.",
    symptom: "Test-Path \\\\HOST\\Share fails despite share existing." },
  { id: "share_reachable", label: "Shared drive reachable", shortLabel: "Share",
    description: "Test-Path against the configured share UNC.",
    remediation: "Verify host online + share present + ACL grants this user.",
    symptom: "DDC fallback to local; cluster cache miss everywhere." },
  { id: "ntfs_perm", label: "NTFS permission (Host only)", shortLabel: "NTFS",
    description: "Host's local NTFS ACL grants the share account Full Control.",
    remediation: "icacls D:\\DDC /grant ddc-svc:(OI)(CI)F.",
    symptom: "Clients can mount but get Access Denied on read/write." },
  { id: "cred_user", label: "User-level credential", shortLabel: "Cred U",
    description: "cmdkey /list contains the SMB host entry for the current user.",
    remediation: "Re-run Mode B inject step to repopulate user-level cmdkey.",
    symptom: "Interactive UE editor cannot mount the share." },
  { id: "cred_system", label: "SYSTEM-level credential", shortLabel: "Cred SYS",
    emphasized: true,
    description: "PsExec -s cmdkey /list shows the entry under LocalSystem.",
    remediation: "Re-run inject-system-credential.ps1; verify PsExec64 staged.",
    symptom: "RenderStream Service / SYSTEM tasks cannot read DDC. Hardest to debug." },
  { id: "env_vars", label: "Environment variables", shortLabel: "Env",
    description: "UE-SharedDataCachePath matches the expected UNC.",
    remediation: "Use Machines > Env vars to push the correct value.",
    symptom: "INI EnvPathOverride resolves to empty; DDC silently local-only." },
  { id: "ini_consistency", label: "INI consistency", shortLabel: "INI",
    description: "Latest INI scan shows no open critical findings on this machine.",
    remediation: "Open INI Scanner, apply suggested fixes.",
    symptom: "Misconfigured DDC paths overrule cluster settings." },
  { id: "system_write", label: "SYSTEM write test", shortLabel: "SYS Write",
    emphasized: true,
    description: "PsExec -s writes a probe file to the share. Final ground-truth test.",
    remediation: "Resolve cred_system + ntfs_perm. If both green and this is red, ACL on a parent dir likely.",
    symptom: "Service-context shader compile output cannot be cached cluster-wide." },
  { id: "pso_precaching", label: "PSO Precaching CVar", shortLabel: "PSO cvar",
    description: "Project ConsoleVariables.ini sets r.PSOPrecaching=1.",
    remediation: "Set the CVar via the project ConsoleVariables.ini.",
    symptom: "Scene-load hitches that PSO Cache files cannot fully cover." },
  { id: "gpu_consistency", label: "GPU / driver consistency", shortLabel: "GPU",
    description: "All cluster machines share the same (gpu_model, driver_version).",
    remediation: "Standardize driver version across the cluster.",
    symptom: "PSO Cache file collected on machine A is invalid on machine B." },
];
