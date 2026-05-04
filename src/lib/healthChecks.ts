export type StatusKind = "healthy" | "warning" | "critical" | "na" | "offline" | "progress" | "unknown";

export const HEALTH_CHECKS = [
  { id: "smb", shortLabel: "SMB", label: "SMB service", description: "Windows Server service is running.", symptom: "DDC shares are unreachable.", remediation: "Start LanmanServer." },
  { id: "firewall_445", shortLabel: "445", label: "Firewall 445", description: "Inbound SMB firewall rules allow TCP 445.", symptom: "Share path times out from peer machines.", remediation: "Enable File and Printer Sharing inbound rules." },
  { id: "share_reachable", shortLabel: "SHR", label: "Share reachable", description: "A non-admin SMB share exists and can be enumerated.", symptom: "Cache root cannot be mounted.", remediation: "Create or repair the DDC share." },
  { id: "ntfs", shortLabel: "ACL", label: "NTFS access", description: "Filesystem and ACL prerequisites are available.", symptom: "Writes fail after network access succeeds.", remediation: "Fix NTFS permissions on the cache root." },
  { id: "cred_user", shortLabel: "USR", label: "User credential", description: "Current user Credential Manager entries can be read.", symptom: "User-context SMB access prompts or fails.", remediation: "Store credentials through UECM." },
  { id: "cred_system", shortLabel: "SYS", label: "SYSTEM credential", description: "SYSTEM credential check prerequisites are staged.", symptom: "Editor works but service/SYSTEM jobs fail.", remediation: "Inject SYSTEM credentials with PsExec64 staged.", emphasized: true },
  { id: "system_write", shortLabel: "WRT", label: "SYSTEM write", description: "SYSTEM-context write prerequisites are staged and testable.", symptom: "Render services cannot write to shared cache.", remediation: "Stage PsExec64 and validate SYSTEM write access to the DDC share.", emphasized: true },
  { id: "ini_consistency", shortLabel: "INI", label: "INI consistency", description: "Latest INI scan has no open critical findings.", symptom: "Machines silently use different cache paths.", remediation: "Resolve INI Scanner findings." },
  { id: "env_vars", shortLabel: "ENV", label: "Environment variables", description: "Shared DDC env var is set at machine scope.", symptom: "EnvPathOverride points to an empty value.", remediation: "Set UE-SharedDataCachePath." },
  { id: "pso_precaching", shortLabel: "PSO", label: "PSO Precaching CVar", description: "Project CVar state is ready for PSO workflows.", symptom: "PSO collection is incomplete or inconsistent.", remediation: "Set PSO precaching CVars in ConsoleVariables.ini." },
  { id: "gpu_consistency", shortLabel: "GPU", label: "GPU/driver consistency", description: "GPU model and driver versions align across cluster.", symptom: "PSO cache is invalid across machines.", remediation: "Align GPU model and driver version before PSO work." },
] as const;
