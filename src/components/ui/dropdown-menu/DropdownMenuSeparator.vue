<script setup lang="ts">
import { computed } from "vue";
import { DropdownMenuSeparator, type DropdownMenuSeparatorProps, useForwardProps } from "reka-ui";
import { cn } from "@/lib/utils";

const props = defineProps<DropdownMenuSeparatorProps & { class?: string }>();
const delegated = computed(() => {
  const { class: _, ...rest } = props;
  return rest;
});
const forwarded = useForwardProps(delegated);
</script>

<template>
  <DropdownMenuSeparator v-bind="forwarded" :class="cn('-mx-1 my-1 h-px bg-border', props.class)" />
</template>
