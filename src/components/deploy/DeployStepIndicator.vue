<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { DeployStep } from "@/lib/deployApi";
import UecmIcon from "@/components/primitives/UecmIcon.vue";

interface HostState { state: string }
interface StepStat { ok_count: number; fail_count: number; hosts: Record<string, HostState> }

defineProps<{
  steps: DeployStep[];
  status: Record<string, StepStat>;
}>();

const { t } = useI18n();

function toneOf(p?: StepStat): string {
  if (!p || Object.keys(p.hosts).length === 0) return "muted";
  if (p.fail_count > 0) return "critical";
  if (Object.values(p.hosts).some((h) => h.state === "running")) return "info";
  if (Object.values(p.hosts).every((h) => h.state === "ok")) return "healthy";
  return "muted";
}

function iconFor(tone: string): string {
  if (tone === "healthy") return "check";
  if (tone === "critical") return "alert-triangle";
  if (tone === "info") return "circle";
  return "circle";
}

function classFor(tone: string): string {
  if (tone === "healthy") return "text-status-healthy";
  if (tone === "critical") return "text-status-critical";
  if (tone === "info") return "text-status-info";
  return "text-muted-foreground";
}
</script>

<template>
  <ol class="space-y-2">
    <li v-for="(s, i) in steps" :key="s" class="flex items-start gap-2 text-sm">
      <span class="mt-0.5 text-muted-foreground tabular-nums">{{ String(i + 1).padStart(2, "0") }}</span>
      <UecmIcon :name="iconFor(toneOf(status[s]))" :class="['mt-0.5', classFor(toneOf(status[s]))]" />
      <div class="flex-1">
        <div>{{ t(`deploy.step.${s}`) }}</div>
        <div v-if="status[s] && Object.keys(status[s].hosts).length" class="text-xs text-muted-foreground">
          {{ status[s].ok_count }} ok · {{ status[s].fail_count }} fail
        </div>
      </div>
    </li>
  </ol>
</template>
