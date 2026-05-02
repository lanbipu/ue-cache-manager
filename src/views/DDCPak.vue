<script setup lang="ts">
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";

const steps = ["Select project", "Resolve engine", "Validate DDC", "Collect artifacts", "Compress pak", "Distribute", "Verify clients", "Archive report"];
</script>

<template>
  <div class="space-y-6 p-6">
    <UecmPageHeader title="DDC Pak" eyebrow="Artifact workflow" description="Package and distribute Derived Data Cache artifacts for render nodes." />
    <section class="rounded-lg border bg-card p-4">
      <div class="grid gap-3 md:grid-cols-4">
        <div v-for="(step, index) in steps" :key="step" class="rounded-md border bg-background p-3">
          <div class="flex items-center gap-2 text-sm font-bold">
            <span class="inline-flex size-6 items-center justify-center rounded-full bg-muted text-xs text-muted-foreground">{{ index + 1 }}</span>
            {{ step }}
          </div>
          <p class="mt-2 text-xs text-muted-foreground">Waiting for a real project selection.</p>
        </div>
      </div>
    </section>
    <section class="rounded-lg border bg-card p-8 text-center">
      <UecmIcon name="package" size="32" class="mx-auto text-muted-foreground" />
      <h2 class="mt-4 font-display text-lg font-extrabold">No DDC package queued</h2>
      <p class="mx-auto mt-2 max-w-xl text-sm text-muted-foreground">
        Package history will appear here after the workflow is connected to real project and artifact data.
      </p>
    </section>
  </div>
</template>
