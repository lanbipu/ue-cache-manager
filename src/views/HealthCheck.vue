<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import UecmKpiTile from "@/components/primitives/UecmKpiTile.vue";
import UecmGpuMatrix from "@/components/primitives/UecmGpuMatrix.vue";
import UecmScoreTile from "@/components/primitives/UecmScoreTile.vue";
import UecmStatusBadge from "@/components/primitives/UecmStatusBadge.vue";
import Button from "@/components/ui/Button.vue";
import HealthMatrix from "@/components/diagnostics/HealthMatrix.vue";
import HealthCheckWizard from "@/components/modals/HealthCheckWizard.vue";
import { HEALTH_CHECKS, type HealthCheckDefinition } from "@/lib/healthChecks";
import { useGpuConsistencyStore } from "@/stores/gpuConsistency";
import { useHealthCheckStore } from "@/stores/healthCheck";
import { useMachinesStore } from "@/stores/machines";
import { tauriApi, type EchoResult, type UecmError } from "@/services/tauri";

const { t } = useI18n();
const machines = useMachinesStore();
const health = useHealthCheckStore();
const gpuStore = useGpuConsistencyStore();
const route = useRoute();
const router = useRouter();

function gotoTab(tab: string, extra: Record<string, string> = {}) {
  router.push({
    path: "/machines",
    query: { ...route.query, ...extra, tab },
  });
}
const showWizard = ref(false);
const selected = ref<{ machineId: number; checkId: string } | null>(null);

const bridgeResult = ref<EchoResult | null>(null);
const bridgeError = ref<UecmError | null>(null);
const bridgeLoading = ref(false);

async function runBridgeTest() {
  bridgeResult.value = null;
  bridgeError.value = null;
  bridgeLoading.value = true;
  try {
    bridgeResult.value = await tauriApi.testPowerShellBridge("hello from UECM");
  } catch (e) {
    bridgeError.value = e as UecmError;
  } finally {
    bridgeLoading.value = false;
  }
}

onMounted(() => {
  void Promise.all([machines.loadMachines(), gpuStore.load()]);
});

const score = computed(() => {
  const total = health.summary.total || 1;
  return Math.max(0, Math.round(((health.summary.healthy - health.summary.critical * 0.75 - health.summary.warning * 0.35) / total) * 100));
});
const tone = computed(() => health.summary.critical > 0 ? "critical" : health.summary.warning > 0 ? "warning" : health.summary.healthy > 0 ? "healthy" : "info");
const verdict = computed(() => {
  if (tone.value === "critical") return t("healthCheck.verdictAttention");
  if (tone.value === "warning") return t("healthCheck.verdictDegraded");
  if (tone.value === "healthy") return t("healthCheck.verdictHealthy");
  return t("healthCheck.verdictIdle");
});
const selectedDetail = computed(() => {
  if (!selected.value) return null;
  const def = HEALTH_CHECKS.find((check) => check.id === selected.value?.checkId) as
    | HealthCheckDefinition
    | undefined;
  const outcome = health.rowsByMachine[selected.value.machineId]?.[selected.value.checkId];
  const machine = machines.machines.find((m) => m.id === selected.value?.machineId);
  return { def, outcome, machine };
});
</script>

<template>
  <div class="flex h-full flex-col">
    <div class="space-y-4 p-6">
      <UecmPageHeader :title="t('healthCheck.title')" :eyebrow="t('healthCheck.eyebrow')" :description="t('healthCheck.description')">
        <template #actions>
          <Button
            variant="outline"
            data-bridge-test-btn
            :disabled="bridgeLoading"
            @click="runBridgeTest"
          >
            <UecmIcon name="terminal" />
            {{ bridgeLoading ? t("dashboard.runningBridgeTest") : t("dashboard.runBridgeTest") }}
          </Button>
          <Button data-open-health-wizard-btn @click="showWizard = true">
            <UecmIcon name="play" /> {{ t("healthCheck.runFullCheck") }}
          </Button>
        </template>
      </UecmPageHeader>

      <div
        v-if="bridgeResult || bridgeError"
        data-bridge-result
        class="rounded-md border bg-card p-3 text-xs"
      >
        <pre
          v-if="bridgeResult"
          class="overflow-auto font-mono text-muted-foreground"
        >{{ JSON.stringify(bridgeResult, null, 2) }}</pre>
        <p v-if="bridgeError" class="text-destructive">
          {{ bridgeError.code }}: {{ bridgeError.message }}
        </p>
      </div>
      <section class="grid grid-cols-5 gap-px overflow-hidden rounded-lg border bg-border">
        <UecmScoreTile :label="t('healthCheck.kpiClusterScore')" :score="score" :tone="tone" :verdict="verdict" />
        <UecmKpiTile :label="t('healthCheck.kpiHealthy')" :value="health.summary.healthy" tone="healthy" />
        <UecmKpiTile :label="t('healthCheck.kpiWarning')" :value="health.summary.warning" tone="warning" />
        <UecmKpiTile :label="t('healthCheck.kpiCritical')" :value="health.summary.critical" tone="critical" />
        <UecmKpiTile :label="t('healthCheck.kpiOffline')" :value="health.summary.offline" tone="offline" />
      </section>
    </div>

    <section v-if="machines.machines.length === 0" class="grid flex-1 place-items-center text-center">
      <p class="text-sm text-muted-foreground">{{ t("healthCheck.noMachines") }}</p>
    </section>
    <section v-else-if="health.scanRunId === null" class="grid flex-1 place-items-center text-center">
      <div>
        <UecmIcon name="heart-pulse" size="32" class="mx-auto text-muted-foreground" />
        <h2 class="mt-4 font-display text-lg font-extrabold">{{ t("healthCheck.emptyTitle") }}</h2>
        <p class="mx-auto mt-2 max-w-xl text-sm text-muted-foreground">{{ t("healthCheck.emptyHint") }}</p>
      </div>
    </section>
    <section v-else class="grid min-h-0 flex-1 grid-cols-1 lg:grid-cols-[3fr_2fr]">
      <HealthMatrix class="border-r" :machines="machines.machines" :rows-by-machine="health.rowsByMachine" :selected-machine-id="selected?.machineId ?? null" :selected-check-id="selected?.checkId ?? null" @select="selected = $event" />
      <aside data-health-detail class="overflow-y-auto p-6">
        <p v-if="!selectedDetail" class="text-sm text-muted-foreground">{{ t("healthCheck.selectCellHint") }}</p>
        <div v-else class="space-y-4">
          <header class="flex items-center gap-3">
            <UecmStatusBadge :tone="selectedDetail.outcome?.status ?? 'unknown'" :label="selectedDetail.outcome?.status ?? t('healthCheck.unknownLabel')" />
            <div>
              <h2 class="font-display text-lg font-extrabold">{{ selectedDetail.def?.label }}</h2>
              <p v-if="selectedDetail.def?.subtitle" class="text-xs text-muted-foreground">{{ selectedDetail.def.subtitle }}</p>
              <p class="font-mono text-xs text-muted-foreground">{{ selectedDetail.machine?.hostname }} · {{ selectedDetail.machine?.ip }}</p>
            </div>
          </header>
          <div v-if="selectedDetail.def?.id === 'pso_precaching'" class="rounded-md border bg-card p-3">
            <button
              type="button"
              class="text-sm font-bold text-primary hover:underline"
              @click="gotoTab('ini', { finding: 'R008' })"
            >
              {{ t("healthCheck.openIniScannerLink") }}
            </button>
          </div>
          <div v-else-if="selectedDetail.def?.id === 'gpu_consistency'" class="rounded-md border bg-card p-3">
            <button
              type="button"
              class="text-sm font-bold text-primary hover:underline"
              @click="gotoTab('health', { gpu: 'true' })"
            >
              {{ t("healthCheck.openGpuMatrixLink") }}
            </button>
          </div>
          <div class="rounded-md border bg-card p-3">
            <div class="font-mono text-[11px] font-bold uppercase text-muted-foreground">{{ t("healthCheck.detailWhatChecks") }}</div>
            <p class="mt-1 text-sm">{{ selectedDetail.def?.description }}</p>
          </div>
          <div class="rounded-md border bg-card p-3">
            <div class="font-mono text-[11px] font-bold uppercase text-muted-foreground">{{ t("healthCheck.detailSymptom") }}</div>
            <p class="mt-1 text-sm">{{ selectedDetail.def?.symptom }}</p>
          </div>
          <div class="rounded-md border bg-card p-3">
            <div class="font-mono text-[11px] font-bold uppercase text-muted-foreground">{{ t("healthCheck.detailHowToFix") }}</div>
            <p class="mt-1 text-sm">{{ selectedDetail.outcome?.remediation ?? selectedDetail.def?.remediation }}</p>
          </div>
          <pre class="rounded-md border bg-card p-3 font-mono text-xs text-muted-foreground whitespace-pre-wrap">{{ selectedDetail.outcome?.message ?? t("healthCheck.noProbeOutput") }}</pre>
        </div>
      </aside>
    </section>

    <section data-health-gpu-section class="space-y-2 border-t p-6">
      <h2 class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">{{ t("healthCheck.gpuMatrixSection") }}</h2>
      <p class="text-xs text-muted-foreground">
        {{ t("healthCheck.gpuMatrixBaselineHint", { baseline: gpuStore.baselineLabel, count: gpuStore.deviationCount }) }}
      </p>
      <UecmGpuMatrix :matrix="gpuStore.matrix" />
    </section>

    <HealthCheckWizard :open="showWizard" @close="showWizard = false" />
  </div>
</template>
