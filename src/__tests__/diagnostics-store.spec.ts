import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useDiagnosticsStore } from "@/stores/diagnostics";

vi.mock("@/services/tauri", () => ({
  tauriApi: {
    scanInis: vi.fn(async () => ({ scan_run_id: 7, critical: 1, warning: 2, healthy: 4 })),
    listFindingsForRun: vi.fn(async () => ([
      { id: 1, scan_run_id: 7, machine_id: 11, rule_id: "R001", severity: "critical",
        category: "project", file_path: "C:\\f.ini", section: "DDC", key_name: "Path",
        line_number: 1, snippet_before: "Path=X", snippet_after: "EnvPathOverride=Y",
        recommended_action: "set", recommended_value: "Y",
        symptom: "s", rationale: "r", fixed_at: null, skipped_at: null },
    ])),
    applyFinding: vi.fn(async () => "C:\\f.ini.bak.20260503-100000"),
    skipFinding: vi.fn(async () => {}),
  },
}));

describe("useDiagnosticsStore", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("runScan populates findings", async () => {
    const s = useDiagnosticsStore();
    await s.runScan([11], {}, "C:\\Users\\X", "UECM:winrm:LANPC");
    expect(s.scanRunId).toBe(7);
    expect(s.findings.length).toBe(1);
    expect(s.summary.critical).toBe(1);
  });

  it("apply marks finding as fixed", async () => {
    const s = useDiagnosticsStore();
    await s.runScan([11], {}, "C:\\Users\\X", "UECM:winrm:LANPC");
    await s.applyFinding(1, "UECM:winrm:LANPC");
    const f = s.findings.find(x => x.id === 1)!;
    expect(f.fixed_at).not.toBeNull();
  });

  it("skip marks finding as skipped", async () => {
    const s = useDiagnosticsStore();
    await s.runScan([11], {}, "C:\\Users\\X", "UECM:winrm:LANPC");
    await s.skipFinding(1);
    const f = s.findings.find(x => x.id === 1)!;
    expect(f.skipped_at).not.toBeNull();
  });
});
