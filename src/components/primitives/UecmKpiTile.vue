<script setup lang="ts">
import { computed } from "vue";
import type { UecmTone } from "./types";

const props = defineProps<{ label: string; value: number | string; tone?: UecmTone }>();
const cls = computed(() => {
  const tone = props.tone ?? "info";
  return {
    healthy: "text-status-healthy",
    warning: "text-status-warning",
    critical: "text-status-critical",
    info: "text-status-info",
    offline: "text-muted-foreground",
    unknown: "text-muted-foreground",
    progress: "text-status-info",
    na: "text-muted-foreground",
  }[tone];
});
</script>

<template>
  <div data-kpi-tile class="min-h-20 bg-card p-4">
    <div class="font-mono text-[11px] font-bold uppercase text-muted-foreground">{{ label }}</div>
    <div class="mt-2 font-display text-2xl font-extrabold" :class="cls">{{ value }}</div>
  </div>
</template>
