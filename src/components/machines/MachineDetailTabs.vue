<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import UecmStatusDot from "@/components/primitives/UecmStatusDot.vue";
import Button from "@/components/ui/Button.vue";
import HostnameEditor from "@/components/machines/HostnameEditor.vue";
import { useMachinesStore } from "@/stores/machines";
import { useCredentialsStore } from "@/stores/credentials";
import type { Machine } from "@/services/tauri";

import Projects from "@/views/Projects.vue";
import DDCPak from "@/views/DDCPak.vue";
import PSOCache from "@/views/PSOCache.vue";
import INIScanner from "@/views/INIScanner.vue";
import HealthCheck from "@/views/HealthCheck.vue";
import Shares from "@/views/Shares.vue";

type TabKey = "overview" | "projects" | "ddc" | "pso" | "ini" | "health";
type StatusTone = "healthy" | "warning" | "critical" | "offline" | "unknown";

const props = defineProps<{ machine: Machine }>();

const emit = defineEmits<{
  (e: "open-credential-modal"): void;
  (e: "open-env-var-modal"): void;
  (e: "open-ini-edit-modal"): void;
  (e: "open-share-wizard"): void;
  (e: "close"): void;
}>();

const route = useRoute();
const router = useRouter();
const { t } = useI18n();
const store = useMachinesStore();
const credentials = useCredentialsStore();

const bootstrapCredentialAlias = ref("");
const enableLocalAccountRemoteAdmin = ref(false);
const showDeleteConfirm = ref(false);

const TABS: { key: TabKey; icon: string }[] = [
  { key: "overview", icon: "layout-dashboard" },
  { key: "projects", icon: "folder-git-2" },
  { key: "ddc", icon: "package" },
  { key: "pso", icon: "database" },
  { key: "ini", icon: "file-search" },
  { key: "health", icon: "heart-pulse" },
];

const activeTab = computed<TabKey>(() => {
  const raw = route.query.tab;
  if (
    raw === "projects" ||
    raw === "ddc" ||
    raw === "pso" ||
    raw === "ini" ||
    raw === "health" ||
    raw === "overview"
  ) {
    return raw;
  }
  return "overview";
});

const detail = computed(() => {
  const d = store.selectedDetail;
  if (!d || d.machine.id !== props.machine.id) return null;
  return d;
});
const machineView = computed(() => detail.value?.machine ?? props.machine);
const winrmCredentials = computed(() =>
  credentials.credentials.filter((cred) => cred.kind === "winrm"),
);
const showBootstrapPanel = computed(
  () => machineView.value && machineView.value.status !== "online",
);

onMounted(() => {
  credentials.load();
});

watch(
  winrmCredentials,
  (items) => {
    if (!bootstrapCredentialAlias.value && items.length > 0) {
      bootstrapCredentialAlias.value = items[0].alias;
    }
  },
  { immediate: true },
);

function switchTo(tab: TabKey) {
  router.push({ path: "/machines", query: { ...route.query, tab } });
}

function statusTone(status: string): StatusTone {
  const s = status.toLowerCase();
  if (s === "online" || s === "healthy") return "healthy";
  if (["critical", "error", "failed"].includes(s)) return "critical";
  if (["warning", "warn", "degraded"].includes(s)) return "warning";
  if (s === "offline") return "offline";
  return "unknown";
}

function runBootstrap() {
  if (!bootstrapCredentialAlias.value) return;
  store.bootstrapSelected(bootstrapCredentialAlias.value, enableLocalAccountRemoteAdmin.value);
}

function onRename(value: string) {
  const id = machineView.value?.id;
  if (id != null) {
    store.renameMachine(id, value);
  }
}

async function confirmDelete() {
  const id = machineView.value?.id;
  if (id == null) return;
  await store.deleteMachine(id);
  showDeleteConfirm.value = false;
  emit("close");
}
</script>

<template>
  <nav
    data-machine-tabs
    role="tablist"
    class="flex shrink-0 items-center gap-1 overflow-x-auto border-b bg-card px-3 py-2"
  >
    <button
      v-for="tab in TABS"
      :key="tab.key"
      type="button"
      role="tab"
      :aria-selected="activeTab === tab.key"
      data-machine-tab
      :data-tab="tab.key"
      class="flex shrink-0 items-center gap-2 rounded-md px-3 py-1.5 text-sm font-medium transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      :class="activeTab === tab.key ? 'bg-accent text-accent-foreground' : 'text-muted-foreground'"
      @click="switchTo(tab.key)"
    >
      <UecmIcon :name="tab.icon" size="15" />
      {{ t(`machineTabs.${tab.key}`) }}
    </button>
  </nav>

  <div data-machine-tab-content class="flex-1 overflow-auto">
    <section v-if="activeTab === 'overview'" data-tab-content="overview" class="space-y-6 p-6">
      <header class="flex flex-wrap items-start justify-between gap-4">
        <div class="min-w-0">
          <div class="flex flex-wrap items-center gap-3">
            <h3 class="font-display text-lg font-extrabold">
              <HostnameEditor v-if="machineView" :value="machineView.hostname" @save="onRename" />
            </h3>
            <span class="flex items-center gap-1.5 rounded-full border bg-muted px-2 py-0.5 text-xs">
              <UecmStatusDot :tone="statusTone(machineView?.status ?? 'unknown')" />
              <span class="font-medium capitalize">{{ machineView?.status }}</span>
            </span>
          </div>
          <p class="mt-1 font-mono text-xs text-muted-foreground">{{ machineView?.ip }}</p>
        </div>
        <div class="flex flex-wrap gap-2">
          <Button
            variant="outline"
            size="sm"
            data-refresh-btn
            :disabled="store.isRefreshing"
            @click="store.refreshSelected()"
          >
            <UecmIcon name="refresh-cw" size="14" />
            {{ store.isRefreshing ? t("common.refreshing") : t("common.refresh") }}
          </Button>
          <Button variant="outline" size="sm" @click="emit('open-credential-modal')">
            <UecmIcon name="key-round" size="14" />
            {{ t("machineDetail.credentials") }}
          </Button>
          <Button variant="outline" size="sm" @click="emit('open-env-var-modal')">
            <UecmIcon name="terminal" size="14" />
            {{ t("machineDetail.envVars") }}
          </Button>
          <Button variant="outline" size="sm" @click="emit('open-ini-edit-modal')">
            <UecmIcon name="file-text" size="14" />
            {{ t("machineDetail.editIni") }}
          </Button>
          <Button
            variant="destructive"
            size="sm"
            data-machine-delete-btn
            @click="showDeleteConfirm = true"
          >
            <UecmIcon name="trash-2" size="14" />
            {{ t("common.delete") }}
          </Button>
        </div>
      </header>

      <div
        v-if="showDeleteConfirm"
        data-delete-confirm
        class="rounded-md border border-destructive/30 bg-destructive/10 p-4 text-sm"
      >
        <p class="mb-3">{{ t("machineDetail.confirmDelete", { hostname: machineView?.hostname }) }}</p>
        <div class="flex gap-2">
          <Button variant="destructive" size="sm" data-confirm-delete @click="confirmDelete">
            {{ t("common.delete") }}
          </Button>
          <Button variant="outline" size="sm" @click="showDeleteConfirm = false">
            {{ t("common.cancel") }}
          </Button>
        </div>
      </div>

      <div class="grid gap-4 md:grid-cols-2">
        <article class="rounded-lg border bg-card p-4">
          <h4 class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">
            {{ t("machineTabs.overviewIdentity") }}
          </h4>
          <dl class="mt-3 grid gap-2 text-sm">
            <div class="flex justify-between gap-3">
              <dt class="text-muted-foreground">{{ t("machineTabs.fieldHostname") }}</dt>
              <dd class="font-medium">{{ machineView?.hostname }}</dd>
            </div>
            <div class="flex justify-between gap-3">
              <dt class="text-muted-foreground">{{ t("machineTabs.fieldIp") }}</dt>
              <dd class="font-mono">{{ machineView?.ip }}</dd>
            </div>
            <div class="flex justify-between gap-3">
              <dt class="text-muted-foreground">{{ t("machineTabs.fieldRole") }}</dt>
              <dd>{{ machineView?.role || t("common.none") }}</dd>
            </div>
            <div class="flex justify-between gap-3">
              <dt class="text-muted-foreground">{{ t("machineTabs.lastSeenLabel") }}</dt>
              <dd class="font-mono text-xs">{{ machineView?.last_seen_at ?? t("common.none") }}</dd>
            </div>
          </dl>
        </article>

        <article class="rounded-lg border bg-card p-4">
          <h4 class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">
            {{ t("machineDetail.ueInstalls") }}
          </h4>
          <p
            v-if="!detail || detail.ue_installs.length === 0"
            class="mt-3 text-sm text-muted-foreground"
          >
            {{ store.isDetailLoading ? t("common.loading") : t("machineDetail.noUeInstalls") }}
          </p>
          <ul v-else class="mt-3 space-y-1.5 text-sm">
            <li
              v-for="install in detail.ue_installs"
              :key="install.id ?? install.version"
              class="flex justify-between gap-3"
            >
              <span class="font-medium">{{ install.version }}</span>
              <span class="truncate font-mono text-xs text-muted-foreground">{{ install.install_path }}</span>
            </li>
          </ul>
        </article>
      </div>

      <article class="rounded-lg border bg-card p-4">
        <h4 class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">
          {{ t("machineDetail.gpus") }}
        </h4>
        <p v-if="!detail || detail.gpus.length === 0" class="mt-3 text-sm text-muted-foreground">
          {{ store.isDetailLoading ? t("common.loading") : t("machineDetail.noGpus") }}
        </p>
        <table v-else class="mt-3 w-full text-sm">
          <thead class="text-xs text-muted-foreground">
            <tr>
              <th class="px-3 py-1 text-left font-medium">{{ t("machineDetail.headerModel") }}</th>
              <th class="px-3 py-1 text-left font-medium">{{ t("machineDetail.headerDriver") }}</th>
              <th class="px-3 py-1 text-left font-medium">{{ t("machineDetail.headerVram") }}</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="gpu in detail.gpus"
              :key="gpu.id ?? gpu.gpu_model"
              class="border-t border-border/60"
            >
              <td class="px-3 py-1.5">{{ gpu.gpu_model }}</td>
              <td class="px-3 py-1.5 font-mono text-xs">{{ gpu.driver_version }}</td>
              <td class="px-3 py-1.5 font-mono text-xs">{{ gpu.vram_mb ? gpu.vram_mb + " MB" : "—" }}</td>
            </tr>
          </tbody>
        </table>
      </article>

      <article
        v-if="showBootstrapPanel"
        data-bootstrap-panel
        class="rounded-lg border border-status-warning/40 bg-status-warning/5 p-4"
      >
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div class="min-w-0">
            <h4 class="font-display text-sm font-extrabold">
              {{ t("machineDetail.bootstrapTitle") }}
            </h4>
            <p class="mt-1 max-w-prose text-sm text-muted-foreground">
              {{ t("machineDetail.bootstrapHint") }}
            </p>
          </div>
          <Button
            variant="outline"
            size="sm"
            data-load-bootstrap-script
            :disabled="store.isLoadingBootstrapScript"
            @click="store.loadBootstrapScript()"
          >
            {{ store.isLoadingBootstrapScript ? t("common.loading") : t("machineDetail.showBootstrapScript") }}
          </Button>
        </div>
        <div class="mt-3 flex flex-wrap items-center gap-2">
          <select
            v-model="bootstrapCredentialAlias"
            data-bootstrap-cred
            class="h-9 min-w-64 rounded-md border border-input bg-transparent px-2 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <option value="">{{ t("machineDetail.selectWinrmCredential") }}</option>
            <option v-for="cred in winrmCredentials" :key="cred.alias" :value="cred.alias">
              {{ cred.alias }} — {{ cred.username }}
            </option>
          </select>
          <Button
            size="sm"
            data-bootstrap-btn
            :disabled="store.isBootstrapping || !bootstrapCredentialAlias"
            @click="runBootstrap"
          >
            {{ store.isBootstrapping ? t("machineDetail.bootstrapping") : t("machineDetail.runBootstrap") }}
          </Button>
          <Button
            v-if="winrmCredentials.length === 0"
            variant="outline"
            size="sm"
            @click="emit('open-credential-modal')"
          >
            {{ t("machineDetail.addCredential") }}
          </Button>
        </div>
        <label class="mt-2 flex items-center gap-2 text-xs text-muted-foreground">
          <input v-model="enableLocalAccountRemoteAdmin" data-bootstrap-local-admin type="checkbox" />
          {{ t("machineDetail.enableLocalAccountRemoteAdmin") }}
        </label>
        <p
          v-if="store.bootstrapResult"
          class="mt-2 text-sm"
          :class="store.bootstrapResult.ok ? 'text-status-healthy' : 'text-status-warning'"
        >
          {{ store.bootstrapResult.message }}
        </p>
        <p v-if="store.bootstrapError" class="mt-2 text-sm text-destructive">
          {{ store.bootstrapError }}
        </p>
        <pre
          v-if="store.bootstrapScript"
          data-bootstrap-script
          class="mt-3 max-h-72 overflow-auto rounded-md border bg-muted p-3 font-mono text-xs"
        >{{ store.bootstrapScript }}</pre>
      </article>

      <p v-if="store.refreshError" class="text-sm text-destructive">
        {{ t("machineDetail.refreshFailed", { message: store.refreshError }) }}
      </p>
    </section>

    <Projects v-else-if="activeTab === 'projects'" data-tab-content="projects" />

    <section v-else-if="activeTab === 'ddc'" data-tab-content="ddc" class="space-y-6 p-6">
      <header class="flex flex-wrap items-start justify-between gap-3">
        <div class="min-w-0">
          <h3 class="font-display text-lg font-extrabold">{{ t("machineDdc.title") }}</h3>
          <p class="mt-1 max-w-prose text-sm text-muted-foreground">
            {{ t("machineDdc.description") }}
          </p>
        </div>
        <Button data-create-share-btn @click="emit('open-share-wizard')">
          <UecmIcon name="plus" size="14" />
          {{ t("machineDdc.createShare") }}
        </Button>
      </header>
      <div class="rounded-lg border bg-card">
        <Shares />
      </div>
      <div class="rounded-lg border bg-card">
        <DDCPak />
      </div>
    </section>

    <PSOCache v-else-if="activeTab === 'pso'" data-tab-content="pso" />
    <INIScanner v-else-if="activeTab === 'ini'" data-tab-content="ini" />
    <HealthCheck v-else-if="activeTab === 'health'" data-tab-content="health" />
  </div>
</template>
