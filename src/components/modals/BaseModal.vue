<script setup lang="ts">
defineProps<{
  open: boolean;
  title: string;
}>();

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
        class="absolute inset-0 bg-black/40"
        @click="onBackdrop"
      ></div>
      <div
        class="relative bg-white rounded shadow-lg w-[480px] max-w-full"
        @click="stopBubble"
      >
        <header class="flex items-center justify-between border-b px-4 py-3">
          <h2 class="font-medium">{{ title }}</h2>
          <button
            data-modal-close
            class="text-gray-500 hover:text-gray-900 text-lg leading-none"
            @click="onCloseClick"
          >
            ×
          </button>
        </header>
        <div class="p-4">
          <slot></slot>
        </div>
        <footer v-if="$slots.footer" class="flex justify-end gap-2 border-t px-4 py-3">
          <slot name="footer"></slot>
        </footer>
      </div>
    </div>
  </Teleport>
</template>
