<script setup lang="ts">
import { ref, watch } from "vue";
import BaseModal from "./BaseModal.vue";
import { tauriApi, type UecmError } from "@/services/tauri";

const props = defineProps<{
  open: boolean;
  machineId: number | null;
  varName: string;
}>();
const emit = defineEmits<{ (e: "close"): void }>();

const currentValue = ref<string | null>(null);
const newValue = ref("");
const loading = ref(false);
const applying = ref(false);
const error = ref<UecmError | null>(null);
const applied = ref(false);

watch(
  () => [props.open, props.machineId],
  async ([open, id]) => {
    if (!open || id === null) return;
    loading.value = true;
    applied.value = false;
    error.value = null;
    try {
      currentValue.value = await tauriApi.getMachineEnvVar(id as number, props.varName);
      newValue.value = currentValue.value ?? "";
    } catch (e) {
      error.value = e as UecmError;
      currentValue.value = null;
    } finally {
      loading.value = false;
    }
  },
);

async function onApply() {
  if (props.machineId === null) return;
  applying.value = true;
  error.value = null;
  try {
    await tauriApi.setMachineEnvVar(props.machineId, props.varName, newValue.value);
    currentValue.value = newValue.value;
    applied.value = true;
  } catch (e) {
    error.value = e as UecmError;
  } finally {
    applying.value = false;
  }
}
</script>

<template>
  <BaseModal :open="props.open" :title="`Set env var: ${props.varName}`" @close="emit('close')">
    <div>
      <p class="text-xs text-gray-500 mb-2">
        Sets the system-level env var on the remote machine. Requires WinRM admin.
      </p>

      <p class="text-sm mb-1">Current value</p>
      <p data-current-value class="text-sm font-mono bg-gray-50 border rounded px-2 py-1 mb-3 break-all">
        <span v-if="loading">loading...</span>
        <span v-else>{{ currentValue ?? "(not set)" }}</span>
      </p>

      <p class="text-sm mb-1">New value</p>
      <input
        data-new-value
        v-model="newValue"
        placeholder="\\HOST\DDC"
        class="w-full border rounded px-2 py-1 text-sm font-mono"
      />

      <p v-if="error" class="mt-2 text-xs text-red-600">{{ error.message }}</p>
      <p v-if="applied" class="mt-2 text-xs text-green-700">Applied + verified.</p>
    </div>
    <template #footer>
      <button class="px-3 py-1 text-sm border rounded hover:bg-gray-100" @click="emit('close')">
        Cancel
      </button>
      <button
        data-apply-btn
        :disabled="applying || loading"
        class="px-3 py-1 text-sm bg-gray-200 rounded hover:bg-gray-300 disabled:opacity-50"
        @click="onApply"
      >
        {{ applying ? "Applying..." : "Apply" }}
      </button>
    </template>
  </BaseModal>
</template>
