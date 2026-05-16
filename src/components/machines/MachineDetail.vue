<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useMachinesStore } from "@/stores/machines";
import { useCredentialsStore } from "@/stores/credentials";
import HostnameEditor from "@/components/machines/HostnameEditor.vue";

const { t } = useI18n();
const store = useMachinesStore();
const credentials = useCredentialsStore();
const bootstrapCredentialAlias = ref("");
const enableLocalAccountRemoteAdmin = ref(false);

const winrmCredentials = computed(() => credentials.credentials.filter((cred) => cred.kind === "winrm"));
const showBootstrapPanel = computed(() => {
  const detail = store.selectedDetail;
  return Boolean(detail && detail.machine.status !== "online");
});

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

const emit = defineEmits<{
  (e: "openEnvVarModal"): void;
  (e: "openIniEditModal"): void;
  (e: "openCredentialModal"): void;
}>();

function statusBadgeClass(status: string): string {
  if (status === "online") return "bg-emerald-500 text-white dark:bg-emerald-600";
  if (status === "offline") return "bg-rose-500 text-white dark:bg-rose-600";
  return "bg-muted text-muted-foreground";
}

function runBootstrap() {
  if (!bootstrapCredentialAlias.value) return;
  store.bootstrapSelected(bootstrapCredentialAlias.value, enableLocalAccountRemoteAdmin.value);
}
</script>

<template>
  <div class="h-full flex flex-col">
    <div v-if="!store.selectedDetail" class="p-6 text-sm text-muted-foreground">
      {{ t("machineDetail.selectHint") }}
    </div>

    <div v-else class="p-6 overflow-auto">
      <header class="flex items-start justify-between mb-4">
        <div>
          <div class="flex items-center gap-2">
            <h2 class="text-xl font-semibold">
              <HostnameEditor
                :value="store.selectedDetail.machine.hostname"
                @save="(v) => store.renameMachine(store.selectedDetail!.machine.id!, v)"
              />
            </h2>
            <span
              data-status-badge
              class="text-xs px-2 py-0.5 rounded-full"
              :class="statusBadgeClass(store.selectedDetail.machine.status)"
            >
              {{ store.selectedDetail.machine.status }}
            </span>
          </div>
          <p class="text-sm text-muted-foreground">{{ store.selectedDetail.machine.ip }}</p>
        </div>
        <div class="flex gap-2">
          <button
            data-refresh-btn
            :disabled="store.isRefreshing"
            class="rounded border border-input px-3 py-1 text-sm hover:bg-accent hover:text-accent-foreground disabled:opacity-50"
            @click="store.refreshSelected()"
          >
            {{ store.isRefreshing ? t("common.refreshing") : t("common.refresh") }}
          </button>
          <button
            class="rounded border border-input px-3 py-1 text-sm hover:bg-accent hover:text-accent-foreground"
            @click="emit('openCredentialModal')"
          >
            {{ t("machineDetail.credentials") }}
          </button>
          <button
            class="rounded border border-input px-3 py-1 text-sm hover:bg-accent hover:text-accent-foreground"
            @click="emit('openEnvVarModal')"
          >
            {{ t("machineDetail.envVars") }}
          </button>
          <button
            class="rounded border border-input px-3 py-1 text-sm hover:bg-accent hover:text-accent-foreground"
            @click="emit('openIniEditModal')"
          >
            {{ t("machineDetail.editIni") }}
          </button>
        </div>
      </header>

      <section class="mt-4">
        <h3 class="font-medium mb-2">{{ t("machineDetail.basics") }}</h3>
        <table class="text-sm w-full">
          <tbody>
            <tr><td class="py-1 text-muted-foreground w-32">{{ t("machineDetail.role") }}</td><td>{{ store.selectedDetail.machine.role }}</td></tr>
            <tr><td class="py-1 text-muted-foreground">{{ t("machineDetail.status") }}</td><td>{{ store.selectedDetail.machine.status }}</td></tr>
            <tr><td class="py-1 text-muted-foreground">{{ t("machineDetail.lastSeen") }}</td><td>{{ store.selectedDetail.machine.last_seen_at ?? "—" }}</td></tr>
          </tbody>
        </table>
      </section>

      <section
        v-if="showBootstrapPanel"
        data-bootstrap-panel
        class="mt-5 border-l-2 border-amber-500 pl-4"
      >
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div class="min-w-0">
            <h3 class="font-medium">{{ t("machineDetail.bootstrapTitle") }}</h3>
            <p class="mt-1 max-w-3xl text-sm text-muted-foreground">
              {{ t("machineDetail.bootstrapHint") }}
            </p>
          </div>
          <button
            data-load-bootstrap-script
            class="rounded border border-input px-3 py-1 text-sm hover:bg-accent hover:text-accent-foreground disabled:opacity-50"
            :disabled="store.isLoadingBootstrapScript"
            @click="store.loadBootstrapScript()"
          >
            {{ store.isLoadingBootstrapScript ? t("common.loading") : t("machineDetail.showBootstrapScript") }}
          </button>
        </div>

        <div class="mt-3 flex flex-wrap items-center gap-2">
          <select
            v-model="bootstrapCredentialAlias"
            data-bootstrap-cred
            class="h-8 min-w-64 rounded border border-input bg-transparent px-2 text-sm text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <option value="">{{ t("machineDetail.selectWinrmCredential") }}</option>
            <option v-for="cred in winrmCredentials" :key="cred.alias" :value="cred.alias">
              {{ cred.alias }} — {{ cred.username }}
            </option>
          </select>
          <button
            data-bootstrap-btn
            class="rounded border border-input px-3 py-1 text-sm hover:bg-accent hover:text-accent-foreground disabled:opacity-50"
            :disabled="store.isBootstrapping || !bootstrapCredentialAlias"
            @click="runBootstrap"
          >
            {{ store.isBootstrapping ? t("machineDetail.bootstrapping") : t("machineDetail.runBootstrap") }}
          </button>
          <button
            v-if="winrmCredentials.length === 0"
            class="rounded border border-input px-3 py-1 text-sm hover:bg-accent hover:text-accent-foreground"
            @click="emit('openCredentialModal')"
          >
            {{ t("machineDetail.addCredential") }}
          </button>
        </div>

        <label class="mt-2 flex items-center gap-2 text-xs text-muted-foreground">
          <input
            v-model="enableLocalAccountRemoteAdmin"
            data-bootstrap-local-admin
            type="checkbox"
          />
          {{ t("machineDetail.enableLocalAccountRemoteAdmin") }}
        </label>

        <p v-if="store.bootstrapResult" class="mt-2 text-sm" :class="store.bootstrapResult.ok ? 'text-emerald-600 dark:text-emerald-400' : 'text-amber-600 dark:text-amber-400'">
          {{ store.bootstrapResult.message }}
        </p>
        <p v-if="store.bootstrapError" class="mt-2 text-sm text-destructive">
          {{ store.bootstrapError }}
        </p>
        <pre
          v-if="store.bootstrapScript"
          data-bootstrap-script
          class="mt-3 max-h-72 overflow-auto rounded border bg-zinc-950 p-3 text-xs text-zinc-100"
        >{{ store.bootstrapScript }}</pre>
      </section>

      <section class="mt-6">
        <h3 class="font-medium mb-2">{{ t("machineDetail.ueInstalls") }}</h3>
        <p v-if="store.selectedDetail.ue_installs.length === 0" class="text-sm text-muted-foreground">
          {{ t("machineDetail.noUeInstalls") }}
        </p>
        <table v-else class="text-sm w-full border">
          <thead class="bg-muted/40 text-muted-foreground">
            <tr>
              <th class="text-left px-3 py-1 font-medium">{{ t("machineDetail.headerVersion") }}</th>
              <th class="text-left px-3 py-1 font-medium">{{ t("machineDetail.headerInstallPath") }}</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="install in store.selectedDetail.ue_installs"
              :key="install.id ?? install.version"
              class="border-t"
            >
              <td class="px-3 py-1">{{ install.version }}</td>
              <td class="px-3 py-1 font-mono text-xs">{{ install.install_path }}</td>
            </tr>
          </tbody>
        </table>
      </section>

      <section class="mt-6">
        <h3 class="font-medium mb-2">{{ t("machineDetail.gpus") }}</h3>
        <p v-if="store.selectedDetail.gpus.length === 0" class="text-sm text-muted-foreground">
          {{ t("machineDetail.noGpus") }}
        </p>
        <table v-else class="text-sm w-full border">
          <thead class="bg-muted/40 text-muted-foreground">
            <tr>
              <th class="text-left px-3 py-1 font-medium">{{ t("machineDetail.headerModel") }}</th>
              <th class="text-left px-3 py-1 font-medium">{{ t("machineDetail.headerDriver") }}</th>
              <th class="text-left px-3 py-1 font-medium">{{ t("machineDetail.headerVram") }}</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="gpu in store.selectedDetail.gpus"
              :key="gpu.id ?? gpu.gpu_model"
              class="border-t"
            >
              <td class="px-3 py-1">{{ gpu.gpu_model }}</td>
              <td class="px-3 py-1">{{ gpu.driver_version }}</td>
              <td class="px-3 py-1">{{ gpu.vram_mb ? gpu.vram_mb + " MB" : "—" }}</td>
            </tr>
          </tbody>
        </table>
      </section>

      <section v-if="store.refreshError" class="mt-4 text-sm text-destructive">
        {{ t("machineDetail.refreshFailed", { message: store.refreshError }) }}
      </section>
    </div>
  </div>
</template>
