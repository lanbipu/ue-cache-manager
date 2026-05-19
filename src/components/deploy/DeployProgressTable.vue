<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { DeployStep } from "@/lib/deployApi";

interface HostCell { state: string; message: string | null }
interface StepStat { hosts: Record<string, HostCell> }

const props = defineProps<{
  steps: DeployStep[];
  status: Record<string, StepStat>;
}>();
const { t } = useI18n();

const hosts = computed<string[]>(() => {
  const set = new Set<string>();
  for (const s of props.steps) {
    const p = props.status[s];
    if (p) for (const h of Object.keys(p.hosts)) set.add(h);
  }
  return Array.from(set).sort();
});

function cell(step: DeployStep, host: string): HostCell | null {
  return props.status[step]?.hosts[host] ?? null;
}

function toneClass(state?: string): string {
  if (state === "ok") return "bg-status-healthy/20 text-status-healthy";
  if (state === "error") return "bg-status-critical/20 text-status-critical";
  if (state === "running") return "bg-status-info/20 text-status-info";
  return "bg-muted/40 text-muted-foreground";
}
</script>

<template>
  <table class="w-full text-xs border-collapse">
    <thead>
      <tr>
        <th class="text-left p-1 font-normal text-muted-foreground">{{ t("deploy.step.header") }}</th>
        <th v-for="h in hosts" :key="h" class="p-1 text-left font-mono">{{ h }}</th>
      </tr>
    </thead>
    <tbody>
      <tr v-for="s in steps" :key="s" class="border-t border-border">
        <td class="p-1 pr-2">{{ t(`deploy.step.${s}`) }}</td>
        <td v-for="h in hosts" :key="h" class="p-1">
          <span :class="['inline-block rounded px-1.5 py-0.5', toneClass(cell(s, h)?.state)]"
                :title="cell(s, h)?.message ?? ''">
            {{ cell(s, h)?.state ?? "-" }}
          </span>
        </td>
      </tr>
    </tbody>
  </table>
</template>
