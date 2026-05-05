<script setup lang="ts">
import type { UecmTone } from "./types";

defineProps<{ label: string; value: number | string; tone?: UecmTone; sublabel?: string }>();
</script>

<template>
  <article data-kpi-tile class="bg-card px-4 py-3">
    <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">{{ label.toUpperCase() }}</p>
    <div
      class="mt-1 font-display text-3xl font-extrabold"
      :class="{
        'text-status-healthy': tone === 'healthy',
        'text-status-warning': tone === 'warning',
        'text-status-critical': tone === 'critical',
        'text-status-info': tone === 'info',
        'text-muted-foreground': tone === 'offline' || tone === 'unknown' || tone === 'na',
      }"
    >{{ value }}</div>
    <p v-if="sublabel" class="mt-0.5 text-xs text-muted-foreground">{{ sublabel }}</p>
  </article>
</template>
