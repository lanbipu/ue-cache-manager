<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
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

const { t } = useI18n();
const machines = useMachinesStore();
const hc = useHealthCheckStore();

const showWizard = ref(false);
const selected = ref<{ machineId: number; checkId: string } | null>(null);

onMounted(() => machines.loadMachines());

const score = computed(() => {
  const total = hc.summary.total || 1;
  return Math.max(0, Math.round(((hc.summary.healthy - hc.summary.critical * 0.75 - hc.summary.warning * 0.35) / total) * 100));
});
const tone = computed<"healthy" | "warning" | "critical" | "info">(() => {
  if (hc.summary.critical > 0) return "critical";
  if (hc.summary.warning > 0) return "warning";
  if (hc.summary.healthy > 0) return "healthy";
  return "info";
});
const verdict = computed(() => tone.value === "critical" ? t("healthCheck.verdictAttention")
                            : tone.value === "warning" ? t("healthCheck.verdictDegraded")
                            : tone.value === "healthy" ? t("healthCheck.verdictHealthy") : t("healthCheck.verdictIdle"));

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
      <UecmPageHeader :title="t('healthCheck.title')" :eyebrow="t('healthCheck.eyebrow')"
        :description="t('healthCheck.description')">
        <template #actions>
          <Button data-open-health-wizard-btn @click="showWizard = true">
            <UecmIcon name="play" /> {{ t("healthCheck.runFullCheck") }}
          </Button>
        </template>
      </UecmPageHeader>
      <section class="grid grid-cols-5 gap-px overflow-hidden rounded-lg border bg-border">
        <UecmScoreTile :label="t('healthCheck.kpiClusterScore')" :score="score" :tone="tone" :verdict="verdict" />
        <UecmKpiTile :label="t('healthCheck.kpiHealthy')"  :value="hc.summary.healthy"  tone="healthy" />
        <UecmKpiTile :label="t('healthCheck.kpiWarning')"  :value="hc.summary.warning"  tone="warning" />
        <UecmKpiTile :label="t('healthCheck.kpiCritical')" :value="hc.summary.critical" tone="critical" />
        <UecmKpiTile :label="t('healthCheck.kpiOffline')"  :value="hc.summary.offline"  tone="offline" />
      </section>
    </div>

    <section v-if="machines.machines.length === 0" class="grid flex-1 place-items-center text-center">
      <p class="text-sm text-muted-foreground">{{ t("healthCheck.noMachines") }}</p>
    </section>
    <section v-else-if="hc.scanRunId === null" class="grid flex-1 place-items-center text-center">
      <div>
        <UecmIcon name="heart-pulse" size="32" class="mx-auto text-muted-foreground" />
        <p class="mt-2 font-display text-lg font-extrabold">{{ t("healthCheck.emptyTitle") }}</p>
        <p class="mt-1 text-sm text-muted-foreground">{{ t("healthCheck.emptyHint") }}</p>
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
          <p class="text-sm text-muted-foreground">{{ t("healthCheck.selectCellHint") }}</p>
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
              <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">{{ t("healthCheck.detailWhatChecks") }}</p>
              <p class="mt-1 text-sm">{{ selectedDetail.def?.description }}</p>
            </div>
            <div class="rounded-md border bg-card p-3">
              <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">{{ t("healthCheck.detailSymptom") }}</p>
              <p class="mt-1 text-sm">{{ selectedDetail.def?.symptom }}</p>
            </div>
            <div class="rounded-md border bg-card p-3">
              <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">{{ t("healthCheck.detailHowToFix") }}</p>
              <p class="mt-1 text-sm">{{ selectedDetail.def?.remediation }}</p>
            </div>
            <div class="rounded-md border bg-card p-3">
              <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">{{ t("healthCheck.detailLastProbe") }}</p>
              <pre class="mt-1 font-mono text-xs text-muted-foreground whitespace-pre-wrap">{{ selectedDetail.outcome?.message }}</pre>
              <p v-if="selectedDetail.outcome?.sample" class="mt-1 font-mono text-[11px] text-muted-foreground">{{ t("healthCheck.detailSamplePrefix", { value: selectedDetail.outcome.sample }) }}</p>
            </div>
          </div>
        </div>
      </aside>
    </section>

    <HealthCheckWizard :open="showWizard" @close="showWizard = false" />
  </div>
</template>
