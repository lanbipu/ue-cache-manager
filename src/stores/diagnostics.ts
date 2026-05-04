import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { tauriApi, type IniFinding, type IniScanSummary, type ScanInisRequest, type UecmError } from "@/services/tauri";

export const useDiagnosticsStore = defineStore("diagnostics", () => {
  const scanRunId = ref<number | null>(null);
  const findings = ref<IniFinding[]>([]);
  const isRunning = ref(false);
  const isApplying = ref(false);
  const error = ref<UecmError | null>(null);
  const summary = ref<IniScanSummary>({ scan_run_id: 0, critical: 0, warning: 0, healthy: 0, info: 0, total_files: 0 });

  const open = computed(() => findings.value.filter((f) => !f.fixed_at && !f.skipped_at));

  async function run(request: ScanInisRequest) {
    isRunning.value = true;
    error.value = null;
    try {
      const response = await tauriApi.scanInis(request);
      scanRunId.value = response.scan_run_id;
      summary.value = response.summary;
      findings.value = response.findings;
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isRunning.value = false;
    }
  }

  async function loadFindings(id: number) {
    scanRunId.value = id;
    findings.value = await tauriApi.listFindings(id);
  }

  async function applyFinding(id: number, credentialAlias: string) {
    isApplying.value = true;
    try {
      await tauriApi.applyFinding(id, credentialAlias);
      if (scanRunId.value != null) await loadFindings(scanRunId.value);
    } finally {
      isApplying.value = false;
    }
  }

  async function skipFinding(id: number) {
    await tauriApi.skipFinding(id);
    if (scanRunId.value != null) await loadFindings(scanRunId.value);
  }

  return { scanRunId, findings, open, summary, isRunning, isApplying, error, run, loadFindings, applyFinding, skipFinding };
});
