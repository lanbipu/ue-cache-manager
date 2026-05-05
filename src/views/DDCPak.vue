<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";

const { t } = useI18n();

const stepKeys = [
  "selectProject",
  "resolveEngine",
  "validateDdc",
  "collectArtifacts",
  "compressPak",
  "distribute",
  "verifyClients",
  "archiveReport",
] as const;

const steps = computed(() => stepKeys.map((k) => t(`ddcPak.steps.${k}`)));
</script>

<template>
  <div class="space-y-6 p-6">
    <UecmPageHeader :title="t('ddcPak.title')" :eyebrow="t('ddcPak.eyebrow')" :description="t('ddcPak.description')" />
    <section class="rounded-lg border bg-card p-4">
      <div class="grid gap-3 md:grid-cols-4">
        <div v-for="(step, index) in steps" :key="step" class="rounded-md border bg-background p-3">
          <div class="flex items-center gap-2 text-sm font-bold">
            <span class="inline-flex size-6 items-center justify-center rounded-full bg-muted text-xs text-muted-foreground">{{ index + 1 }}</span>
            {{ step }}
          </div>
          <p class="mt-2 text-xs text-muted-foreground">{{ t("ddcPak.waitingForProject") }}</p>
        </div>
      </div>
    </section>
    <section class="rounded-lg border bg-card p-8 text-center">
      <UecmIcon name="package" size="32" class="mx-auto text-muted-foreground" />
      <h2 class="mt-4 font-display text-lg font-extrabold">{{ t("ddcPak.emptyTitle") }}</h2>
      <p class="mx-auto mt-2 max-w-xl text-sm text-muted-foreground">
        {{ t("ddcPak.emptyHint") }}
      </p>
    </section>
  </div>
</template>
