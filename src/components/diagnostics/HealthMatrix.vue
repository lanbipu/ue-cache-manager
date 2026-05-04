<script setup lang="ts">
import UecmMatrixCell from "@/components/primitives/UecmMatrixCell.vue";
import { HEALTH_CHECKS, type StatusKind } from "@/lib/healthChecks";
import type { CheckOutcome, Machine } from "@/services/tauri";

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
    <table data-health-matrix class="min-w-[980px] border-collapse text-xs">
      <thead class="sticky top-0 bg-card">
        <tr>
          <th class="sticky left-0 z-10 min-w-[180px] border-b bg-card px-3 py-2 text-left">Machine</th>
          <th v-for="check in HEALTH_CHECKS" :key="check.id" class="border-b px-2 py-2 text-center" :class="'emphasized' in check && check.emphasized ? 'border-b-2 border-primary' : ''">
            <span class="font-mono uppercase">{{ check.shortLabel }}</span>
          </th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="machine in machines" :key="machine.id ?? machine.ip">
          <td class="sticky left-0 z-10 min-w-[180px] border-b bg-card px-3 py-2">
            <div class="font-mono text-[12px] font-bold">{{ machine.hostname }}</div>
            <div class="font-mono text-[10px] text-muted-foreground">{{ machine.ip }}</div>
          </td>
          <td v-for="check in HEALTH_CHECKS" :key="check.id" class="border-b px-1 py-1 text-center">
            <button data-matrix-cell @click="machine.id != null && emit('select', { machineId: machine.id, checkId: check.id })">
              <UecmMatrixCell :tone="cellStatus(machine.id ?? -1, check.id)" />
            </button>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
