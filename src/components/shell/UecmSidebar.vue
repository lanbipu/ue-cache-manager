<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import { useI18n } from "vue-i18n";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import UecmStatusDot from "@/components/primitives/UecmStatusDot.vue";
import { useClusterStore } from "@/stores/cluster";

const route = useRoute();
const cluster = useClusterStore();
const { t } = useI18n();

const navItems = computed(() => [
  { to: "/machines", label: t("nav.machines"), icon: "server" },
  { to: "/deploy", label: t("nav.deploy"), icon: "send" },
]);

const activePath = computed(() => route.path);

function isItemActive(itemPath: string) {
  if (itemPath === "/machines") {
    return activePath.value === "/" || activePath.value.startsWith("/machines");
  }
  return activePath.value === itemPath;
}
</script>

<template>
  <aside class="flex h-full w-60 shrink-0 flex-col border-r border-sidebar-border bg-sidebar text-sidebar-foreground">
    <div class="flex h-16 items-center gap-3 border-b border-sidebar-border px-4">
      <img src="@/assets/uecm-mark.svg" alt="" class="size-8" />
      <div class="min-w-0">
        <div class="font-display text-sm font-extrabold">{{ t("shell.appName") }}</div>
        <div class="text-xs text-muted-foreground">{{ cluster.summary }}</div>
      </div>
    </div>

    <nav class="flex-1 space-y-1 overflow-y-auto p-3">
      <RouterLink
        v-for="item in navItems"
        :key="item.to"
        data-nav-item
        :to="item.to"
        class="flex h-10 items-center gap-3 rounded-md px-3 text-sm font-medium text-muted-foreground transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
        :class="isItemActive(item.to) ? 'bg-sidebar-accent text-sidebar-accent-foreground' : ''"
      >
        <UecmIcon :name="item.icon" size="17" />
        <span>{{ item.label }}</span>
      </RouterLink>
    </nav>

    <div class="border-t border-sidebar-border p-4">
      <div class="rounded-lg border border-sidebar-border bg-background/40 p-3">
        <div class="flex items-center justify-between">
          <span class="text-xs font-bold uppercase tracking-wide text-muted-foreground">{{ t("shell.clusterScore") }}</span>
          <UecmStatusDot :tone="cluster.critical ? 'critical' : cluster.warning ? 'warning' : 'healthy'" />
        </div>
        <div class="mt-2 font-display text-2xl font-extrabold">{{ cluster.score }}</div>
        <div class="mt-1 text-xs text-muted-foreground">
          {{ t("shell.criticalWarningSummary", { critical: cluster.critical, warning: cluster.warning }) }}
        </div>
      </div>
    </div>
  </aside>
</template>
