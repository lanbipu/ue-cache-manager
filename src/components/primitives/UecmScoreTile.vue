<script setup lang="ts">
import UecmStatusBadge from "./UecmStatusBadge.vue";
import type { UecmTone } from "./types";

withDefaults(defineProps<{ label: string; score: number; verdict: string; tone?: UecmTone }>(), {
  tone: "info",
});
</script>

<template>
  <div data-score-tile class="min-h-20 bg-card p-4">
    <div class="flex items-center justify-between gap-3">
      <div class="font-mono text-[11px] font-bold uppercase text-muted-foreground">{{ label }}</div>
      <UecmStatusBadge :tone="tone" :label="verdict" size="sm" />
    </div>
    <div class="mt-2 font-display text-3xl font-extrabold">{{ score }}</div>
  </div>
</template>
