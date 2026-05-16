<script setup lang="ts">
import { DialogClose, DialogContent, DialogOverlay, DialogPortal, DialogRoot, DialogTitle } from "reka-ui";
import { useI18n } from "vue-i18n";
import UecmIcon from "@/components/primitives/UecmIcon.vue";

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

const { t } = useI18n();

function onOpenChange(next: boolean) {
  if (!next) emit("close");
}
</script>

<template>
  <DialogRoot :open="open" @update:open="onOpenChange">
    <DialogPortal>
      <DialogOverlay
        data-modal-backdrop
        class="fixed inset-0 z-50 bg-foreground/60 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0"
        @click="emit('close')"
      />
      <DialogContent
        data-modal
        aria-describedby=""
        class="fixed left-1/2 top-1/2 z-50 flex max-h-[85vh] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-lg border bg-popover text-popover-foreground shadow-2xl outline-none data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95"
        :class="{
          'w-[min(420px,calc(100vw-2rem))]': size === 'sm',
          'w-[min(520px,calc(100vw-2rem))]': size === 'md',
          'w-[min(720px,calc(100vw-2rem))]': size === 'lg',
          'w-[min(920px,calc(100vw-2rem))]': size === 'xl',
        }"
      >
        <header class="flex shrink-0 items-center justify-between border-b px-4 py-3">
          <DialogTitle class="font-display font-extrabold">{{ title }}</DialogTitle>
          <DialogClose
            data-modal-close
            :aria-label="t('common.close')"
            class="rounded-md p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <UecmIcon name="x" size="18" />
          </DialogClose>
        </header>
        <div class="flex-1 overflow-auto p-4">
          <slot></slot>
        </div>
        <footer v-if="$slots.footer" class="flex shrink-0 justify-end gap-2 border-t px-4 py-3">
          <slot name="footer"></slot>
        </footer>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
