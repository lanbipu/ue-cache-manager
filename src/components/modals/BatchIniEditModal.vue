<script setup lang="ts">
import { computed, ref, watch } from "vue";
import BaseModal from "./BaseModal.vue";
import BatchProgressTable from "@/components/batch/BatchProgressTable.vue";
import { useBatchStore } from "@/stores/batch";
import { useCredentialsStore } from "@/stores/credentials";
import { useMachinesStore } from "@/stores/machines";

const props = defineProps<{
  open: boolean;
  machineIds: number[];
}>();
const emit = defineEmits<{ (e: "close"): void }>();

const batch = useBatchStore();
const credentials = useCredentialsStore();
const machines = useMachinesStore();

const filePath = ref("C:\\path\\to\\Project\\Config\\DefaultEngine.ini");
const section = ref("Core.System");
const name = ref("");
const value = ref("");
const credentialAlias = ref<string>("");
const submitError = ref<string | null>(null);

watch(
  () => props.open,
  (val) => {
    if (val) {
      name.value = "";
      value.value = "";
      credentialAlias.value = "";
      submitError.value = null;
      batch.reset();
      credentials.load();
    }
  },
  { immediate: true },
);

const targetMachines = computed(() =>
  machines.machines.filter((m) => m.id != null && props.machineIds.includes(m.id)),
);

const canApply = computed(
  () =>
    !batch.isRunning &&
    filePath.value.trim() !== "" &&
    section.value.trim() !== "" &&
    name.value.trim() !== "" &&
    credentialAlias.value !== "" &&
    props.machineIds.length > 0,
);

async function onApply() {
  submitError.value = null;
  try {
    await batch.runIniKey(
      props.machineIds,
      filePath.value.trim(),
      section.value.trim(),
      name.value.trim(),
      value.value,
      credentialAlias.value,
    );
  } catch (e) {
    submitError.value = (e as { message?: string }).message ?? "batch failed";
  }
}
</script>

<template>
  <BaseModal :open="props.open" title="Batch INI key edit" @close="emit('close')">
    <div data-batch-ini-modal>
      <p class="text-sm mb-2 text-gray-600">
        Will apply to {{ machineIds.length }} machine{{ machineIds.length === 1 ? "" : "s" }}.
      </p>
      <label class="block text-sm mb-1">INI file path</label>
      <input data-ini-path v-model="filePath" class="w-full border rounded px-2 py-1 text-sm mb-3" />
      <label class="block text-sm mb-1">Section</label>
      <input data-ini-section v-model="section" class="w-full border rounded px-2 py-1 text-sm mb-3" />
      <label class="block text-sm mb-1">Key</label>
      <input data-ini-name v-model="name" class="w-full border rounded px-2 py-1 text-sm mb-3" />
      <label class="block text-sm mb-1">Value</label>
      <input data-ini-value v-model="value" class="w-full border rounded px-2 py-1 text-sm mb-3" />
      <label class="block text-sm mb-1">Credential alias</label>
      <select data-ini-cred v-model="credentialAlias" class="w-full border rounded px-2 py-1 text-sm mb-3">
        <option value="" disabled>— select credential —</option>
        <option v-for="c in credentials.credentials" :key="c.id ?? c.alias" :value="c.alias">
          {{ c.alias }}
        </option>
      </select>

      <BatchProgressTable :machines="targetMachines" :by-machine="batch.byMachine" />

      <p v-if="submitError" data-batch-error class="mt-2 text-xs text-red-600">
        {{ submitError }}
      </p>
    </div>

    <template #footer>
      <button
        data-batch-apply-btn
        :disabled="!canApply"
        class="px-3 py-1 text-sm bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
        @click="onApply"
      >
        {{ batch.isRunning ? "Running..." : "Apply" }}
      </button>
      <button class="px-3 py-1 text-sm border rounded hover:bg-gray-100" @click="emit('close')">
        Close
      </button>
    </template>
  </BaseModal>
</template>
