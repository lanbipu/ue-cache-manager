<script setup lang="ts">
import { computed } from "vue";
import UecmIcon from "./UecmIcon.vue";
import UecmProgressBar from "./UecmProgressBar.vue";
import UecmStatusBadge from "./UecmStatusBadge.vue";
import type { UecmTone } from "./types";

const props = withDefaults(
  defineProps<{
    title: string;
    subtitle?: string | null;
    status: "queued" | "spawning" | "running" | "verifying" | "completed" | "ok" | "err" | "cancelled" | "error" | "verify_failed";
    progress?: number | null;
    progressLabel?: string | null;
    cancellable?: boolean;
  }>(),
  {
    subtitle: null,
    progress: null,
    progressLabel: null,
    cancellable: false,
  },
);

const emit = defineEmits<{
  (e: "cancel"): void;
}>();

const tone = computed<UecmTone>(() => {
  if (["completed", "ok"].includes(props.status)) return "healthy";
  if (["err", "error", "verify_failed"].includes(props.status)) return "critical";
  if (props.status === "cancelled") return "offline";
  if (props.status === "verifying") return "warning";
  return "progress";
});

const label = computed(() => props.status.replace("_", " "));
</script>

<template>
  <article data-task-card class="rounded-lg border bg-card p-4">
    <div class="flex items-start justify-between gap-3">
      <div class="min-w-0">
        <h3 class="truncate font-display text-sm font-extrabold">{{ title }}</h3>
        <p v-if="subtitle" class="mt-1 truncate text-xs text-muted-foreground">{{ subtitle }}</p>
      </div>
      <div class="flex shrink-0 items-center gap-2">
        <UecmStatusBadge :tone="tone" :label="label" size="sm" />
        <button
          v-if="cancellable"
          data-task-cancel
          class="inline-flex size-8 items-center justify-center rounded-md border text-muted-foreground hover:bg-accent hover:text-accent-foreground"
          title="Cancel"
          @click="emit('cancel')"
        >
          <UecmIcon name="x" size="15" />
        </button>
      </div>
    </div>
    <UecmProgressBar
      v-if="['spawning', 'running', 'verifying'].includes(status)"
      class="mt-3"
      :value="progress"
      :label="progressLabel"
      :indeterminate="progress == null"
    />
  </article>
</template>
