<script setup lang="ts">
import { computed } from "vue";
import type { UecmTone } from "./types";

const props = withDefaults(defineProps<{
  code: string;
  tone?: UecmTone;
  startLine?: number;
  highlightLine?: number;
  caption?: string;
}>(), { tone: "info", startLine: 1 });

const lines = computed(() => props.code.split("\n").map((text, i) => ({
  number: props.startLine + i,
  text,
  highlighted: props.highlightLine !== undefined && (props.startLine + i) === props.highlightLine,
})));

const toneCls = computed(() => {
  const map: Record<UecmTone, string> = {
    healthy: "border-status-healthy/30 bg-status-healthy/5",
    warning: "border-status-warning/30 bg-status-warning/5",
    critical: "border-status-critical/30 bg-status-critical/5",
    info: "border-status-info/30 bg-status-info/5",
    offline: "border-muted bg-muted/20",
    unknown: "border-muted bg-muted/20",
    progress: "border-status-info/30 bg-status-info/5",
    na: "border-muted bg-muted/20",
  };
  return map[props.tone];
});
</script>

<template>
  <div data-codeblock class="overflow-hidden rounded-md border" :class="toneCls">
    <div v-if="caption" class="border-b bg-muted/30 px-3 py-1.5 text-xs font-bold uppercase tracking-wide text-muted-foreground">
      {{ caption }}
    </div>
    <pre class="overflow-x-auto p-3 font-mono text-xs leading-5"><code><div
        v-for="line in lines" :key="line.number"
        :class="line.highlighted ? 'bg-yellow-500/15' : ''"
        class="flex"
      ><span class="mr-3 inline-block min-w-[2rem] select-none text-right text-muted-foreground">{{ line.number }}</span><span class="whitespace-pre">{{ line.text }}</span></div></code></pre>
  </div>
</template>
