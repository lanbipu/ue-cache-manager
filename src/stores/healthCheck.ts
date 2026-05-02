import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { tauriApi, type CheckOutcome, type HealthCheckRun, type RunHealthCheckRequest, type UecmError } from "@/services/tauri";

export const useHealthCheckStore = defineStore("healthCheck", () => {
  const scanRunId = ref<number | null>(null);
  const results = ref<HealthCheckRun[]>([]);
  const isRunning = ref(false);
  const error = ref<UecmError | null>(null);

  const rowsByMachine = computed<Record<number, Record<string, CheckOutcome>>>(() => {
    const out: Record<number, Record<string, CheckOutcome>> = {};
    for (const row of results.value) out[row.machine_id] = row.machine_results;
    return out;
  });

  const summary = computed(() => {
    const out = { healthy: 0, warning: 0, critical: 0, offline: 0, unknown: 0, total: 0 };
    for (const row of results.value) {
      for (const outcome of Object.values(row.machine_results)) {
        out.total += 1;
        if (outcome.status === "healthy") out.healthy += 1;
        else if (outcome.status === "warning") out.warning += 1;
        else if (outcome.status === "critical") out.critical += 1;
        else if (outcome.status === "offline") out.offline += 1;
        else out.unknown += 1;
      }
    }
    return out;
  });

  async function run(request: RunHealthCheckRequest) {
    isRunning.value = true;
    error.value = null;
    try {
      const response = await tauriApi.runHealthCheck(request);
      scanRunId.value = response.scan_run_id;
      results.value = response.results;
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isRunning.value = false;
    }
  }

  async function loadResults(id: number) {
    scanRunId.value = id;
    results.value = await tauriApi.listHealthResultsForRun(id);
  }

  return { scanRunId, results, rowsByMachine, summary, isRunning, error, run, loadResults };
});
