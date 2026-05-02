<script setup lang="ts">
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
</script>

<template>
  <div class="space-y-6 p-6">
    <UecmPageHeader title="PSO Cache" eyebrow="Shader pipeline" description="PSO cache checks are not connected yet." />
    <section class="rounded-lg border bg-card p-8 text-center">
      <UecmIcon name="database" size="32" class="mx-auto text-muted-foreground" />
      <h2 class="mt-4 font-display text-lg font-extrabold">No PSO scan data</h2>
      <p class="mx-auto mt-2 max-w-xl text-sm text-muted-foreground">
        CVar and cache readiness rows will appear here after project scan results are backed by a real store.
      </p>
    </section>
  </div>
</template>
