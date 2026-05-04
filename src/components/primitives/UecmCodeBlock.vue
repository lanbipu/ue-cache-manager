<script setup lang="ts">
defineProps<{
  before: string;
  after?: string | null;
}>();
</script>

<template>
  <div data-code-block class="overflow-hidden rounded-md border bg-card">
    <pre class="overflow-auto p-3 font-mono text-xs leading-5 text-foreground"><code>{{ before || "(empty)" }}</code></pre>
    <pre v-if="after" class="border-t bg-status-healthy/10 p-3 font-mono text-xs leading-5 text-status-healthy"><code>{{ after }}</code></pre>
  </div>
</template>
