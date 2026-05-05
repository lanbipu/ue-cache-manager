import { defineStore } from "pinia";
import { ref } from "vue";
import { tauriApi, type CheckOutcome, type HealthRunSummary, type UecmError } from "@/services/tauri";

export const useHealthCheckStore = defineStore("healthCheck", () => {
  const scanRunId = ref<number | null>(null);
  const summary = ref<HealthRunSummary>({ scan_run_id: 0, healthy: 0, warning: 0, critical: 0, offline: 0, total: 0 });
  const rowsByMachine = ref<Record<number, Record<string, CheckOutcome>>>({});
  const isRunning = ref(false);
  const error = ref<UecmError | null>(null);

  async function run(machineIds: number[], projectPaths: Record<number, string[]>, credentialAlias: string) {
    isRunning.value = true; error.value = null;
    try {
      const s = await tauriApi.runHealthCheck(machineIds, projectPaths, credentialAlias);
      scanRunId.value = s.scan_run_id;
      summary.value = s;
      const rows = await tauriApi.listHealthResultsForRun(s.scan_run_id);
      const byMachine: Record<number, Record<string, CheckOutcome>> = {};
      for (const r of rows) byMachine[r.machine_id] = r.machine_results;
      rowsByMachine.value = byMachine;
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isRunning.value = false;
    }
  }

  return { scanRunId, summary, rowsByMachine, isRunning, error, run };
});
