<script setup lang="ts">
import UecmMatrixCell from "@/components/primitives/UecmMatrixCell.vue";
import UecmStatusDot from "@/components/primitives/UecmStatusDot.vue";
import { HEALTH_CHECKS, type StatusKind } from "@/lib/healthChecks";
import type { Machine, CheckOutcome } from "@/services/tauri";

const props = defineProps<{
  machines: Machine[];
  rowsByMachine: Record<number, Record<string, CheckOutcome>>;
  selectedMachineId: number | null;
  selectedCheckId: string | null;
}>();
const emit = defineEmits<{ select: [{ machineId: number; checkId: string }] }>();

function cellStatus(machineId: number, checkId: string): StatusKind {
  return (props.rowsByMachine[machineId]?.[checkId]?.status as StatusKind) ?? "unknown";
}
</script>

<template>
  <div class="overflow-auto">
    <table data-health-matrix class="min-w-[920px] border-collapse text-xs">
      <thead class="sticky top-0 bg-card">
        <tr>
          <th class="sticky left-0 z-10 min-w-[180px] border-b bg-card px-3 py-2 text-left">Machine</th>
          <th
            v-for="c in HEALTH_CHECKS" :key="c.id"
            class="border-b px-2 py-2 text-center align-bottom"
            :class="c.emphasized ? 'border-b-2 border-primary' : ''"
          >
            <span class="font-mono uppercase">{{ c.shortLabel }}</span>
          </th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="m in machines" :key="m.id ?? m.ip">
          <td class="sticky left-0 z-10 min-w-[180px] border-b bg-card px-3 py-2">
            <div class="flex items-center gap-2">
              <UecmStatusDot :tone="(m.status as StatusKind)" />
              <div>
                <div class="font-mono text-[12px] font-medium">{{ m.hostname }}</div>
                <div class="font-mono text-[10px] text-muted-foreground">{{ m.ip }}</div>
              </div>
            </div>
          </td>
          <td v-for="c in HEALTH_CHECKS" :key="c.id" class="border-b px-1 py-1 text-center">
            <button data-matrix-cell @click="m.id != null && emit('select', { machineId: m.id, checkId: c.id })">
              <UecmMatrixCell :tone="cellStatus(m.id ?? -1, c.id)" />
            </button>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
