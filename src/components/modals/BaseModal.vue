<script setup lang="ts">
withDefaults(defineProps<{
  open: boolean;
  title: string;
  size?: "sm" | "md" | "lg" | "xl";
}>(), {
  size: "md",
});

const emit = defineEmits<{
  (e: "close"): void;
}>();

function onBackdrop() {
  emit("close");
}

function onCloseClick() {
  emit("close");
}

function stopBubble(e: Event) {
  e.stopPropagation();
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      data-modal
      class="fixed inset-0 z-50 flex items-center justify-center"
    >
      <div
        data-modal-backdrop
        class="absolute inset-0 bg-black/60 backdrop-blur-sm"
        @click="onBackdrop"
      ></div>
      <div
        class="relative max-h-[85vh] max-w-[calc(100vw-2rem)] overflow-hidden rounded-lg border bg-popover text-popover-foreground shadow-2xl"
        :class="{
          'w-[420px]': size === 'sm',
          'w-[520px]': size === 'md',
          'w-[720px]': size === 'lg',
          'w-[920px]': size === 'xl',
        }"
        @click="stopBubble"
      >
        <header class="flex items-center justify-between border-b px-4 py-3">
          <h2 class="font-display font-extrabold">{{ title }}</h2>
          <button
            data-modal-close
            class="rounded-md px-2 py-1 text-lg leading-none text-muted-foreground hover:bg-accent hover:text-accent-foreground"
            @click="onCloseClick"
          >
            ×
          </button>
        </header>
        <div class="max-h-[calc(85vh-7rem)] overflow-auto p-4">
          <slot></slot>
        </div>
        <footer v-if="$slots.footer" class="flex justify-end gap-2 border-t px-4 py-3">
          <slot name="footer"></slot>
        </footer>
      </div>
    </div>
  </Teleport>
</template>
