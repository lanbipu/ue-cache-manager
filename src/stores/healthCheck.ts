import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { tauriApi, PROBE_LAYER_MAP, type CheckOutcome, type HealthCheckRun, type ProbeLayer, type RunHealthCheckRequest, type UecmError } from "@/services/tauri";
import { useDiagnosticsStore } from "./diagnostics";
import { useGpuConsistencyStore } from "./gpuConsistency";

export interface LayeredOutcome {
  key: string;
  outcome: CheckOutcome;
}

export const useHealthCheckStore = defineStore("healthCheck", () => {
  const diagnostics = useDiagnosticsStore();
  const gpuConsistency = useGpuConsistencyStore();
  const scanRunId = ref<number | null>(null);
  const results = ref<HealthCheckRun[]>([]);
  const isRunning = ref(false);
  const error = ref<UecmError | null>(null);

  const rowsByMachine = computed<Record<number, Record<string, CheckOutcome>>>(() => {
    const out: Record<number, Record<string, CheckOutcome>> = {};
    for (const row of results.value) {
      out[row.machine_id] = {
        ...row.machine_results,
        pso_precaching: derivePsoPrecaching(row.machine_id, row.machine_results.pso_precaching),
        gpu_consistency: deriveGpuConsistency(row.machine_id, row.machine_results.gpu_consistency),
      };
    }
    return out;
  });

  const summary = computed(() => {
    const out = { healthy: 0, warning: 0, critical: 0, offline: 0, unknown: 0, total: 0 };
    for (const row of Object.values(rowsByMachine.value)) {
      for (const outcome of Object.values(row)) {
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

  const probesByLayer = computed<Record<number, Record<ProbeLayer, LayeredOutcome[]>>>(() => {
    const out: Record<number, Record<ProbeLayer, LayeredOutcome[]>> = {};
    for (const [mid, row] of Object.entries(rowsByMachine.value)) {
      const grouped: Record<ProbeLayer, LayeredOutcome[]> = {
        l1_port: [],
        l2_bootstrap: [],
        l3_business: [],
      };
      for (const [key, outcome] of Object.entries(row)) {
        const layer = (PROBE_LAYER_MAP as Record<string, ProbeLayer | undefined>)[key];
        if (!layer) {
          // Unknown key — log loud so renames/typos don't silently disappear in UI.
          console.warn(
            `[healthCheck] unknown probe key '${key}' not in PROBE_LAYER_MAP — ` +
            `update src/services/tauri.ts and src-tauri/src/core/probe_keys.rs in sync.`
          );
          continue;
        }
        grouped[layer].push({ key, outcome });
      }
      for (const layer of ["l1_port", "l2_bootstrap", "l3_business"] as const) {
        grouped[layer].sort((a, b) => a.key.localeCompare(b.key));
      }
      out[Number(mid)] = grouped;
    }
    return out;
  });

  function derivePsoPrecaching(machineId: number, fallback?: CheckOutcome): CheckOutcome {
    if (diagnostics.scanRunId == null) {
      return fallback ?? unknown("No PSO CVar scanner data loaded.");
    }
    const findings = diagnostics.open.filter(
      (finding) =>
        finding.machine_id === machineId &&
        ["R008", "R009", "R010"].includes(finding.rule_id),
    );
    if (findings.length === 0) {
      return {
        status: "healthy",
        message: "No open R008-R010 PSO CVar findings.",
        sample: "",
        remediation: "No action required.",
      };
    }
    const critical = findings.filter((finding) => finding.severity === "critical").length;
    return {
      status: critical > 0 ? "critical" : "warning",
      message: `${findings.length} open PSO CVar finding(s): ${findings.map((finding) => finding.rule_id).join(", ")}`,
      sample: findings[0]?.file_path ?? "",
      remediation: "Open INI Scanner and apply R008-R010 recommendations.",
    };
  }

  function deriveGpuConsistency(machineId: number, fallback?: CheckOutcome): CheckOutcome {
    const cell = gpuConsistency.matrix?.cells.find((item) => item.machine_id === machineId);
    if (!cell) {
      return fallback ?? unknown("No GPU consistency matrix loaded.");
    }
    if (cell.status === "match") {
      return {
        status: "healthy",
        message: cell.signature
          ? `${cell.signature.model} / driver ${cell.signature.driver}`
          : "GPU signature matches baseline.",
        sample: gpuConsistency.baselineLabel,
        remediation: "No action required.",
      };
    }
    if (cell.status === "deviation") {
      return {
        status: "warning",
        message: cell.signature
          ? `${cell.signature.model} / driver ${cell.signature.driver}`
          : "GPU signature differs from baseline.",
        sample: gpuConsistency.baselineLabel,
        remediation: "Align GPU model and driver version before distributing PSO cache files.",
      };
    }
    return unknown("No GPU inventory row for this machine.");
  }

  function unknown(message: string): CheckOutcome {
    return {
      status: "unknown",
      message,
      sample: "",
      remediation: "Refresh inventory or run the relevant scan.",
    };
  }

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

  return { scanRunId, results, rowsByMachine, summary, probesByLayer, isRunning, error, run, loadResults };
});
