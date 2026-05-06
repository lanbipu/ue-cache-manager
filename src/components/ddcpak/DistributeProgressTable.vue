<script setup lang="ts">
import UecmStatusBadge from "@/components/primitives/UecmStatusBadge.vue";
import type { DistributeJobState } from "@/stores/ddcPak";

defineProps<{
  job: DistributeJobState;
}>();

const emit = defineEmits<{
  (e: "retry", targetMachineId: number): void;
}>();

function tone(status: string) {
  if (status === "ok") return "healthy";
  if (status === "err") return "critical";
  if (status === "running") return "progress";
  return "unknown";
}
</script>

<template>
  <div data-distribute-progress-table class="overflow-hidden rounded-lg border">
    <table class="w-full text-left text-sm">
      <thead class="bg-muted text-xs uppercase text-muted-foreground">
        <tr>
          <th class="px-3 py-2">Target</th>
          <th class="px-3 py-2">Host</th>
          <th class="px-3 py-2">Status</th>
          <th class="px-3 py-2">Message</th>
          <th class="px-3 py-2 text-right">Action</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="target in job.targets" :key="target.target_machine_id" class="border-t">
          <td class="px-3 py-2 font-bold">{{ target.target_machine_id }}</td>
          <td class="px-3 py-2">{{ target.target_host }}</td>
          <td class="px-3 py-2">
            <UecmStatusBadge :tone="tone(target.status)" :label="target.status" size="sm" />
          </td>
          <td class="px-3 py-2 text-muted-foreground">{{ target.message ?? "" }}</td>
          <td class="px-3 py-2 text-right">
            <button
              v-if="target.status === 'err'"
              class="rounded-md border px-2 py-1 text-xs font-bold hover:bg-accent"
              @click="emit('retry', target.target_machine_id)"
            >
              Retry
            </button>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
