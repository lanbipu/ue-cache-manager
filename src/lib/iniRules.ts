export const INI_RULES: Record<string, { title: string; label?: string; description?: string; rationale?: string; tone: string }> = {
  R001: { title: "Hardcoded DDC Path", tone: "critical" },
  R002: { title: "User-level DDC override", tone: "critical" },
  R003: { title: "Unreachable cache path", tone: "critical" },
  R004: { title: "Mapped drive path", tone: "warning" },
  R005: { title: "Deprecated CVar", tone: "warning" },
  R006: { title: "Missing env var", tone: "warning" },
  R007: { title: "Healthy EnvPathOverride", tone: "healthy" },
  R008: { title: "PSO precaching disabled", tone: "critical" },
  R009: { title: "PSO compile disabled", tone: "warning" },
  R010: { title: "Global shader PSO disabled", tone: "warning" },
  SCAN_ERROR: { title: "Read skipped", tone: "info" },
};
