<script setup lang="ts">
import { computed } from "vue";

const rawIcons = import.meta.glob("@/assets/icons/*.svg", { eager: true, query: "?raw", import: "default" }) as Record<string, string>;

const iconMap: Record<string, string> = Object.fromEntries(
  Object.entries(rawIcons).map(([path, raw]) => [
    path.split("/").pop()!.replace(/\.svg$/, ""),
    raw,
  ])
);

const props = withDefaults(defineProps<{
  name: string;
  size?: number | string;
  stroke?: number | string;
}>(), {
  size: 16,
  stroke: 1.5,
});

const svg = computed(() => {
  const raw = iconMap[props.name];
  if (!raw) {
    if (import.meta.env.DEV) console.warn(`[UecmIcon] unknown icon "${props.name}"`);
    return "";
  }
  return raw
    .replace(/\swidth="[^"]*"/, ' width="100%"')
    .replace(/\sheight="[^"]*"/, ' height="100%"')
    .replace(/stroke-width="[^"]*"/, `stroke-width="${props.stroke}"`);
});

const dim = computed(() => {
  if (typeof props.size === "number") return `${props.size}px`;
  return /^\d+(\.\d+)?$/.test(props.size) ? `${props.size}px` : props.size;
});
</script>

<template>
  <span
    class="inline-flex items-center justify-center align-middle shrink-0"
    :style="{ width: dim, height: dim }"
    v-html="svg"
  />
</template>

<style scoped>
:deep(svg) {
  width: 100%;
  height: 100%;
  display: block;
  flex-shrink: 0;
}
</style>
