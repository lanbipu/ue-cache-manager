<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { RouterLink } from "vue-router";
import Button from "@/components/ui/Button.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmStateBlock from "@/components/primitives/UecmStateBlock.vue";
import UecmStat from "@/components/primitives/UecmStat.vue";
import UecmStatusBadge from "@/components/primitives/UecmStatusBadge.vue";
import { useDdcPakStore } from "@/stores/ddcPak";
import { useGpuConsistencyStore } from "@/stores/gpuConsistency";
import { useHealthCheckStore } from "@/stores/healthCheck";
import { useMachinesStore } from "@/stores/machines";
import { useProjectsStore } from "@/stores/projects";
import { usePsoStore } from "@/stores/pso";
import { tauriApi, type EchoResult, type UecmError } from "@/services/tauri";

const machines = useMachinesStore();
const projects = useProjectsStore();
const ddcPak = useDdcPakStore();
const pso = usePsoStore();
const health = useHealthCheckStore();
const gpu = useGpuConsistencyStore();
const result = ref<EchoResult | null>(null);
const error = ref<UecmError | null>(null);
const loading = ref(false);

const online = computed(() => machines.machines.filter((machine) => machine.status.toLowerCase() === "online").length);
const critical = computed(() => machines.machines.filter((machine) => ["critical", "error", "failed"].includes(machine.status.toLowerCase())).length);
const warning = computed(() => machines.machines.filter((machine) => ["warning", "warn", "degraded"].includes(machine.status.toLowerCase())).length);
const lastDdcJob = computed(() => ddcPak.generateJobs[0] ?? null);
const lastPsoJob = computed(() => pso.collectJobs[0] ?? null);
const healthScore = computed(() => {
  const total = health.summary.total;
  if (total === 0) return null;
  return Math.max(0, Math.round(((health.summary.healthy - health.summary.critical * 0.75 - health.summary.warning * 0.35) / total) * 100));
});
function alertTone(status: string) {
  const normalized = status.toLowerCase();
  if (normalized === "offline") return "offline" as const;
  if (["critical", "error", "failed"].includes(normalized)) return "critical" as const;
  return "warning" as const;
}
const alerts = computed(() =>
  machines.machines
    .filter((machine) => machine.status.toLowerCase() !== "online")
    .map((machine) => ({
      tone: alertTone(machine.status),
      title: `${machine.hostname} ${machine.status}`,
      detail: `${machine.ip} last seen ${machine.last_seen_at ?? "unknown"}`,
    })),
);

onMounted(() => {
  void Promise.all([machines.loadMachines(), projects.load(), gpu.load()]);
});

async function runBridgeTest() {
  result.value = null;
  error.value = null;
  loading.value = true;
  try {
    result.value = await tauriApi.testPowerShellBridge("hello from UECM");
  } catch (e) {
    error.value = e as UecmError;
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <div class="h-full space-y-6 overflow-auto p-6">
    <UecmPageHeader
      title="Dashboard"
      eyebrow="Mission Control"
      description="Operational snapshot for inventory, projects, DDC pak, PSO cache, and health gates."
    >
      <template #actions>
        <Button variant="outline" data-bridge-test-btn :disabled="loading" @click="runBridgeTest">
          <UecmIcon name="terminal" />
          {{ loading ? "Running..." : "Run bridge test" }}
        </Button>
        <Button as="a" href="#/shares">
          <UecmIcon name="plus" />
          Create Shared DDC
        </Button>
      </template>
    </UecmPageHeader>

    <section data-dashboard-kpi class="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
      <RouterLink to="/machines" class="rounded-lg border bg-card p-4 hover:bg-muted/40">
        <p class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">Machines online</p>
        <p class="mt-2 font-display text-3xl font-extrabold">
          {{ online }} <span class="text-base text-muted-foreground">/ {{ machines.machines.length }}</span>
        </p>
      </RouterLink>
      <RouterLink to="/projects" class="rounded-lg border bg-card p-4 hover:bg-muted/40">
        <p class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">Projects discovered</p>
        <p class="mt-2 font-display text-3xl font-extrabold">{{ projects.projects.length }}</p>
      </RouterLink>
      <RouterLink to="/health-check?gpu=true" class="rounded-lg border bg-card p-4 hover:bg-muted/40">
        <p class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">GPU baseline</p>
        <p class="mt-2 truncate font-mono text-sm">{{ gpu.baselineLabel }}</p>
        <p class="mt-1 text-xs text-status-warning">{{ gpu.deviationCount }} deviation(s)</p>
      </RouterLink>
      <RouterLink to="/health-check" class="rounded-lg border bg-card p-4 hover:bg-muted/40">
        <p class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">Health score</p>
        <p class="mt-2 font-display text-3xl font-extrabold">{{ healthScore ?? "-" }}</p>
      </RouterLink>
    </section>

    <section class="grid gap-4 md:grid-cols-4">
      <UecmStat label="Machines" :value="machines.machines.length" icon="server" :detail="`${online} online`" />
      <UecmStat label="Critical" :value="critical" icon="alert-triangle" detail="SYSTEM / INI blockers" />
      <UecmStat label="Warnings" :value="warning" icon="info" detail="Driver or config drift" />
      <UecmStat label="DDC Path" value="\\HOST-01" icon="hard-drive" detail="Shared cache baseline" />
    </section>

    <section data-dashboard-recent class="grid gap-4 md:grid-cols-2">
      <RouterLink to="/ddc-pak" class="rounded-lg border bg-card p-4 hover:bg-muted/40">
        <p class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">Last DDC pak</p>
        <p class="mt-2 text-sm">
          {{ lastDdcJob ? `${lastDdcJob.status} / ${lastDdcJob.job_id}` : "No DDC pak job queued." }}
        </p>
      </RouterLink>
      <RouterLink to="/pso-cache" class="rounded-lg border bg-card p-4 hover:bg-muted/40">
        <p class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">Last PSO collection</p>
        <p class="mt-2 text-sm">
          {{ lastPsoJob ? `${lastPsoJob.status} / ${lastPsoJob.files_collected ?? "?"} file(s)` : "No PSO collection queued." }}
        </p>
      </RouterLink>
    </section>

    <section class="grid gap-4 xl:grid-cols-[1.2fr_0.8fr]">
      <article class="rounded-lg border bg-card p-4">
        <div class="mb-3 flex items-center justify-between">
          <h2 class="font-display text-lg font-extrabold">Cluster Alerts</h2>
          <UecmStatusBadge tone="warning" :label="`${alerts.length} open`" size="sm" />
        </div>
        <UecmStateBlock
          v-if="machines.machines.length === 0"
          variant="empty"
          title="No machines registered yet"
          message="Use Machines > Scan to build the inventory."
        />
        <UecmStateBlock
          v-else-if="alerts.length === 0"
          variant="empty"
          title="No open machine status alerts"
          message="Machine inventory is currently clean."
        />
        <div v-else class="divide-y">
          <div v-for="alert in alerts" :key="alert.title" class="flex items-start gap-3 py-3">
            <UecmStatusBadge :tone="alert.tone" :label="alert.tone" size="sm" />
            <div>
              <div class="text-sm font-bold">{{ alert.title }}</div>
              <div class="text-sm text-muted-foreground">{{ alert.detail }}</div>
            </div>
          </div>
        </div>
      </article>

      <article class="rounded-lg border bg-card p-4">
        <h2 class="font-display text-lg font-extrabold">PowerShell Bridge</h2>
        <p class="mt-1 text-sm text-muted-foreground">
          Verifies frontend to Rust to PowerShell sidecar pipeline. Non-Windows machines may return a Windows-only error.
        </p>
        <pre v-if="result" class="mt-4 overflow-auto rounded-md border bg-muted p-3 font-mono text-xs">{{ JSON.stringify(result, null, 2) }}</pre>
        <p v-if="error" class="mt-4 rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">
          {{ error.code }}: {{ error.message }}
        </p>
      </article>
    </section>
  </div>
</template>
