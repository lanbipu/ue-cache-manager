import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { tauriApi, type IniFinding, type ScanRunSummary, type UecmError } from "@/services/tauri";

export const useDiagnosticsStore = defineStore("diagnostics", () => {
  const scanRunId = ref<number | null>(null);
  const summary = ref<ScanRunSummary>({ scan_run_id: 0, critical: 0, warning: 0, healthy: 0 });
  const findings = ref<IniFinding[]>([]);
  const isScanning = ref(false);
  const error = ref<UecmError | null>(null);

  const open = computed(() => findings.value.filter(f => !f.fixed_at && !f.skipped_at));

  async function runScan(
    machineIds: number[],
    projectPaths: Record<number, string[]>,
    userProfile: string,
    credentialAlias: string,
  ) {
    isScanning.value = true; error.value = null;
    try {
      const s = await tauriApi.scanInis(machineIds, projectPaths, userProfile, credentialAlias);
      scanRunId.value = s.scan_run_id;
      summary.value = s;
      findings.value = await tauriApi.listFindingsForRun(s.scan_run_id);
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isScanning.value = false;
    }
  }

  async function applyFinding(findingId: number, credentialAlias: string) {
    error.value = null;
    try {
      await tauriApi.applyFinding(findingId, credentialAlias);
      const f = findings.value.find(x => x.id === findingId);
      if (f) f.fixed_at = new Date().toISOString();
    } catch (e) {
      error.value = e as UecmError;
    }
  }

  async function skipFinding(findingId: number) {
    error.value = null;
    try {
      await tauriApi.skipFinding(findingId);
      const f = findings.value.find(x => x.id === findingId);
      if (f) f.skipped_at = new Date().toISOString();
    } catch (e) {
      error.value = e as UecmError;
    }
  }

  return { scanRunId, summary, findings, open, isScanning, error,
           runScan, applyFinding, skipFinding };
});
