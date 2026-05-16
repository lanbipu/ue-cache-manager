<script setup lang="ts">
import { computed, ref } from "vue";
import { RouterView } from "vue-router";
import UecmSidebar from "./UecmSidebar.vue";
import UecmTopBar from "./UecmTopBar.vue";
import UecmLogPanel from "./UecmLogPanel.vue";
import { useClusterStore } from "@/stores/cluster";

const logOpen = ref(false);
const cluster = useClusterStore();

const liveAnnouncement = computed(() => {
  if (cluster.critical > 0) {
    return `${cluster.critical} critical, ${cluster.warning} warning`;
  }
  if (cluster.warning > 0) {
    return `${cluster.warning} warning`;
  }
  return "";
});
</script>

<template>
  <div class="flex h-full w-full overflow-hidden bg-background text-foreground">
    <UecmSidebar />
    <div class="flex min-w-0 flex-1 flex-col overflow-hidden">
      <UecmTopBar :log-open="logOpen" @toggle-log="logOpen = !logOpen" />
      <main class="min-h-0 flex-1 overflow-auto">
        <RouterView />
      </main>
      <UecmLogPanel v-if="logOpen" @close="logOpen = false" />
    </div>
    <div
      data-a11y-live
      role="status"
      aria-live="polite"
      aria-atomic="true"
      class="sr-only"
    >
      {{ liveAnnouncement }}
    </div>
  </div>
</template>
