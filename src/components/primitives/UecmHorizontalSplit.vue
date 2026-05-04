<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    leftWeight?: number;
    rightWeight?: number;
  }>(),
  {
    leftWeight: 1,
    rightWeight: 2,
  },
);
</script>

<template>
  <div
    data-horizontal-split
    class="grid h-full min-h-0"
    :style="{ gridTemplateColumns: `${props.leftWeight}fr ${props.rightWeight}fr` }"
  >
    <div class="min-h-0 overflow-y-auto border-r">
      <slot name="left" />
    </div>
    <div class="min-h-0 overflow-y-auto">
      <slot name="right" />
    </div>
  </div>
</template>
