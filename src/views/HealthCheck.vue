<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmKpiTile from "@/components/primitives/UecmKpiTile.vue";
import UecmScoreTile from "@/components/primitives/UecmScoreTile.vue";
import UecmStatusBadge from "@/components/primitives/UecmStatusBadge.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import Button from "@/components/ui/Button.vue";
import HealthMatrix from "@/components/diagnostics/HealthMatrix.vue";
import HealthCheckWizard from "@/components/modals/HealthCheckWizard.vue";
import { HEALTH_CHECKS } from "@/lib/healthChecks";
import { useMachinesStore } from "@/stores/machines";
import { useHealthCheckStore } from "@/stores/healthCheck";

const machines = useMachinesStore();
const hc = useHealthCheckStore();

const showWizard = ref(false);
const selected = ref<{ machineId: number; checkId: string } | null>(null);

onMounted(() => machines.loadMachines());

const score = computed(() => {
  const t = hc.summary.total || 1;
  return Math.max(0, Math.round(((hc.summary.healthy - hc.summary.critical * 0.75 - hc.summary.warning * 0.35) / t) * 100));
});
const tone = computed<"healthy" | "warning" | "critical" | "info">(() => {
  if (hc.summary.critical > 0) return "critical";
  if (hc.summary.warning > 0) return "warning";
  if (hc.summary.healthy > 0) return "healthy";
  return "info";
});
const verdict = computed(() => tone.value === "critical" ? "ATTENTION"
                            : tone.value === "warning" ? "DEGRADED"
                            : tone.value === "healthy" ? "HEALTHY" : "IDLE");

const selectedDetail = computed(() => {
  if (!selected.value) return null;
  const def = HEALTH_CHECKS.find(c => c.id === selected.value!.checkId);
  const outcome = hc.rowsByMachine[selected.value.machineId]?.[selected.value.checkId];
  const machine = machines.machines.find(m => m.id === selected.value!.machineId);
  return { def, outcome, machine };
});
</script>

<template>
  <div class="flex h-full flex-col">
    <div class="space-y-4 p-6">
      <UecmPageHeader title="Health Check" eyebrow="Matrix"
        description="11 checks per machine. Click a cell for the diagnostic detail.">
        <template #actions>
          <Button data-open-health-wizard-btn @click="showWizard = true">
            <UecmIcon name="play" /> Run full check
          </Button>
        </template>
      </UecmPageHeader>
      <section class="grid grid-cols-5 gap-px overflow-hidden rounded-lg border bg-border">
        <UecmScoreTile label="Cluster Score" :score="score" :tone="tone" :verdict="verdict" />
        <UecmKpiTile label="Healthy"  :value="hc.summary.healthy"  tone="healthy" />
        <UecmKpiTile label="Warning"  :value="hc.summary.warning"  tone="warning" />
        <UecmKpiTile label="Critical" :value="hc.summary.critical" tone="critical" />
        <UecmKpiTile label="Offline"  :value="hc.summary.offline"  tone="offline" />
      </section>
    </div>

    <section v-if="machines.machines.length === 0" class="grid flex-1 place-items-center text-center">
      <p class="text-sm text-muted-foreground">No machines registered. Use Machines &gt; Scan first.</p>
    </section>
    <section v-else-if="hc.scanRunId === null" class="grid flex-1 place-items-center text-center">
      <div>
        <UecmIcon name="heart-pulse" size="32" class="mx-auto text-muted-foreground" />
        <p class="mt-2 font-display text-lg font-extrabold">Run a full health check</p>
        <p class="mt-1 text-sm text-muted-foreground">Click "Run full check" to populate the matrix.</p>
      </div>
    </section>
    <section v-else class="grid min-h-0 flex-1 grid-cols-1 lg:grid-cols-[3fr_2fr]">
      <HealthMatrix class="border-r"
                    :machines="machines.machines"
                    :rows-by-machine="hc.rowsByMachine"
                    :selected-machine-id="selected?.machineId ?? null"
                    :selected-check-id="selected?.checkId ?? null"
                    @select="selected = $event" />
      <aside data-health-detail class="overflow-y-auto p-6">
        <div v-if="!selectedDetail">
          <p class="text-sm text-muted-foreground">Select a cell to view its diagnostic.</p>
        </div>
        <div v-else>
          <header class="flex items-center gap-3">
            <UecmStatusBadge :tone="(selectedDetail.outcome?.status as any) ?? 'unknown'"
                             :label="selectedDetail.outcome?.status ?? 'unknown'" size="md" />
            <h2 class="font-display text-lg font-extrabold">{{ selectedDetail.def?.label }}</h2>
          </header>
          <p class="mt-1 font-mono text-xs text-muted-foreground">
            {{ selectedDetail.machine?.hostname }} · {{ selectedDetail.machine?.ip }}
          </p>
          <div class="mt-4 space-y-3">
            <div class="rounded-md border bg-card p-3">
              <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">What this checks</p>
              <p class="mt-1 text-sm">{{ selectedDetail.def?.description }}</p>
            </div>
            <div class="rounded-md border bg-card p-3">
              <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">User-facing symptom</p>
              <p class="mt-1 text-sm">{{ selectedDetail.def?.symptom }}</p>
            </div>
            <div class="rounded-md border bg-card p-3">
              <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">How to fix</p>
              <p class="mt-1 text-sm">{{ selectedDetail.def?.remediation }}</p>
            </div>
            <div class="rounded-md border bg-card p-3">
              <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">Last probe output</p>
              <pre class="mt-1 font-mono text-xs text-muted-foreground whitespace-pre-wrap">{{ selectedDetail.outcome?.message }}</pre>
              <p v-if="selectedDetail.outcome?.sample" class="mt-1 font-mono text-[11px] text-muted-foreground">Sample: {{ selectedDetail.outcome.sample }}</p>
            </div>
          </div>
        </div>
      </aside>
    </section>

    <HealthCheckWizard :open="showWizard" @close="showWizard = false" />
  </div>
</template>
