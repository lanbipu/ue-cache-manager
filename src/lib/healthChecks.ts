export type StatusKind = "healthy" | "warning" | "critical" | "na" | "offline" | "progress" | "unknown";

export const HEALTH_CHECKS = [
  { id: "smb", label: "SMB" },
  { id: "fw445", label: "Firewall 445" },
  { id: "share", label: "Share" },
  { id: "credSystem", label: "SYSTEM credential" },
  { id: "envvar", label: "Env vars" },
  { id: "ini", label: "INI" },
  { id: "sysWrite", label: "SYSTEM write" },
  { id: "gpu", label: "GPU" },
] as const;
