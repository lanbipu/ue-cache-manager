<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  before?: string;
  after?: string | null;
  code?: string;
  startLine?: number;
  highlightLine?: number;
  tone?: string;
  caption?: string;
}>();

const content = computed(() => props.before ?? props.code ?? "");
const lines = computed(() => content.value.split(/\r?\n/));
</script>

<template>
  <div data-code-block class="overflow-hidden rounded-md border bg-card">
    <div v-if="caption" class="border-b px-3 py-2 font-mono text-[11px] font-bold uppercase text-muted-foreground">{{ caption }}</div>
    <pre class="overflow-auto p-3 font-mono text-xs leading-5 text-foreground"><code v-if="content"><span
      v-for="(line, idx) in lines"
      :key="idx"
      class="block"
      :class="highlightLine === (startLine ?? 1) + idx ? 'bg-yellow-500/15' : ''"
    ><span class="mr-3 select-none text-muted-foreground">{{ (startLine ?? 1) + idx }}</span>{{ line }}</span></code><code v-else>(empty)</code></pre>
    <pre v-if="after" class="border-t bg-status-healthy/10 p-3 font-mono text-xs leading-5 text-status-healthy"><code>{{ after }}</code></pre>
  </div>
</template>
