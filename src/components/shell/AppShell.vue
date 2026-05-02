<script setup lang="ts">
import { ref } from "vue";
import { RouterView } from "vue-router";
import UecmSidebar from "./UecmSidebar.vue";
import UecmTopBar from "./UecmTopBar.vue";
import UecmLogPanel from "./UecmLogPanel.vue";

const logOpen = ref(false);
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
  </div>
</template>
