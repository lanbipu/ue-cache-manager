<script setup lang="ts">
import { computed } from "vue";
import { DropdownMenuLabel, type DropdownMenuLabelProps, useForwardProps } from "reka-ui";
import { cn } from "@/lib/utils";

const props = defineProps<DropdownMenuLabelProps & { class?: string }>();
const delegated = computed(() => {
  const { class: _, ...rest } = props;
  return rest;
});
const forwarded = useForwardProps(delegated);
</script>

<template>
  <DropdownMenuLabel v-bind="forwarded" :class="cn('px-2 py-1.5 text-xs font-semibold uppercase tracking-wide text-muted-foreground', props.class)">
    <slot />
  </DropdownMenuLabel>
</template>
