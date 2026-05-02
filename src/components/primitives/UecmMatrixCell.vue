<script setup lang="ts">
import { computed } from "vue";
import type { UecmTone } from "./types";

const props = defineProps<{ tone: UecmTone; label?: string }>();

const cls = computed(() => {
  const map: Record<UecmTone, string> = {
    healthy: "bg-status-healthy/20 text-status-healthy",
    warning: "bg-status-warning/20 text-status-warning",
    critical: "bg-status-critical/20 text-status-critical",
    info: "bg-status-info/20 text-status-info",
    offline: "bg-status-offline/20 text-muted-foreground",
    unknown: "bg-hatched text-muted-foreground",
    progress: "bg-status-info/20 text-status-info",
    na: "bg-muted text-muted-foreground",
  };
  return map[props.tone];
});
</script>

<template>
  <span class="inline-flex size-8 items-center justify-center rounded border text-[10px] font-bold uppercase" :class="cls">
    {{ label ?? tone.slice(0, 2) }}
  </span>
</template>
