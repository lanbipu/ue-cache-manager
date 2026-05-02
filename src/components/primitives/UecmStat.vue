<script setup lang="ts">
import UecmIcon from "./UecmIcon.vue";

defineProps<{
  label: string;
  value: string | number;
  icon?: string;
  detail?: string;
}>();
</script>

<template>
  <article class="rounded-lg border bg-card p-4 text-card-foreground shadow-sm">
    <div class="flex items-center justify-between gap-3">
      <p class="text-xs font-bold uppercase tracking-wide text-muted-foreground">{{ label }}</p>
      <UecmIcon v-if="icon" :name="icon" class="text-muted-foreground" />
    </div>
    <div class="mt-3 font-display text-3xl font-extrabold">{{ value }}</div>
    <p v-if="detail" class="mt-1 text-xs text-muted-foreground">{{ detail }}</p>
  </article>
</template>
