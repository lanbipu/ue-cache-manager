<script setup lang="ts">
import { computed } from "vue";
import type { GpuMatrix, GpuSignature, MachineGpuCell } from "@/services/tauri";

const props = defineProps<{ matrix: GpuMatrix | null }>();

const rows = computed(() => props.matrix?.signatures ?? []);
const cells = computed(() => props.matrix?.cells ?? []);

function signatureKey(signature: GpuSignature) {
  return `${signature.vendor}:${signature.model}:${signature.driver}`;
}

function cellMark(cell: MachineGpuCell, signature: GpuSignature) {
  if (!cell.signature) return "-";
  return signatureKey(cell.signature) === signatureKey(signature) ? "OK" : "";
}

function isBaseline(signature: GpuSignature) {
  return props.matrix?.baseline
    ? signatureKey(props.matrix.baseline) === signatureKey(signature)
    : false;
}
</script>

<template>
  <div data-gpu-matrix class="overflow-x-auto rounded-lg border bg-card">
    <table class="min-w-full text-xs">
      <thead class="bg-muted text-muted-foreground">
        <tr>
          <th class="min-w-[220px] px-3 py-2 text-left">GPU + Driver</th>
          <th class="px-3 py-2 text-left">Count</th>
          <th
            v-for="cell in cells"
            :key="cell.machine_id"
            class="px-2 py-2 text-center"
            data-gpu-matrix-machine-col
          >
            <span class="font-mono">{{ cell.hostname }}</span>
          </th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="row in rows"
          :key="signatureKey(row.signature)"
          class="border-t"
          :class="isBaseline(row.signature) ? 'bg-status-healthy/10' : ''"
          data-gpu-matrix-row
        >
          <td class="px-3 py-2 font-mono">
            {{ row.signature.vendor }} / {{ row.signature.model }} / {{ row.signature.driver }}
          </td>
          <td class="px-3 py-2 font-mono">{{ row.count }}</td>
          <td
            v-for="cell in cells"
            :key="`${signatureKey(row.signature)}-${cell.machine_id}`"
            class="px-2 py-2 text-center font-mono"
            data-gpu-matrix-cell
          >
            {{ cellMark(cell, row.signature) }}
          </td>
        </tr>
      </tbody>
    </table>
    <p v-if="rows.length === 0" data-gpu-matrix-empty class="p-4 text-sm text-muted-foreground">
      No GPU data yet. Refresh machine details to detect GPUs.
    </p>
  </div>
</template>
