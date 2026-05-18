<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import DeployDdcWizard from "@/components/modals/DeployDdcWizard.vue";

const { t } = useI18n();
const open = ref(false);
</script>

<template>
  <div class="p-6">
    <h1 class="font-display text-2xl mb-3">{{ t("deploy.pageTitle") }}</h1>
    <p class="text-muted-foreground mb-4">{{ t("deploy.pageDesc") }}</p>
    <button class="px-4 py-2 rounded bg-primary text-primary-foreground" @click="open = true">
      {{ t("deploy.openWizard") }}
    </button>
    <DeployDdcWizard :open="open" @close="open = false" />
  </div>
</template>
