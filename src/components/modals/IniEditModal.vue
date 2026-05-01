<script setup lang="ts">
import { ref, watch } from "vue";
import BaseModal from "./BaseModal.vue";
import { tauriApi, type IniKey, type UecmError } from "@/services/tauri";

const props = defineProps<{
  open: boolean;
  machineId: number | null;
}>();
const emit = defineEmits<{ (e: "close"): void }>();

const filePath = ref("");
const section = ref("");
const keyName = ref("");
const keyValue = ref("");
const loadedKeys = ref<IniKey[]>([]);
const lastBackup = ref<string | null>(null);
const reading = ref(false);
const applying = ref(false);
const error = ref<UecmError | null>(null);

watch(
  () => props.open,
  (val) => {
    if (val) {
      filePath.value = "";
      section.value = "";
      keyName.value = "";
      keyValue.value = "";
      loadedKeys.value = [];
      lastBackup.value = null;
      error.value = null;
    }
  },
);

async function onRead() {
  if (props.machineId === null || !filePath.value || !section.value) return;
  reading.value = true;
  error.value = null;
  try {
    loadedKeys.value = await tauriApi.readIniSection(props.machineId, filePath.value, section.value);
  } catch (e) {
    error.value = e as UecmError;
    loadedKeys.value = [];
  } finally {
    reading.value = false;
  }
}

async function onApply() {
  if (props.machineId === null || !filePath.value || !section.value || !keyName.value) return;
  applying.value = true;
  error.value = null;
  try {
    const result = await tauriApi.setIniKey(
      props.machineId,
      filePath.value,
      section.value,
      keyName.value,
      keyValue.value,
    );
    lastBackup.value = result.backup_path;
    // Re-read to confirm
    await onRead();
  } catch (e) {
    error.value = e as UecmError;
  } finally {
    applying.value = false;
  }
}
</script>

<template>
  <BaseModal :open="props.open" title="Edit INI key" @close="emit('close')">
    <div>
      <p class="text-xs text-gray-500 mb-2">
        Reads/writes a single key in an [section] of an INI file on the remote machine.
        Auto-backs up the file before writing.
      </p>

      <label class="block text-sm mb-1">File path (remote)</label>
      <input
        data-ini-path
        v-model="filePath"
        placeholder="C:\\Path\\To\\Project\\Config\\DefaultEngine.ini"
        class="w-full border rounded px-2 py-1 text-sm font-mono mb-2"
      />

      <label class="block text-sm mb-1">Section (without brackets)</label>
      <input
        data-ini-section
        v-model="section"
        placeholder="Core.System"
        class="w-full border rounded px-2 py-1 text-sm font-mono mb-2"
      />

      <button
        data-ini-read-btn
        :disabled="reading"
        class="px-3 py-1 text-sm border rounded hover:bg-gray-100 disabled:opacity-50 mb-3"
        @click="onRead"
      >
        {{ reading ? "Reading..." : "Read section" }}
      </button>

      <div v-if="loadedKeys.length > 0" class="mb-3">
        <p class="text-sm font-medium mb-1">Existing keys</p>
        <table class="w-full text-xs border">
          <thead class="bg-gray-50">
            <tr><th class="text-left px-2 py-1">Name</th><th class="text-left px-2 py-1">Value</th></tr>
          </thead>
          <tbody>
            <tr v-for="k in loadedKeys" :key="k.name" data-ini-row class="border-t">
              <td class="px-2 py-1 font-mono">{{ k.name }}</td>
              <td class="px-2 py-1 font-mono break-all">{{ k.value }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <hr class="my-3" />

      <label class="block text-sm mb-1">Key name</label>
      <input
        data-ini-key
        v-model="keyName"
        placeholder="DDCStrategy"
        class="w-full border rounded px-2 py-1 text-sm font-mono mb-2"
      />

      <label class="block text-sm mb-1">New value</label>
      <input
        data-ini-value
        v-model="keyValue"
        placeholder="Filesystem"
        class="w-full border rounded px-2 py-1 text-sm font-mono mb-2"
      />

      <p v-if="lastBackup" class="text-xs text-green-700">
        Applied. Backup saved to <span class="font-mono">{{ lastBackup }}</span>
      </p>
      <p v-if="error" class="text-xs text-red-600">{{ error.message }}</p>
    </div>
    <template #footer>
      <button class="px-3 py-1 text-sm border rounded hover:bg-gray-100" @click="emit('close')">
        Cancel
      </button>
      <button
        data-ini-apply-btn
        :disabled="applying"
        class="px-3 py-1 text-sm bg-gray-200 rounded hover:bg-gray-300 disabled:opacity-50"
        @click="onApply"
      >
        {{ applying ? "Applying..." : "Apply" }}
      </button>
    </template>
  </BaseModal>
</template>
