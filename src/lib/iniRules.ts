export interface IniRuleDef {
  id: string;
  label: string;
  description: string;
  rationale: string;
  fixHint: string;
}

export const INI_RULES: Record<string, IniRuleDef> = {
  R001: {
    id: "R001",
    label: "Hardcoded DDC path overrides env-var",
    description: "Path= is set in DerivedDataCacheSettings without EnvPathOverride.",
    rationale: "UE only consults EnvPathOverride when it's present. With a literal Path=, the env var is ignored — silently.",
    fixHint: "Replace Path= with EnvPathOverride=UE-SharedDataCachePath; ensure the env var is set on every machine.",
  },
  R002: {
    id: "R002",
    label: "User-level DDC override (silent killer)",
    description: "EditorPerProjectUserSettings.ini contains DDC keys.",
    rationale: "User-level config is the highest-priority source. It silently masks every cluster setting until the file is cleaned up.",
    fixHint: "Remove the DDC section from the user-level file. UECM auto-backs up before removing.",
  },
  R003: {
    id: "R003",
    label: "DDC path resolves to unreachable target",
    description: "The configured path returned probe failure.",
    rationale: "Pointing at an offline share or stale UNC means cluster DDC always misses, falls back to per-machine compute.",
    fixHint: "Repoint to a live share, or fix the share / firewall / network.",
  },
  R004: {
    id: "R004",
    label: "Mapped drive letter in DDC path",
    description: "Path uses Z:\\ or similar mapped drive instead of UNC.",
    rationale: "Windows Services (RenderStream, etc.) can't see user-mapped drive letters. They fail silently when the path resolves to nothing.",
    fixHint: "Replace with the underlying \\\\HOST\\Share path.",
  },
  R005: {
    id: "R005",
    label: "Deprecated CVar present",
    description: "An obsolete CVar (e.g. r.SShaderCache) is set.",
    rationale: "Deprecated CVars do nothing in current UE; they only confuse readers.",
    fixHint: "Remove the line.",
  },
  R006: {
    id: "R006",
    label: "EnvPathOverride references missing env-var",
    description: "INI references an env-var that is not set on the machine.",
    rationale: "The override resolves to empty; DDC silently falls back to local.",
    fixHint: "Use Env vars in Machine detail to set the variable on the machine.",
  },
  R007: {
    id: "R007",
    label: "Healthy: env-var-driven DDC config",
    description: "EnvPathOverride references a populated env-var.",
    rationale: "This is the recommended cluster-friendly config.",
    fixHint: "(no fix required)",
  },
};
