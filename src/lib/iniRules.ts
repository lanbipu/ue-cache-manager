export const INI_RULES: Record<string, { title: string; tone: string }> = {
  R001: { title: "Hardcoded DDC Path", tone: "critical" },
  R002: { title: "User-level DDC override", tone: "critical" },
  R003: { title: "Unreachable cache path", tone: "critical" },
  R004: { title: "Mapped drive path", tone: "warning" },
  R005: { title: "Deprecated CVar", tone: "warning" },
  R006: { title: "Missing env var", tone: "warning" },
  R007: { title: "Healthy EnvPathOverride", tone: "healthy" },
  SCAN_ERROR: { title: "Read skipped", tone: "info" },
};
