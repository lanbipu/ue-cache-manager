<script setup lang="ts">
defineProps<{
  variant: "empty" | "loading" | "error";
  title?: string;
  message?: string;
  retryLabel?: string;
}>();

const emit = defineEmits<{
  (e: "retry"): void;
}>();
</script>

<template>
  <div
    :data-state-block="variant"
    class="rounded-lg border p-6 text-sm"
    :class="{
      'bg-card text-muted-foreground': variant !== 'error',
      'border-destructive/30 bg-destructive/10 text-destructive': variant === 'error',
    }"
  >
    <p v-if="title" class="font-display text-sm font-extrabold text-foreground">{{ title }}</p>
    <p v-if="message" class="mt-1 text-sm">{{ message }}</p>
    <button
      v-if="retryLabel"
      data-state-block-retry
      class="mt-3 rounded-md border px-3 py-1 text-xs font-bold hover:bg-accent"
      @click="emit('retry')"
    >
      {{ retryLabel }}
    </button>
  </div>
</template>
