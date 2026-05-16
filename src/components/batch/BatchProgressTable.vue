<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { BatchEvent, Machine } from "@/services/tauri";

const { t } = useI18n();

defineProps<{
  machines: Machine[];
  byMachine: Map<number, BatchEvent>;
}>();

function statusGlyph(status: string | undefined): string {
  if (status === "ok") return "✓";
  if (status === "err") return "✗";
  if (status === "running") return "↻";
  return "—";
}

function statusClass(status: string | undefined): string {
  if (status === "ok") return "text-emerald-600 dark:text-emerald-400";
  if (status === "err") return "text-destructive";
  if (status === "running") return "text-primary";
  return "text-muted-foreground";
}
</script>

<template>
  <table data-batch-progress class="w-full text-sm border">
    <thead class="bg-muted/40 text-muted-foreground">
      <tr>
        <th class="text-left px-2 py-1 w-8"></th>
        <th class="text-left px-2 py-1 font-medium">{{ t("batchTable.headerMachine") }}</th>
        <th class="text-left px-2 py-1 font-medium">{{ t("batchTable.headerMessage") }}</th>
      </tr>
    </thead>
    <tbody>
      <tr
        v-for="m in machines"
        :key="m.id ?? m.ip"
        data-batch-row
        class="border-t"
      >
        <td
          class="px-2 py-1 text-center"
          :class="statusClass(m.id != null ? byMachine.get(m.id)?.status : undefined)"
        >
          {{ statusGlyph(m.id != null ? byMachine.get(m.id)?.status : undefined) }}
        </td>
        <td class="px-2 py-1">
          {{ m.hostname }}
          <span class="text-xs text-muted-foreground ml-1">{{ m.ip }}</span>
        </td>
        <td class="px-2 py-1 text-xs text-muted-foreground truncate max-w-xs">
          {{ m.id != null ? (byMachine.get(m.id)?.message ?? "") : "" }}
        </td>
      </tr>
    </tbody>
  </table>
</template>
