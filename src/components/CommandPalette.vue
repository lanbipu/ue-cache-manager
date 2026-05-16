<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { useEventListener } from "@vueuse/core";
import BaseModal from "@/components/modals/BaseModal.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import { useMachinesStore } from "@/stores/machines";

type Group = "machine" | "tab" | "action";

interface Item {
  id: string;
  group: Group;
  label: string;
  hint?: string;
  icon: string;
  action: () => void;
}

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const store = useMachinesStore();

const emit = defineEmits<{
  (e: "request-scan"): void;
  (e: "request-create-share"): void;
  (e: "request-batch-env"): void;
  (e: "request-batch-ini"): void;
}>();

const open = ref(false);
const query = ref("");
const selectedIndex = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);

const TABS = ["overview", "projects", "ddc", "pso", "ini", "health"] as const;

const items = computed<Item[]>(() => {
  const machineItems: Item[] = store.machines.map((m) => ({
    id: `machine-${m.id}`,
    group: "machine",
    label: m.hostname,
    hint: m.ip,
    icon: "server",
    action: () => {
      router.push({
        path: "/machines",
        query: {
          ...route.query,
          host: String(m.id),
          tab: (route.query.tab as string) ?? "overview",
        },
      });
    },
  }));

  const tabItems: Item[] = TABS.map((key) => ({
    id: `tab-${key}`,
    group: "tab",
    label: t(`machineTabs.${key}`),
    icon: "layout-dashboard",
    action: () => {
      router.push({ path: "/machines", query: { ...route.query, tab: key } });
    },
  }));

  const actionItems: Item[] = [
    {
      id: "action-scan",
      group: "action",
      label: t("commandPalette.actionScan"),
      icon: "radar",
      action: () => emit("request-scan"),
    },
    {
      id: "action-create-share",
      group: "action",
      label: t("commandPalette.actionCreateShare"),
      icon: "plus",
      action: () => emit("request-create-share"),
    },
    {
      id: "action-batch-env",
      group: "action",
      label: t("commandPalette.actionBatchEnv"),
      icon: "terminal",
      action: () => emit("request-batch-env"),
    },
    {
      id: "action-batch-ini",
      group: "action",
      label: t("commandPalette.actionBatchIni"),
      icon: "file-text",
      action: () => emit("request-batch-ini"),
    },
  ];

  return [...machineItems, ...tabItems, ...actionItems];
});

const filteredItems = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return items.value;
  return items.value.filter(
    (item) =>
      item.label.toLowerCase().includes(q) || item.hint?.toLowerCase().includes(q),
  );
});

useEventListener("keydown", (e: KeyboardEvent) => {
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
    e.preventDefault();
    open.value = !open.value;
  }
});

watch(open, async (v) => {
  if (v) {
    query.value = "";
    selectedIndex.value = 0;
    await nextTick();
    inputRef.value?.focus();
  }
});

watch(filteredItems, () => {
  selectedIndex.value = 0;
});

function onPanelKey(e: KeyboardEvent) {
  if (!open.value) return;
  const n = filteredItems.value.length;
  if (e.key === "ArrowDown") {
    e.preventDefault();
    selectedIndex.value = n === 0 ? 0 : (selectedIndex.value + 1) % n;
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    selectedIndex.value = n === 0 ? 0 : (selectedIndex.value - 1 + n) % n;
  } else if (e.key === "Enter") {
    e.preventDefault();
    runItem(filteredItems.value[selectedIndex.value]);
  }
}

function runItem(item: Item | undefined) {
  if (!item) return;
  item.action();
  open.value = false;
}

function groupLabel(group: Group): string {
  if (group === "machine") return t("commandPalette.groupMachine");
  if (group === "tab") return t("commandPalette.groupTab");
  return t("commandPalette.groupAction");
}

const groupedView = computed(() => {
  const groups: { group: Group; items: Item[] }[] = [];
  for (const item of filteredItems.value) {
    const last = groups[groups.length - 1];
    if (last && last.group === item.group) {
      last.items.push(item);
    } else {
      groups.push({ group: item.group, items: [item] });
    }
  }
  return groups;
});
</script>

<template>
  <BaseModal :open="open" :title="t('commandPalette.title')" size="lg" @close="open = false">
    <div data-command-palette class="space-y-3" @keydown="onPanelKey">
      <input
        ref="inputRef"
        v-model="query"
        type="text"
        :placeholder="t('commandPalette.placeholder')"
        class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      />
      <div class="max-h-80 overflow-auto rounded-md border">
        <p v-if="filteredItems.length === 0" class="p-4 text-center text-sm text-muted-foreground">
          {{ t("commandPalette.empty") }}
        </p>
        <template v-else>
          <div v-for="group in groupedView" :key="group.group" class="border-b last:border-b-0">
            <div class="px-3 py-1.5 text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
              {{ groupLabel(group.group) }}
            </div>
            <ul>
              <li
                v-for="item in group.items"
                :key="item.id"
                :data-cmd-item="item.id"
                :data-selected="filteredItems[selectedIndex]?.id === item.id"
                class="flex cursor-pointer items-center gap-3 px-3 py-2 text-sm"
                :class="filteredItems[selectedIndex]?.id === item.id ? 'bg-accent text-accent-foreground' : ''"
                @click="runItem(item)"
                @mouseover="selectedIndex = filteredItems.findIndex((i) => i.id === item.id)"
              >
                <UecmIcon :name="item.icon" size="16" class="text-muted-foreground" />
                <span class="font-medium">{{ item.label }}</span>
                <span v-if="item.hint" class="ml-auto font-mono text-xs text-muted-foreground">
                  {{ item.hint }}
                </span>
              </li>
            </ul>
          </div>
        </template>
      </div>
    </div>
  </BaseModal>
</template>
