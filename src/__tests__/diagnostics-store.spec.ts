import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useDiagnosticsStore } from "@/stores/diagnostics";

const { scanInis, listFindings, applyFinding, skipFinding } = vi.hoisted(() => ({
  scanInis: vi.fn(),
  listFindings: vi.fn(),
  applyFinding: vi.fn(),
  skipFinding: vi.fn(),
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: { scanInis, listFindings, applyFinding, skipFinding },
}));

describe("diagnostics store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("stores scan response", async () => {
    scanInis.mockResolvedValue({ scan_run_id: 1, summary: { scan_run_id: 1, critical: 1, warning: 0, healthy: 0, info: 0, total_files: 2 }, findings: [{ id: 5, fixed_at: null, skipped_at: null }] });
    const store = useDiagnosticsStore();
    await store.run({ machine_ids: [1], credential_alias: "cred", project_paths: [], user_profile_path: null });
    expect(store.scanRunId).toBe(1);
    expect(store.open.length).toBe(1);
    expect(store.summary.critical).toBe(1);
  });

  it("reloads after applying a finding", async () => {
    listFindings.mockResolvedValue([]);
    const store = useDiagnosticsStore();
    store.scanRunId = 1;
    await store.applyFinding(5, "cred");
    expect(applyFinding).toHaveBeenCalledWith(5, "cred");
    expect(listFindings).toHaveBeenCalledWith(1);
  });
});
