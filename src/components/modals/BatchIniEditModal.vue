<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
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

const { t } = useI18n();
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

const applyToLabel = computed(() => {
  const count = props.machineIds.length;
  return count === 1
    ? t("modal.batchEnv.applyToOne", { count })
    : t("modal.batchEnv.applyToMany", { count });
});

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
    submitError.value = (e as { message?: string }).message ?? t("modal.batchEnv.batchFailed");
  }
}
</script>

<template>
  <BaseModal :open="props.open" :title="t('modal.batchIni.title')" @close="emit('close')">
    <div data-batch-ini-modal>
      <p class="text-sm mb-2 text-gray-600">
        {{ applyToLabel }}
      </p>
      <label class="block text-sm mb-1">{{ t("modal.batchIni.iniPath") }}</label>
      <input data-ini-path v-model="filePath" class="w-full border rounded px-2 py-1 text-sm mb-3" />
      <label class="block text-sm mb-1">{{ t("modal.batchIni.section") }}</label>
      <input data-ini-section v-model="section" class="w-full border rounded px-2 py-1 text-sm mb-3" />
      <label class="block text-sm mb-1">{{ t("modal.batchIni.key") }}</label>
      <input data-ini-name v-model="name" class="w-full border rounded px-2 py-1 text-sm mb-3" />
      <label class="block text-sm mb-1">{{ t("modal.batchIni.value") }}</label>
      <input data-ini-value v-model="value" class="w-full border rounded px-2 py-1 text-sm mb-3" />
      <label class="block text-sm mb-1">{{ t("modal.batchIni.credentialAlias") }}</label>
      <select data-ini-cred v-model="credentialAlias" class="w-full border rounded px-2 py-1 text-sm mb-3">
        <option value="" disabled>{{ t("modal.batchEnv.selectCredentialPlaceholder") }}</option>
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
        {{ batch.isRunning ? t("common.running") : t("common.apply") }}
      </button>
      <button class="px-3 py-1 text-sm border rounded hover:bg-gray-100" @click="emit('close')">
        {{ t("common.close") }}
      </button>
    </template>
  </BaseModal>
</template>
