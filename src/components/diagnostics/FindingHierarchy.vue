<script setup lang="ts">
import { computed } from "vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import UecmStatusBadge from "@/components/primitives/UecmStatusBadge.vue";
import type { IniFinding } from "@/services/tauri";

const props = defineProps<{
  findings: IniFinding[];
  selectedId: number | null;
  hostnameById: Record<number, string>;
  groupBy: "machine" | "category";
}>();
const emit = defineEmits<{ select: [finding: IniFinding] }>();

function group<T>(arr: T[], fn: (t: T) => string): Record<string, T[]> {
  const out: Record<string, T[]> = {};
  for (const x of arr) {
    const k = fn(x);
    (out[k] ||= []).push(x);
  }
  return out;
}

const tree = computed(() => {
  const top = props.groupBy === "machine"
    ? group(props.findings, f => String(f.machine_id))
    : group(props.findings, f => f.category);
  return Object.entries(top).map(([k, items]) => {
    const byFile = group(items, f => f.file_path);
    const counts = items.reduce((acc, f) => {
      if (f.severity === "critical") acc.c++; else if (f.severity === "warning") acc.w++;
      return acc;
    }, { c: 0, w: 0 });
    return {
      key: k,
      label: props.groupBy === "machine" ? (props.hostnameById[Number(k)] ?? `#${k}`) : k.toUpperCase(),
      counts,
      files: Object.entries(byFile).map(([fp, fItems]) => ({ filePath: fp, items: fItems })),
    };
  });
});
</script>

<template>
  <div data-finding-hierarchy class="overflow-y-auto">
    <div v-for="grp in tree" :key="grp.key" class="border-b last:border-b-0">
      <div class="sticky top-0 z-10 flex items-center gap-2 bg-muted/40 px-4 py-2 text-sm font-bold">
        <UecmIcon name="server" size="14" />
        {{ grp.label }}
        <UecmStatusBadge v-if="grp.counts.c" tone="critical" :label="`${grp.counts.c}C`" size="sm" />
        <UecmStatusBadge v-if="grp.counts.w" tone="warning" :label="`${grp.counts.w}W`" size="sm" />
      </div>
      <div v-for="file in grp.files" :key="file.filePath" class="border-t">
        <div class="flex items-center gap-2 px-7 py-1.5 text-xs">
          <UecmIcon name="file-text" size="12" class="text-muted-foreground" />
          <span class="font-mono text-muted-foreground">{{ file.filePath.split('\\').slice(-2).join('\\') }}</span>
        </div>
        <button
          v-for="f in file.items"
          :key="f.id ?? `${f.scan_run_id}-${f.line_number}`"
          data-finding-row
          class="flex w-full items-center gap-2 px-12 py-1.5 text-xs hover:bg-accent/50"
          :class="props.selectedId === f.id ? 'bg-accent' : ''"
          @click="emit('select', f)"
        >
          <UecmStatusBadge :tone="f.severity" :label="f.severity[0].toUpperCase()" size="sm" />
          <span class="flex-1 truncate text-left">{{ f.rule_id }} · {{ f.section }}</span>
          <span class="font-mono text-[10px] text-muted-foreground">L{{ f.line_number ?? '?' }}</span>
        </button>
      </div>
    </div>
  </div>
</template>
