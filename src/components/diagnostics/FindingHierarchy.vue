<script setup lang="ts">
import type { IniFinding } from "@/services/tauri";
import UecmStatusBadge from "@/components/primitives/UecmStatusBadge.vue";

defineProps<{
  findings: IniFinding[];
  selectedId: number | null;
  hostnameById: Record<number, string>;
}>();
const emit = defineEmits<{ select: [finding: IniFinding] }>();
</script>

<template>
  <div data-finding-hierarchy class="overflow-y-auto">
    <button
      v-for="finding in findings"
      :key="finding.id ?? `${finding.machine_id}-${finding.rule_id}-${finding.file_path}`"
      data-finding-row
      class="block w-full border-b px-4 py-3 text-left hover:bg-accent"
      :class="selectedId === finding.id ? 'bg-accent' : ''"
      @click="emit('select', finding)"
    >
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <div class="truncate font-mono text-xs font-bold">{{ hostnameById[finding.machine_id] ?? finding.machine_id }}</div>
          <div class="truncate text-xs text-muted-foreground">{{ finding.file_path }}</div>
        </div>
        <UecmStatusBadge :tone="finding.severity" :label="finding.rule_id" size="sm" />
      </div>
      <div class="mt-2 text-sm">{{ finding.symptom }}</div>
    </button>
  </div>
</template>
