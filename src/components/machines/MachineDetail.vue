<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useMachinesStore } from "@/stores/machines";
import HostnameEditor from "@/components/machines/HostnameEditor.vue";

const { t } = useI18n();
const store = useMachinesStore();

const emit = defineEmits<{
  (e: "openEnvVarModal"): void;
  (e: "openIniEditModal"): void;
  (e: "openCredentialModal"): void;
}>();

function statusBadgeClass(status: string): string {
  if (status === "online") return "bg-green-500 text-white";
  if (status === "offline") return "bg-red-500 text-white";
  return "bg-gray-400 text-white";
}
</script>

<template>
  <div class="h-full flex flex-col">
    <div v-if="!store.selectedDetail" class="p-6 text-sm text-gray-500">
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
          <p class="text-sm text-gray-500">{{ store.selectedDetail.machine.ip }}</p>
        </div>
        <div class="flex gap-2">
          <button
            data-refresh-btn
            :disabled="store.isRefreshing"
            class="px-3 py-1 text-sm border rounded hover:bg-gray-100 disabled:opacity-50"
            @click="store.refreshSelected()"
          >
            {{ store.isRefreshing ? t("common.refreshing") : t("common.refresh") }}
          </button>
          <button
            class="px-3 py-1 text-sm border rounded hover:bg-gray-100"
            @click="emit('openCredentialModal')"
          >
            {{ t("machineDetail.credentials") }}
          </button>
          <button
            class="px-3 py-1 text-sm border rounded hover:bg-gray-100"
            @click="emit('openEnvVarModal')"
          >
            {{ t("machineDetail.envVars") }}
          </button>
          <button
            class="px-3 py-1 text-sm border rounded hover:bg-gray-100"
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
            <tr><td class="py-1 text-gray-500 w-32">{{ t("machineDetail.role") }}</td><td>{{ store.selectedDetail.machine.role }}</td></tr>
            <tr><td class="py-1 text-gray-500">{{ t("machineDetail.status") }}</td><td>{{ store.selectedDetail.machine.status }}</td></tr>
            <tr><td class="py-1 text-gray-500">{{ t("machineDetail.lastSeen") }}</td><td>{{ store.selectedDetail.machine.last_seen_at ?? "—" }}</td></tr>
          </tbody>
        </table>
      </section>

      <section class="mt-6">
        <h3 class="font-medium mb-2">{{ t("machineDetail.ueInstalls") }}</h3>
        <p v-if="store.selectedDetail.ue_installs.length === 0" class="text-sm text-gray-500">
          {{ t("machineDetail.noUeInstalls") }}
        </p>
        <table v-else class="text-sm w-full border">
          <thead class="bg-gray-50">
            <tr>
              <th class="text-left px-3 py-1">{{ t("machineDetail.headerVersion") }}</th>
              <th class="text-left px-3 py-1">{{ t("machineDetail.headerInstallPath") }}</th>
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
        <p v-if="store.selectedDetail.gpus.length === 0" class="text-sm text-gray-500">
          {{ t("machineDetail.noGpus") }}
        </p>
        <table v-else class="text-sm w-full border">
          <thead class="bg-gray-50">
            <tr>
              <th class="text-left px-3 py-1">{{ t("machineDetail.headerModel") }}</th>
              <th class="text-left px-3 py-1">{{ t("machineDetail.headerDriver") }}</th>
              <th class="text-left px-3 py-1">{{ t("machineDetail.headerVram") }}</th>
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

      <section v-if="store.refreshError" class="mt-4 text-sm text-red-600">
        {{ t("machineDetail.refreshFailed", { message: store.refreshError }) }}
      </section>
    </div>
  </div>
</template>
