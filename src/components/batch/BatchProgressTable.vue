<script setup lang="ts">
import type { BatchEvent, Machine } from "@/services/tauri";

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
  if (status === "ok") return "text-green-600";
  if (status === "err") return "text-red-600";
  if (status === "running") return "text-blue-600";
  return "text-gray-400";
}
</script>

<template>
  <table data-batch-progress class="w-full text-sm border">
    <thead class="bg-gray-50">
      <tr>
        <th class="text-left px-2 py-1 w-8"></th>
        <th class="text-left px-2 py-1">Machine</th>
        <th class="text-left px-2 py-1">Message</th>
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
          <span class="text-xs text-gray-500 ml-1">{{ m.ip }}</span>
        </td>
        <td class="px-2 py-1 text-xs text-gray-600 truncate max-w-xs">
          {{ m.id != null ? (byMachine.get(m.id)?.message ?? "") : "" }}
        </td>
      </tr>
    </tbody>
  </table>
</template>
