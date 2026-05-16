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
          class="flex-1 rounded border border-input bg-transparent px-2 py-1 text-sm text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        />
        <button
          data-scan-btn
          :disabled="discovery.isScanning"
          class="rounded bg-primary px-3 py-1 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          @click="onScan"
        >
          {{ discovery.isScanning ? t("common.scanning") : t("machines.scan") }}
        </button>
      </div>

      <p v-if="discovery.error" class="mt-2 text-xs text-destructive">
        {{ discovery.error.message }}
      </p>

      <div class="mt-4">
        <p v-if="!discovery.isScanning && discovery.probed.length === 0" class="text-sm text-muted-foreground">
          {{ t("modal.discovery.noHosts") }}
        </p>
        <table v-else class="w-full text-sm border">
          <thead class="bg-muted/40 text-muted-foreground">
            <tr>
              <th class="text-left px-2 py-1 font-medium">{{ t("modal.discovery.headerIp") }}</th>
              <th class="text-left px-2 py-1 font-medium">{{ t("modal.discovery.headerWinrm") }}</th>
              <th class="text-left px-2 py-1 font-medium">{{ t("modal.discovery.headerSmb") }}</th>
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
                  class="rounded border border-input px-2 py-0.5 text-xs hover:bg-accent hover:text-accent-foreground"
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
      <button class="rounded border border-input px-3 py-1 text-sm hover:bg-accent hover:text-accent-foreground" @click="emit('close')">
        {{ t("common.done") }}
      </button>
    </template>
  </BaseModal>
</template>
