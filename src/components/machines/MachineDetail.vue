<script setup lang="ts">
import { useMachinesStore } from "@/stores/machines";

const store = useMachinesStore();

const emit = defineEmits<{
  (e: "openEnvVarModal"): void;
  (e: "openIniEditModal"): void;
  (e: "openCredentialModal"): void;
}>();
</script>

<template>
  <div class="h-full flex flex-col">
    <div v-if="!store.selectedDetail" class="p-6 text-sm text-gray-500">
      Select a machine from the list to view details.
    </div>

    <div v-else class="p-6 overflow-auto">
      <header class="flex items-start justify-between mb-4">
        <div>
          <h2 class="text-xl font-semibold">{{ store.selectedDetail.machine.hostname }}</h2>
          <p class="text-sm text-gray-500">{{ store.selectedDetail.machine.ip }}</p>
        </div>
        <div class="flex gap-2">
          <button
            data-refresh-btn
            :disabled="store.isRefreshing"
            class="px-3 py-1 text-sm border rounded hover:bg-gray-100 disabled:opacity-50"
            @click="store.refreshSelected()"
          >
            {{ store.isRefreshing ? "Refreshing..." : "Refresh" }}
          </button>
          <button
            class="px-3 py-1 text-sm border rounded hover:bg-gray-100"
            @click="emit('openCredentialModal')"
          >
            Credentials
          </button>
          <button
            class="px-3 py-1 text-sm border rounded hover:bg-gray-100"
            @click="emit('openEnvVarModal')"
          >
            Env vars
          </button>
          <button
            class="px-3 py-1 text-sm border rounded hover:bg-gray-100"
            @click="emit('openIniEditModal')"
          >
            Edit INI
          </button>
        </div>
      </header>

      <section class="mt-4">
        <h3 class="font-medium mb-2">Basics</h3>
        <table class="text-sm w-full">
          <tbody>
            <tr><td class="py-1 text-gray-500 w-32">Role</td><td>{{ store.selectedDetail.machine.role }}</td></tr>
            <tr><td class="py-1 text-gray-500">Status</td><td>{{ store.selectedDetail.machine.status }}</td></tr>
            <tr><td class="py-1 text-gray-500">Last seen</td><td>{{ store.selectedDetail.machine.last_seen_at ?? "—" }}</td></tr>
          </tbody>
        </table>
      </section>

      <section class="mt-6">
        <h3 class="font-medium mb-2">UE installs</h3>
        <p v-if="store.selectedDetail.ue_installs.length === 0" class="text-sm text-gray-500">
          No UE installs detected. Click Refresh to scan.
        </p>
        <table v-else class="text-sm w-full border">
          <thead class="bg-gray-50">
            <tr>
              <th class="text-left px-3 py-1">Version</th>
              <th class="text-left px-3 py-1">Install path</th>
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
        <h3 class="font-medium mb-2">GPUs</h3>
        <p v-if="store.selectedDetail.gpus.length === 0" class="text-sm text-gray-500">
          No GPU info. Click Refresh to scan.
        </p>
        <table v-else class="text-sm w-full border">
          <thead class="bg-gray-50">
            <tr>
              <th class="text-left px-3 py-1">Model</th>
              <th class="text-left px-3 py-1">Driver</th>
              <th class="text-left px-3 py-1">VRAM</th>
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

      <section v-if="store.lastRefresh && !store.lastRefresh.winrm_ok" class="mt-4 text-sm text-red-600">
        Refresh failed: {{ store.lastRefresh.error ?? "WinRM unreachable" }}
      </section>
    </div>
  </div>
</template>
