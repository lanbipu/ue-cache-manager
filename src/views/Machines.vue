<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useMachinesStore } from "@/stores/machines";
import MachineDetail from "@/components/machines/MachineDetail.vue";
import DiscoveryWizard from "@/components/modals/DiscoveryWizard.vue";
import CredentialDialog from "@/components/modals/CredentialDialog.vue";
import EnvVarConfigModal from "@/components/modals/EnvVarConfigModal.vue";
import IniEditModal from "@/components/modals/IniEditModal.vue";
import ShareCreateWizard from "@/components/modals/ShareCreateWizard.vue";

const store = useMachinesStore();
const showDiscovery = ref(false);
const showCredentials = ref(false);
const showEnvVar = ref(false);
const showIniEdit = ref(false);
const showShareWizard = ref(false);

const selectedId = computed(() => store.selectedDetail?.machine.id ?? null);

onMounted(() => {
  store.loadMachines();
});

async function onSelect(id: number | null) {
  if (id === null) return;
  await store.selectMachine(id);
}

async function onDelete(id: number | null) {
  if (id === null) return;
  await store.deleteMachine(id);
}
</script>

<template>
  <div class="h-full flex">
    <aside class="w-80 border-r overflow-auto p-4">
      <header class="flex items-center justify-between mb-3 gap-2">
        <h1 class="text-lg font-semibold">Machines</h1>
        <div class="flex gap-1">
          <button
            data-create-share-btn
            class="px-2 py-1 text-xs border rounded hover:bg-gray-100"
            @click="showShareWizard = true"
          >
            Share
          </button>
          <button
            data-discover-btn
            class="px-3 py-1 text-sm bg-gray-200 rounded hover:bg-gray-300"
            @click="showDiscovery = true"
          >
            Scan
          </button>
        </div>
      </header>

      <p v-if="store.isLoading" class="text-sm text-gray-500">Loading...</p>
      <p v-else-if="store.machines.length === 0" class="text-sm text-gray-500">
        No machines yet. Click Scan to discover.
      </p>
      <ul v-else class="space-y-1">
        <li
          v-for="m in store.machines"
          :key="m.id ?? m.ip"
          data-machine-row
          class="px-2 py-2 rounded cursor-pointer hover:bg-gray-100 flex items-center justify-between"
          :class="store.selectedDetail?.machine.id === m.id ? 'bg-gray-200 font-medium' : ''"
          @click="onSelect(m.id)"
        >
          <span class="truncate">
            {{ m.hostname }}<br />
            <span class="text-xs text-gray-500 font-normal">{{ m.ip }}</span>
          </span>
          <button
            class="text-xs text-red-600 hover:underline ml-2"
            @click.stop="onDelete(m.id)"
          >
            ×
          </button>
        </li>
      </ul>

      <p v-if="store.error" class="mt-3 text-xs text-red-600">
        {{ store.error.message }}
      </p>
    </aside>

    <main class="flex-1 overflow-auto">
      <MachineDetail
        @open-credential-modal="showCredentials = true"
        @open-env-var-modal="showEnvVar = true"
        @open-ini-edit-modal="showIniEdit = true"
      />
    </main>

    <DiscoveryWizard :open="showDiscovery" @close="showDiscovery = false" />
    <CredentialDialog :open="showCredentials" @close="showCredentials = false" />
    <EnvVarConfigModal
      :open="showEnvVar"
      :machine-id="selectedId"
      var-name="UE-SharedDataCachePath"
      @close="showEnvVar = false"
    />
    <IniEditModal
      :open="showIniEdit"
      :machine-id="selectedId"
      @close="showIniEdit = false"
    />
    <ShareCreateWizard :open="showShareWizard" @close="showShareWizard = false" />
  </div>
</template>
