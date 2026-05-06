<script setup lang="ts">
import { computed } from "vue";
import UecmTaskCard from "@/components/primitives/UecmTaskCard.vue";
import type { CollectJobState } from "@/stores/pso";

const props = defineProps<{
  job: CollectJobState;
  sourceLabel: string;
  projectLabel: string;
}>();

const emit = defineEmits<{
  (e: "cancel", jobId: string): void;
}>();

const taskStatus = computed(() => {
  if (props.job.status === "queued" || props.job.status === "spawning") return "spawning";
  if (props.job.status === "collecting" || props.job.status === "completing") return "running";
  return props.job.status;
});

const progressLabel = computed(() => {
  if (props.job.status === "spawning") return "Spawning UE";
  if (props.job.status === "collecting") return "Collecting PSOs";
  if (props.job.status === "completing") return "Finalising collected files";
  return null;
});

const subtitle = computed(() => `${props.projectLabel} on ${props.sourceLabel}`);
</script>

<template>
  <div data-pso-job-card>
    <UecmTaskCard
      :title="`PSO Collect · ${job.job_id}`"
      :subtitle="subtitle"
      :status="taskStatus"
      :progress="null"
      :progress-label="progressLabel"
      :cancellable="['spawning', 'collecting'].includes(job.status)"
      @cancel="emit('cancel', job.job_id)"
    />
    <pre
      v-if="job.log_lines.length"
      class="mt-2 max-h-32 overflow-auto rounded-md border bg-muted p-3 text-xs"
    >{{ job.log_lines.slice(-8).join("\n") }}</pre>
    <p v-if="job.files_collected != null" data-pso-job-files class="mt-2 text-xs text-muted-foreground">
      {{ job.files_collected }} file(s) collected
    </p>
  </div>
</template>
