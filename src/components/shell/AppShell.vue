<script setup lang="ts">
import { RouterView } from "vue-router";
import ActivityBar from "./ActivityBar.vue";
</script>

<template>
  <div class="flex h-full w-full">
    <ActivityBar />
    <main class="flex-1 overflow-auto">
      <RouterView />
    </main>
  </div>
</template>
