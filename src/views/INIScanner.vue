<script setup lang="ts">
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
</script>

<template>
  <div class="space-y-6 p-6">
    <UecmPageHeader title="INI Scanner" eyebrow="Config drift" description="INI drift results are not connected yet." />
    <section class="rounded-lg border bg-card p-8 text-center">
      <UecmIcon name="file-search" size="32" class="mx-auto text-muted-foreground" />
      <h2 class="mt-4 font-display text-lg font-extrabold">No INI comparison results</h2>
      <p class="mx-auto mt-2 max-w-xl text-sm text-muted-foreground">
        Real DefaultEngine.ini diff output will appear here after scanner data is wired into the frontend.
      </p>
    </section>
  </div>
</template>
