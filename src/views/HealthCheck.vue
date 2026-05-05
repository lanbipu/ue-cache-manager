<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmMatrixCell from "@/components/primitives/UecmMatrixCell.vue";
import UecmStat from "@/components/primitives/UecmStat.vue";
import { HEALTH_CHECKS, type StatusKind } from "@/lib/healthChecks";
import { useMachinesStore } from "@/stores/machines";

const { t } = useI18n();
const machines = useMachinesStore();

function statusTone(status: string): StatusKind {
  const normalized = status.toLowerCase();
  if (normalized === "online") return "healthy";
  if (normalized === "offline") return "offline";
  if (["critical", "error", "failed"].includes(normalized)) return "critical";
  if (["warning", "warn", "degraded"].includes(normalized)) return "warning";
  return "unknown";
}

const healthy = computed(() => machines.machines.filter((machine) => statusTone(machine.status) === "healthy").length);
const warnings = computed(() => machines.machines.filter((machine) => statusTone(machine.status) === "warning").length);
const critical = computed(() => machines.machines.filter((machine) => statusTone(machine.status) === "critical").length);

onMounted(() => {
  machines.loadMachines();
});
</script>

<template>
  <div class="space-y-6 p-6">
    <UecmPageHeader :title="t('healthCheck.title')" :eyebrow="t('healthCheck.eyebrow')" :description="t('healthCheck.description')" />
    <section class="grid gap-4 md:grid-cols-3">
      <UecmStat :label="t('healthCheck.healthy')" :value="healthy" icon="shield-check" />
      <UecmStat :label="t('healthCheck.warnings')" :value="warnings" icon="info" />
      <UecmStat :label="t('healthCheck.critical')" :value="critical" icon="alert-triangle" />
    </section>
    <div class="overflow-auto rounded-lg border bg-card">
      <p v-if="machines.machines.length === 0" class="p-6 text-sm text-muted-foreground">
        {{ t("healthCheck.noMachines") }}
      </p>
      <table v-else class="min-w-[900px] w-full text-sm">
        <thead class="bg-muted text-muted-foreground">
          <tr>
            <th class="sticky left-0 bg-muted px-4 py-3 text-left">{{ t("healthCheck.headerMachine") }}</th>
            <th v-for="check in HEALTH_CHECKS" :key="check.id" class="px-3 py-3 text-center">{{ check.label }}</th>
          </tr>
        </thead>
        <tbody class="divide-y">
          <tr v-for="machine in machines.machines" :key="machine.id ?? machine.ip">
            <td class="sticky left-0 bg-card px-4 py-3">
              <div class="font-bold">{{ machine.hostname }}</div>
              <div class="font-mono text-xs text-muted-foreground">{{ machine.ip }}</div>
            </td>
            <td v-for="check in HEALTH_CHECKS" :key="check.id" class="px-3 py-2 text-center">
              <UecmMatrixCell :tone="statusTone(machine.status)" />
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
