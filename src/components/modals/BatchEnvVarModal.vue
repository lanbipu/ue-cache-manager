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

const name = ref("UE-SharedDataCachePath");
const customName = ref("");
const value = ref("");
const credentialAlias = ref<string>("");
const submitError = ref<string | null>(null);

watch(
  () => props.open,
  (val) => {
    if (val) {
      name.value = "UE-SharedDataCachePath";
      customName.value = "";
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
    name.value.trim() !== "" &&
    (name.value !== "__custom" || customName.value.trim() !== "") &&
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
  const effectiveName = name.value === "__custom" ? customName.value.trim() : name.value;
  try {
    await batch.runEnvVar(props.machineIds, effectiveName, value.value, credentialAlias.value);
  } catch (e) {
    submitError.value = (e as { message?: string }).message ?? t("modal.batchEnv.batchFailed");
  }
}
</script>

<template>
  <BaseModal :open="props.open" :title="t('modal.batchEnv.title')" @close="emit('close')">
    <div data-batch-env-modal>
      <p class="text-sm mb-2 text-muted-foreground">
        {{ applyToLabel }}
      </p>
      <label class="block text-sm mb-1">{{ t("modal.batchEnv.varName") }}</label>
      <select
        data-env-name
        v-model="name"
        class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <option value="UE-SharedDataCachePath">{{ t("modal.batchEnv.shared") }}</option>
        <option value="UE-LocalDataCachePath">{{ t("modal.batchEnv.local") }}</option>
        <option value="__custom">{{ t("modal.batchEnv.custom") }}</option>
      </select>
      <input
        v-if="name === '__custom'"
        data-env-name-custom
        v-model="customName"
        class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        :placeholder="t('modal.batchEnv.customPlaceholder')"
      />
      <label class="block text-sm mb-1">{{ t("modal.batchEnv.value") }}</label>
      <input
        data-env-value
        v-model="value"
        class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      />
      <label class="block text-sm mb-1">{{ t("modal.batchEnv.credentialAlias") }}</label>
      <select
        data-env-cred
        v-model="credentialAlias"
        class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <option value="" disabled>{{ t("modal.batchEnv.selectCredentialPlaceholder") }}</option>
        <option v-for="c in credentials.credentials" :key="c.id ?? c.alias" :value="c.alias">
          {{ c.alias }}
        </option>
      </select>

      <BatchProgressTable :machines="targetMachines" :by-machine="batch.byMachine" />

      <p v-if="submitError" data-batch-error class="mt-2 text-xs text-destructive">
        {{ submitError }}
      </p>
    </div>

    <template #footer>
      <button
        data-batch-apply-btn
        :disabled="!canApply"
        class="rounded bg-primary px-3 py-1 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        @click="onApply"
      >
        {{ batch.isRunning ? t("common.running") : t("common.apply") }}
      </button>
      <button class="rounded border border-input px-3 py-1 text-sm hover:bg-accent hover:text-accent-foreground" @click="emit('close')">
        {{ t("common.close") }}
      </button>
    </template>
  </BaseModal>
</template>
