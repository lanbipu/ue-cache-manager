<script setup lang="ts">
import { useI18n } from "vue-i18n";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import UecmThemeToggle from "@/components/primitives/UecmThemeToggle.vue";
import UecmLanguageToggle from "@/components/primitives/UecmLanguageToggle.vue";
import Button from "@/components/ui/Button.vue";
import Input from "@/components/ui/Input.vue";
import { useTasksStore } from "@/stores/tasks";
import { useClusterStore } from "@/stores/cluster";

defineProps<{ logOpen: boolean }>();
const emit = defineEmits<{ toggleLog: [] }>();
const tasks = useTasksStore();
const cluster = useClusterStore();
const { t } = useI18n();
</script>

<template>
  <header class="flex h-14 shrink-0 items-center justify-between gap-3 border-b bg-background px-4">
    <div class="relative w-full max-w-md">
      <UecmIcon name="search" size="16" class="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
      <Input class="pl-9" :placeholder="t('shell.searchPlaceholder')" />
    </div>

    <div class="flex items-center gap-2">
      <div v-if="tasks.activeTask" class="hidden items-center gap-2 rounded-full border bg-card px-3 py-1.5 text-xs md:flex">
        <span class="font-bold">{{ tasks.activeTask.label }}</span>
        <span class="text-muted-foreground">{{ tasks.activeTask.progress }}%</span>
      </div>
      <div class="hidden rounded-full border bg-card px-3 py-1.5 text-xs text-muted-foreground lg:block">
        {{ cluster.summary }}
      </div>
      <UecmLanguageToggle />
      <UecmThemeToggle />
      <Button
        variant="ghost"
        size="icon-sm"
        :aria-pressed="logOpen"
        :aria-label="t('shell.activityLog')"
        @click="emit('toggleLog')"
      >
        <UecmIcon :name="logOpen ? 'panel-bottom-close' : 'terminal'" />
      </Button>
    </div>
  </header>
</template>
