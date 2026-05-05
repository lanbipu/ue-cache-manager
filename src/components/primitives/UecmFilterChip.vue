<script setup lang="ts">
defineProps<{ label: string; value: string }>();
</script>

<template>
  <button data-filter-chip class="inline-flex items-center gap-1.5 rounded-full border bg-card px-3 py-1 text-xs hover:bg-accent">
    <span class="font-mono uppercase tracking-wide text-muted-foreground">{{ label }}:</span>
    <span class="font-bold">{{ value }}</span>
    <span aria-hidden="true">▾</span>
  </button>
</template>
