<script setup lang="ts">
import UecmTaskCard from "@/components/primitives/UecmTaskCard.vue";
import type { GenerateJobState } from "@/stores/ddcPak";

defineProps<{
  job: GenerateJobState;
}>();

const emit = defineEmits<{
  (e: "cancel", jobId: string): void;
}>();
</script>

<template>
  <UecmTaskCard
    data-pak-job-card
    :title="`Generate · ${job.job_id}`"
    :subtitle="job.output?.path ?? `Project ${job.project_id}`"
    :status="job.status"
    :progress="job.progress_pct"
    :progress-label="job.progress_label"
    :cancellable="['spawning', 'running', 'verifying'].includes(job.status)"
    @cancel="emit('cancel', job.job_id)"
  />
  <pre v-if="job.log_lines.length" class="mt-2 max-h-32 overflow-auto rounded-md border bg-muted p-3 text-xs">{{ job.log_lines.slice(-8).join("\n") }}</pre>
</template>
