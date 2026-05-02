<script setup lang="ts">
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
</script>

<template>
  <div class="space-y-6 p-6">
    <UecmPageHeader title="Projects" eyebrow="Unreal projects" description="Project inventory is not connected yet." />
    <section class="rounded-lg border bg-card p-8 text-center">
      <UecmIcon name="folder-git-2" size="32" class="mx-auto text-muted-foreground" />
      <h2 class="mt-4 font-display text-lg font-extrabold">No project inventory source</h2>
      <p class="mx-auto mt-2 max-w-xl text-sm text-muted-foreground">
        This view is ready for styling, but it is intentionally empty until a real project discovery API or store is wired in.
      </p>
    </section>
  </div>
</template>
