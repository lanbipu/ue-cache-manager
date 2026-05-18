<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import UecmKpiTile from "@/components/primitives/UecmKpiTile.vue";
import UecmGpuMatrix from "@/components/primitives/UecmGpuMatrix.vue";
import UecmScoreTile from "@/components/primitives/UecmScoreTile.vue";
import UecmStatusBadge from "@/components/primitives/UecmStatusBadge.vue";
import Button from "@/components/ui/Button.vue";
import HealthCheckWizard from "@/components/modals/HealthCheckWizard.vue";
import { useGpuConsistencyStore } from "@/stores/gpuConsistency";
import { useHealthCheckStore } from "@/stores/healthCheck";
import { useMachinesStore } from "@/stores/machines";
import { tauriApi, type EchoResult, type UecmError } from "@/services/tauri";

const { t } = useI18n();
const machines = useMachinesStore();
const health = useHealthCheckStore();
const gpuStore = useGpuConsistencyStore();

const showWizard = ref(false);

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
const LAYER_ORDER = ["l1_port", "l2_bootstrap", "l3_business"] as const;
const LAYER_FALLBACK: Record<(typeof LAYER_ORDER)[number], string> = {
  l1_port: "L1 - Port reachability",
  l2_bootstrap: "L2 - Bootstrap configuration",
  l3_business: "L3 - Business workflow",
};

function layerLabel(layer: (typeof LAYER_ORDER)[number]): string {
  const key = `healthCheck.layer.${layer}`;
  const translated = t(key);
  return translated === key ? LAYER_FALLBACK[layer] : translated;
}

function probeLabel(probeKey: string): string {
  const i18nKey = `healthCheck.probe.${probeKey}`;
  const translated = t(i18nKey);
  // vue-i18n returns the key string itself when no entry matches —
  // fall back to the raw probe key (still human-readable, e.g. "tcp_5985").
  return translated === i18nKey ? probeKey : translated;
}

function toneFor(status: string): "healthy" | "warning" | "critical" | "offline" | "unknown" | "na" {
  switch (status) {
    case "healthy":
    case "warning":
    case "critical":
    case "offline":
    case "unknown":
    case "na":
      return status;
    default:
      return "unknown";
  }
}
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
    <section v-else class="min-h-0 flex-1">
      <div data-health-layers class="space-y-6 border-b p-6">
        <div v-for="machine in machines.machines.filter((m) => m.id != null)" :key="`layered-${machine.id}`" class="space-y-2">
          <header class="flex items-baseline gap-2">
            <h3 class="font-display text-sm font-extrabold">{{ machine.hostname }}</h3>
            <span class="font-mono text-xs text-muted-foreground">{{ machine.ip }}</span>
          </header>
          <template v-for="layer in LAYER_ORDER" :key="`${machine.id}-${layer}`">
            <section class="mt-4">
              <h4 class="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                {{ layerLabel(layer) }}
              </h4>
              <table class="w-full text-sm">
                <tbody>
                  <template
                    v-for="probe in health.probesByLayer[machine.id as number]?.[layer] ?? []"
                    :key="probe.key"
                  >
                    <tr class="border-t border-border" :data-probe-key="probe.key">
                      <td class="px-2 py-1 text-foreground">
                        {{ probeLabel(probe.key) }}
                        <span class="ml-2 font-mono text-xs text-muted-foreground">{{ probe.key }}</span>
                      </td>
                      <td class="px-2 py-1">
                        <UecmStatusBadge :tone="toneFor(probe.outcome.status)" :label="probe.outcome.status" />
                      </td>
                      <td class="px-2 py-1 text-foreground">{{ probe.outcome.message }}</td>
                    </tr>
                    <tr
                      v-if="probe.outcome.remediation && (probe.outcome.status === 'critical' || probe.outcome.status === 'warning')"
                      class="border-t border-border/50"
                      :data-probe-remediation="probe.key"
                    >
                      <td></td>
                      <td colspan="2" class="px-2 py-1 text-xs text-muted-foreground">
                        <UecmIcon name="wrench" class="mr-1 inline" />
                        {{ probe.outcome.remediation }}
                      </td>
                    </tr>
                  </template>
                </tbody>
              </table>
            </section>
          </template>
        </div>
      </div>
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
