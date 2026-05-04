<script setup lang="ts">
defineProps<{ label: string; value: string }>();
</script>

<template>
  <button data-filter-chip class="inline-flex h-8 items-center gap-2 rounded-md border bg-card px-3 font-mono text-xs uppercase text-muted-foreground hover:bg-accent">
    <span>{{ label }}</span>
    <span class="font-bold text-foreground">{{ value }}</span>
    <span>▾</span>
  </button>
</template>
