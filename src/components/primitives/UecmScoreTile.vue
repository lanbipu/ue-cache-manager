<script setup lang="ts">
import { computed } from "vue";
import type { UecmTone } from "./types";

const props = defineProps<{ label: string; score: number; tone: UecmTone; verdict: string }>();

const ringCls = computed(() => ({
  healthy: "ring-status-healthy",
  warning: "ring-status-warning",
  critical: "ring-status-critical",
  info: "ring-status-info",
  offline: "ring-muted",
  unknown: "ring-muted",
  progress: "ring-status-info",
  na: "ring-muted",
}[props.tone]));
</script>

<template>
  <article data-score-tile class="flex items-center gap-4 bg-card px-4 py-3">
    <div :class="['flex size-14 items-center justify-center rounded-full ring-2', ringCls]">
      <span class="font-display text-2xl font-extrabold">{{ score }}</span>
    </div>
    <div>
      <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">{{ label }}</p>
      <p class="mt-0.5 text-sm font-bold uppercase">{{ verdict }}</p>
    </div>
  </article>
</template>
