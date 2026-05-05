<script setup lang="ts">
import { useI18n } from "vue-i18n";
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";

const { t } = useI18n();
</script>

<template>
  <div class="space-y-6 p-6">
    <UecmPageHeader :title="t('projects.title')" :eyebrow="t('projects.eyebrow')" :description="t('projects.description')" />
    <section class="rounded-lg border bg-card p-8 text-center">
      <UecmIcon name="folder-git-2" size="32" class="mx-auto text-muted-foreground" />
      <h2 class="mt-4 font-display text-lg font-extrabold">{{ t("projects.emptyTitle") }}</h2>
      <p class="mx-auto mt-2 max-w-xl text-sm text-muted-foreground">
        {{ t("projects.emptyHint") }}
      </p>
    </section>
  </div>
</template>
