<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import BaseModal from "./BaseModal.vue";
import { useDiscoveryStore } from "@/stores/discovery";
import { useMachinesStore } from "@/stores/machines";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "close"): void }>();

const { t } = useI18n();
const discovery = useDiscoveryStore();
const machines = useMachinesStore();
const cidrInput = ref(discovery.cidr);

watch(() => props.open, (val) => {
  if (val) {
    cidrInput.value = discovery.cidr;
  }
});

async function onScan() {
  await discovery.scan(cidrInput.value);
}

async function onAdd(ip: string) {
  await discovery.addToInventory(ip, null);
  await machines.loadMachines();
}
</script>

<template>
  <BaseModal :open="props.open" :title="t('modal.discovery.title')" @close="emit('close')">
    <div>
      <label class="block text-sm mb-1">{{ t("modal.discovery.cidrLabel") }}</label>
      <div class="flex gap-2">
        <input
          data-cidr-input
          v-model="cidrInput"
          placeholder="192.168.10.0/24"
          class="flex-1 border rounded px-2 py-1 text-sm"
        />
        <button
          data-scan-btn
          :disabled="discovery.isScanning"
          class="px-3 py-1 text-sm bg-gray-200 rounded hover:bg-gray-300 disabled:opacity-50"
          @click="onScan"
        >
          {{ discovery.isScanning ? t("common.scanning") : t("machines.scan") }}
        </button>
      </div>

      <p v-if="discovery.error" class="mt-2 text-xs text-red-600">
        {{ discovery.error.message }}
      </p>

      <div class="mt-4">
        <p v-if="!discovery.isScanning && discovery.probed.length === 0" class="text-sm text-gray-500">
          {{ t("modal.discovery.noHosts") }}
        </p>
        <table v-else class="w-full text-sm border">
          <thead class="bg-gray-50">
            <tr>
              <th class="text-left px-2 py-1">{{ t("modal.discovery.headerIp") }}</th>
              <th class="text-left px-2 py-1">{{ t("modal.discovery.headerWinrm") }}</th>
              <th class="text-left px-2 py-1">{{ t("modal.discovery.headerSmb") }}</th>
              <th class="px-2 py-1"></th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="host in discovery.probed"
              :key="host.ip"
              data-probed-row
              class="border-t"
            >
              <td class="px-2 py-1 font-mono text-xs">{{ host.ip }}</td>
              <td class="px-2 py-1">{{ host.winrm_open ? "✓" : "—" }}</td>
              <td class="px-2 py-1">{{ host.smb_open ? "✓" : "—" }}</td>
              <td class="px-2 py-1 text-right">
                <button
                  data-add-btn
                  class="text-xs px-2 py-0.5 border rounded hover:bg-gray-100"
                  @click="onAdd(host.ip)"
                >
                  {{ t("common.add") }}
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
    <template #footer>
      <button class="px-3 py-1 text-sm border rounded hover:bg-gray-100" @click="emit('close')">
        {{ t("common.done") }}
      </button>
    </template>
  </BaseModal>
</template>
